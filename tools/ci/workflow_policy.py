#!/usr/bin/env python3
"""Enforce Python-only repository workflow orchestration."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Sequence


SCRIPT_SUFFIXES = frozenset({".sh", ".bash", ".ps1", ".bat", ".cmd"})
WORKFLOW_SUFFIXES = frozenset({".yml", ".yaml"})
GITLINK_MODE = "160000"

_STEP_KEY = r"(?:-\s+)?"
_BLOCK_SCALAR_INDICATORS = r"(?:[+-]?[1-9]?|[1-9][+-]?)"
_LITERAL_RUN = re.compile(
    rf"^(?P<indent>\s*){_STEP_KEY}run:\s*\|{_BLOCK_SCALAR_INDICATORS}\s*(?:#.*)?$"
)
_FOLDED_RUN = re.compile(
    rf"^(?P<indent>\s*){_STEP_KEY}run:\s*>{_BLOCK_SCALAR_INDICATORS}\s*(?:#.*)?$"
)
_INLINE_RUN = re.compile(
    rf"^(?P<indent>\s*){_STEP_KEY}run:\s*(?P<command>\S.*)$"
)
_EXPLICIT_SHELL = re.compile(
    rf"^\s*{_STEP_KEY}shell:\s*(?P<shell>\S(?:.*\S)?)\s*$"
)
_GITHUB_EXPRESSION = re.compile(r"\$\{\{.*?\}\}", re.DOTALL)
_YAML_QUOTED_SCALAR = re.compile(
    r"^(?P<quote>['\"])(?P<value>.*)(?P=quote)(?:\s+#.*)?$", re.DOTALL
)
_QUOTED_TEXT = re.compile(r"'(?:[^']|'')*'|\"(?:\\.|[^\"\\])*\"")
_CONTROL_FLOW = (
    re.compile(
        r"(?is)(?:^|[;&|]\s*)if\s*(?:\(|(?:not\s+)?(?:exist|defined|errorlevel)\b|.*?\bthen\b)"
    ),
    re.compile(
        r"(?is)(?:^|[;&|]\s*)(?:for|foreach|while|until|select)\s*(?:\(|\b).*?(?:\bdo\b|\bin\b|\{)"
    ),
    re.compile(r"(?is)(?:^|[;&|]\s*)case\s+.*?\bin\b"),
    re.compile(r"(?is)(?:^|[;&|]\s*)switch\s*(?:\(|\b)"),
    re.compile(r"(?is)(?:^|[;&|]\s*)(?:try|catch|finally)\s*\{"),
    re.compile(r"(?im)^\s*(?:then|elif|else|fi|done|esac)\b"),
    re.compile(r"(?:&&|\|\|)"),
    re.compile(r";\s*\S"),
    re.compile(r"(?<!\|)\|(?!\|)"),
)


class PolicyError(RuntimeError):
    """The repository policy could not inspect its tracked inputs."""


@dataclass(frozen=True)
class IndexEntry:
    """One path and object mode from the Git index."""

    mode: str
    path: PurePosixPath


@dataclass(frozen=True)
class PolicyViolation:
    """One stable policy diagnostic."""

    path: PurePosixPath
    line: int
    message: str

    def __str__(self) -> str:
        return f"{self.path.as_posix()}:{self.line}: {self.message}"


def _parse_index(output: bytes) -> tuple[IndexEntry, ...]:
    entries = []
    for record in output.split(b"\0"):
        if not record:
            continue
        try:
            metadata, raw_path = record.split(b"\t", 1)
            mode = metadata.split(b" ", 1)[0].decode("ascii")
        except (UnicodeDecodeError, ValueError) as error:
            raise PolicyError("git ls-files returned malformed index data") from error
        entries.append(IndexEntry(mode=mode, path=PurePosixPath(os.fsdecode(raw_path))))
    return tuple(entries)


def tracked_index(repo_root: Path) -> tuple[IndexEntry, ...]:
    """Read the staged tracked-file boundary without descending into submodules."""
    try:
        result = subprocess.run(
            ("git", "ls-files", "--stage", "-z"),
            cwd=repo_root,
            check=False,
            capture_output=True,
        )
    except OSError as error:
        raise PolicyError(f"could not execute git ls-files: {error}") from error
    if result.returncode != 0:
        diagnostic = os.fsdecode(result.stderr).strip() or "git ls-files failed"
        raise PolicyError(diagnostic)
    return _parse_index(result.stdout)


def _inside(path: PurePosixPath, root: PurePosixPath) -> bool:
    return path == root or root in path.parents


def _workflow_path(path: PurePosixPath) -> bool:
    return (
        len(path.parts) >= 3
        and path.parts[:2] == (".github", "workflows")
        and path.suffix.casefold() in WORKFLOW_SUFFIXES
    )


def _leading_width(line: str) -> int:
    return len(line) - len(line.lstrip())


def _block_lines(lines: Sequence[str], header: int, indent: int) -> tuple[str, ...]:
    content = []
    for line in lines[header + 1 :]:
        if line.strip() and _leading_width(line) <= indent:
            break
        content.append(line)
    return tuple(content)


def _contains_control_flow(command: str) -> bool:
    quoted_scalar = _YAML_QUOTED_SCALAR.fullmatch(command.strip())
    if quoted_scalar is not None:
        command = quoted_scalar.group("value")
    command = _GITHUB_EXPRESSION.sub("EXPRESSION", command)
    command = _QUOTED_TEXT.sub("QUOTED", command)
    command = re.sub(r"(?m)#.*$", "", command)
    return any(pattern.search(command) is not None for pattern in _CONTROL_FLOW)


def _explicit_shell(value: str) -> str | None:
    value = value.split("#", 1)[0].strip()
    match = re.match(
        r"^[\"']?(bash|pwsh|powershell|cmd)[\"']?(?:\s|$)", value, re.IGNORECASE
    )
    return match.group(1).casefold() if match is not None else None


def check_workflow_text(
    path: PurePosixPath, source: str
) -> tuple[PolicyViolation, ...]:
    """Check one tracked workflow and return stable source diagnostics."""
    lines = source.splitlines()
    violations = []
    for index, line in enumerate(lines):
        line_number = index + 1
        shell_match = _EXPLICIT_SHELL.match(line)
        if shell_match is not None:
            shell = _explicit_shell(shell_match.group("shell"))
            if shell is not None:
                violations.append(
                    PolicyViolation(
                        path,
                        line_number,
                        f"explicit runner shell {shell!r} is not allowed",
                    )
                )

        literal_match = _LITERAL_RUN.match(line)
        if literal_match is not None:
            violations.append(
                PolicyViolation(
                    path,
                    line_number,
                    "literal multi-line run blocks are not allowed",
                )
            )
            continue

        folded_match = _FOLDED_RUN.match(line)
        if folded_match is not None:
            block = _block_lines(
                lines, index, len(folded_match.group("indent"))
            )
            command = " ".join(part.strip() for part in block if part.strip())
            if _contains_control_flow(command):
                violations.append(
                    PolicyViolation(path, line_number, "shell control flow is not allowed")
                )
            continue

        inline_match = _INLINE_RUN.match(line)
        if inline_match is not None and _contains_control_flow(
            inline_match.group("command")
        ):
            violations.append(
                PolicyViolation(path, line_number, "shell control flow is not allowed")
            )
    return tuple(violations)


def check_repository(repo_root: Path) -> tuple[PolicyViolation, ...]:
    """Check every maintained tracked script and GitHub workflow."""
    root = Path(repo_root).resolve()
    entries = tracked_index(root)
    gitlinks = tuple(entry.path for entry in entries if entry.mode == GITLINK_MODE)
    violations = []
    for entry in entries:
        if entry.mode == GITLINK_MODE or any(
            _inside(entry.path, gitlink) for gitlink in gitlinks
        ):
            continue
        if entry.path.suffix.casefold() in SCRIPT_SUFFIXES:
            violations.append(
                PolicyViolation(
                    entry.path,
                    1,
                    "tracked maintained command script is not allowed",
                )
            )
        if not _workflow_path(entry.path):
            continue
        workflow = root.joinpath(*entry.path.parts)
        try:
            source = workflow.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            violations.append(
                PolicyViolation(
                    entry.path, 1, f"tracked workflow could not be read: {error}"
                )
            )
            continue
        violations.extend(check_workflow_text(entry.path, source))
    return tuple(
        sorted(
            violations,
            key=lambda violation: (
                violation.path.as_posix(),
                violation.line,
                violation.message,
            ),
        )
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="explicitly run repository validation (the default mode)",
    )
    parser.add_argument(
        "--repo-root", type=Path, default=Path(__file__).resolve().parents[2]
    )
    arguments = parser.parse_args(argv)
    try:
        violations = check_repository(arguments.repo_root)
    except PolicyError as error:
        print(f"workflow-policy: {error}", file=sys.stderr)
        return 2
    for violation in violations:
        print(violation, file=sys.stderr)
    if violations:
        return 1
    print("Workflow policy passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
