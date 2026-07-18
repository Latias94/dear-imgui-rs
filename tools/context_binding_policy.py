"""Reject direct Dear ImGui Context switching in maintained safe Rust sources."""

from __future__ import annotations

import argparse
import bisect
import os
import pathlib
import sys
from dataclasses import dataclass
from typing import Iterable, Sequence

import api_surface_report


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
FORBIDDEN_IDENT = "igSetCurrentContext"
IGNORED_DIRECTORY_NAMES = frozenset({".git", "node_modules", "target"})

# Test Engine migrates to ContextBinding in U8, after its attachment state model exists.
PRODUCTION_ALLOW_COUNTS = {
    "dear-imgui/src/context/binding.rs": 2,
    "extensions/dear-imgui-test-engine/src/engine.rs": 2,
}


@dataclass(frozen=True)
class DirectContextSwitch:
    path: str
    line: int


@dataclass(frozen=True)
class PolicyAudit:
    unexpected: tuple[DirectContextSwitch, ...]
    allow_count_mismatches: tuple[str, ...]

    def passed(self) -> bool:
        return not self.unexpected and not self.allow_count_mismatches


def _safe_source_roots(repo_root: pathlib.Path) -> tuple[pathlib.Path, ...]:
    roots = [repo_root / "dear-imgui" / "src", repo_root / "dear-app" / "src"]
    for parent in (repo_root / "backends", repo_root / "extensions"):
        if not parent.is_dir():
            continue
        for crate in sorted(parent.iterdir()):
            source = crate / "src"
            if source.is_dir() and not crate.name.endswith("-sys"):
                roots.append(source)
    roots.extend(
        path
        for path in (
            repo_root / "examples",
            repo_root / "examples-android",
            repo_root / "examples-ios",
            repo_root / "examples-wasm",
        )
        if path.is_dir()
    )
    return tuple(roots)


def _iter_rust_sources(roots: Iterable[pathlib.Path]) -> Iterable[pathlib.Path]:
    seen: set[pathlib.Path] = set()
    for root in roots:
        if any(part in IGNORED_DIRECTORY_NAMES for part in root.parts):
            continue
        for directory, directory_names, file_names in os.walk(root):
            directory_names[:] = sorted(
                name
                for name in directory_names
                if name not in IGNORED_DIRECTORY_NAMES
            )
            for file_name in sorted(file_names):
                if not file_name.endswith(".rs"):
                    continue
                path = pathlib.Path(directory, file_name)
                resolved = path.resolve()
                if resolved not in seen:
                    seen.add(resolved)
                    yield path


def _is_standalone_test_source(path: pathlib.Path) -> bool:
    return path.name in {"test.rs", "tests.rs"} or "tests" in path.parts


def _cfg_test_module_ranges(
    tokens: Sequence[api_surface_report._RustToken],
) -> tuple[tuple[int, int], ...]:
    brace_matches = api_surface_report._matching_braces(tokens)
    ranges: list[tuple[int, int]] = []
    for index, token in enumerate(tokens):
        if token.value != "mod":
            continue
        cursor = index + 1
        while cursor < len(tokens) and tokens[cursor].value not in {"{", ";"}:
            cursor += 1
        if cursor >= len(tokens) or tokens[cursor].value != "{" or cursor not in brace_matches:
            continue

        header_start = index - 1
        while header_start >= 0 and tokens[header_start].value not in {";", "{", "}"}:
            header_start -= 1
        header = tokens[header_start + 1 : index]
        cfg_test = False
        attr_index = 0
        while attr_index < len(header):
            end = api_surface_report._attribute_end(header, attr_index)
            if end is None:
                attr_index += 1
                continue
            cfg_test |= api_surface_report._is_cfg_test(
                header[attr_index + 2 : end - 1]
            )
            attr_index = end
        if cfg_test:
            ranges.append(
                (tokens[cursor].start, tokens[brace_matches[cursor]].end)
            )
    return tuple(ranges)


def _inside_ranges(offset: int, ranges: Sequence[tuple[int, int]]) -> bool:
    return any(start <= offset < end for start, end in ranges)


def _line_number(line_starts: Sequence[int], offset: int) -> int:
    return bisect.bisect_right(line_starts, offset)


def audit_sources(
    repo_root: pathlib.Path,
    roots: Iterable[pathlib.Path],
    allow_counts: dict[str, int],
) -> PolicyAudit:
    unexpected: list[DirectContextSwitch] = []
    observed_allowed = {path: 0 for path in allow_counts}

    for path in _iter_rust_sources(roots):
        source = path.read_text(encoding="utf-8")
        tokens = api_surface_report._tokenize_rust(source)
        test_ranges = () if _is_standalone_test_source(path) else _cfg_test_module_ranges(tokens)
        line_starts = [0]
        line_starts.extend(index + 1 for index, char in enumerate(source) if char == "\n")
        relative = path.resolve().relative_to(repo_root.resolve()).as_posix()

        for token in tokens:
            if token.kind != "ident" or token.value != FORBIDDEN_IDENT:
                continue
            if _is_standalone_test_source(path) or _inside_ranges(token.start, test_ranges):
                continue
            if relative in observed_allowed:
                observed_allowed[relative] += 1
                continue
            unexpected.append(
                DirectContextSwitch(
                    path=relative,
                    line=_line_number(line_starts, token.start),
                )
            )

    mismatches = tuple(
        f"{path}: expected {expected}, observed {observed_allowed[path]}"
        for path, expected in sorted(allow_counts.items())
        if observed_allowed[path] != expected
    )
    return PolicyAudit(tuple(unexpected), mismatches)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=pathlib.Path, default=REPO_ROOT)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    repo_root = args.repo_root.resolve()
    audit = audit_sources(
        repo_root,
        _safe_source_roots(repo_root),
        PRODUCTION_ALLOW_COUNTS,
    )
    if audit.passed():
        print("Context binding source policy passed")
        return 0

    for switch in audit.unexpected:
        print(
            f"{switch.path}:{switch.line}: direct {FORBIDDEN_IDENT} bypasses ContextBinding",
            file=sys.stderr,
        )
    for mismatch in audit.allow_count_mismatches:
        print(f"stale Context binding allowlist: {mismatch}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
