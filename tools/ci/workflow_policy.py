#!/usr/bin/env python3
"""Require repository-maintained command scripts to be Python."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Sequence


SCRIPT_SUFFIXES = frozenset({".sh", ".bash", ".zsh", ".ksh", ".ps1", ".bat", ".cmd"})
SHELL_INTERPRETERS = frozenset({b"sh", b"bash", b"zsh", b"ksh", b"fish"})
GITLINK_MODE = "160000"


class PolicyError(RuntimeError):
    """The repository index could not be inspected."""


@dataclass(frozen=True)
class PolicyViolation:
    path: PurePosixPath
    message: str

    def __str__(self) -> str:
        return f"{self.path.as_posix()}:1: {self.message}"


def _has_shell_shebang(first_line: bytes) -> bool:
    if not first_line.startswith(b"#!"):
        return False
    arguments = first_line[2:].strip().split()
    if not arguments:
        return False
    interpreter = arguments[0].replace(b"\\", b"/").rsplit(b"/", 1)[-1].lower()
    if interpreter == b"env":
        commands = [argument for argument in arguments[1:] if not argument.startswith(b"-")]
        if not commands:
            return False
        interpreter = commands[0].replace(b"\\", b"/").rsplit(b"/", 1)[-1].lower()
    return interpreter in SHELL_INTERPRETERS


def tracked_files(repo_root: Path) -> tuple[PurePosixPath, ...]:
    """Return non-submodule paths from the Git index."""

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

    paths = []
    for record in result.stdout.split(b"\0"):
        if not record:
            continue
        try:
            metadata, raw_path = record.split(b"\t", 1)
            mode = metadata.split(b" ", 1)[0].decode("ascii")
        except (UnicodeDecodeError, ValueError) as error:
            raise PolicyError("git ls-files returned malformed index data") from error
        if mode != GITLINK_MODE:
            paths.append(PurePosixPath(os.fsdecode(raw_path)))
    return tuple(paths)


def check_repository(repo_root: Path) -> tuple[PolicyViolation, ...]:
    repo_root = Path(repo_root).resolve()
    violations = []
    for path in tracked_files(repo_root):
        if path.suffix.casefold() in SCRIPT_SUFFIXES:
            violations.append(
                PolicyViolation(path, "tracked maintained command script is not allowed")
            )
            continue
        if path.suffix:
            continue
        try:
            with repo_root.joinpath(*path.parts).open("rb") as source:
                first_line = source.readline(256)
        except OSError as error:
            raise PolicyError(f"could not inspect {path.as_posix()}: {error}") from error
        if _has_shell_shebang(first_line):
            violations.append(
                PolicyViolation(path, "extensionless shell entry point is not allowed")
            )
    return tuple(sorted(violations, key=lambda violation: violation.path.as_posix()))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help=argparse.SUPPRESS)
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
