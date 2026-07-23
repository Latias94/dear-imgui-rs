"""Reject Rust callback items that expose Dear ImGui aggregates by value over C ABI."""

from __future__ import annotations

import argparse
import bisect
import pathlib
import sys
from dataclasses import dataclass
from typing import Iterable, Sequence

import api_surface_report
import context_binding_policy


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
AGGREGATE_TYPES = frozenset({"ImVec2", "ImVec2_c", "ImVec4", "ImVec4_c"})


@dataclass(frozen=True)
class AggregateCallbackViolation:
    path: str
    line: int
    function: str


def _matching_parenthesis(
    tokens: Sequence[api_surface_report._RustToken], opening: int
) -> int | None:
    depth = 0
    for index in range(opening, len(tokens)):
        value = tokens[index].value
        if value == "(":
            depth += 1
        elif value == ")":
            depth -= 1
            if depth == 0:
                return index
    return None


def _top_level_segments(
    tokens: Sequence[api_surface_report._RustToken],
) -> Iterable[Sequence[api_surface_report._RustToken]]:
    opening = {"(", "[", "{", "<"}
    closing = {")", "]", "}", ">"}
    depth = 0
    start = 0
    for index, token in enumerate(tokens):
        if token.value in opening:
            depth += 1
        elif token.value in closing:
            depth = max(0, depth - 1)
        elif token.value == "," and depth == 0:
            yield tokens[start:index]
            start = index + 1
    yield tokens[start:]


def _contains_by_value_aggregate(
    type_tokens: Sequence[api_surface_report._RustToken],
) -> bool:
    first_type_token = next(
        (token.value for token in type_tokens if token.value not in {"mut", "const"}),
        None,
    )
    if first_type_token in {"*", "&"}:
        return False

    opening = {"(", "[", "{", "<"}
    closing = {")", "]", "}", ">"}
    depth = 0
    for token in type_tokens:
        if token.value in opening:
            depth += 1
        elif token.value in closing:
            depth = max(0, depth - 1)
        elif token.value in AGGREGATE_TYPES and depth == 0:
            return True
    return False


def _signature_has_by_value_aggregate(
    tokens: Sequence[api_surface_report._RustToken],
    opening: int,
    closing: int,
) -> bool:
    for parameter in _top_level_segments(tokens[opening + 1 : closing]):
        colon = next(
            (index for index, token in enumerate(parameter) if token.value == ":"),
            None,
        )
        if colon is not None and _contains_by_value_aggregate(parameter[colon + 1 :]):
            return True

    cursor = closing + 1
    if (
        cursor + 1 < len(tokens)
        and tokens[cursor].value == "-"
        and tokens[cursor + 1].value == ">"
    ):
        cursor += 2
        end = cursor
        while end < len(tokens) and tokens[end].value not in {"{", ";", "where"}:
            end += 1
        if _contains_by_value_aggregate(tokens[cursor:end]):
            return True
    return False


def audit_sources(
    repo_root: pathlib.Path,
    roots: Iterable[pathlib.Path],
) -> tuple[AggregateCallbackViolation, ...]:
    violations: list[AggregateCallbackViolation] = []
    resolved_root = repo_root.resolve()

    for path in context_binding_policy._iter_rust_sources(roots):
        source = path.read_text(encoding="utf-8")
        tokens = api_surface_report._tokenize_rust(source)
        standalone_test = context_binding_policy._is_standalone_test_source(path)
        test_ranges = (
            ()
            if standalone_test
            else context_binding_policy._cfg_test_module_ranges(tokens)
        )
        line_starts = [0]
        line_starts.extend(index + 1 for index, char in enumerate(source) if char == "\n")
        relative = path.resolve().relative_to(resolved_root).as_posix()

        for index, token in enumerate(tokens):
            if token.value != "fn" or index < 3:
                continue
            if not (
                tokens[index - 3].value == "unsafe"
                and tokens[index - 2].value == "extern"
                and tokens[index - 1].kind == "string"
                and tokens[index - 1].value in {"C", "C-unwind"}
            ):
                continue
            if standalone_test or context_binding_policy._inside_ranges(
                token.start, test_ranges
            ):
                continue
            if index + 1 >= len(tokens) or tokens[index + 1].kind != "ident":
                # Function-pointer types use `fn(` directly. The policy governs callback
                # implementations that can be installed into native C++ slots.
                continue

            opening = index + 1
            while opening < len(tokens) and tokens[opening].value not in {"(", ";", "{"}:
                opening += 1
            if opening >= len(tokens) or tokens[opening].value != "(":
                continue
            closing = _matching_parenthesis(tokens, opening)
            if closing is None or not _signature_has_by_value_aggregate(
                tokens, opening, closing
            ):
                continue

            function = tokens[index + 1].value if index + 1 < len(tokens) else "<unknown>"
            violations.append(
                AggregateCallbackViolation(
                    path=relative,
                    line=bisect.bisect_right(line_starts, token.start),
                    function=function,
                )
            )

    return tuple(violations)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=pathlib.Path, default=REPO_ROOT)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    repo_root = args.repo_root.resolve()
    violations = audit_sources(
        repo_root,
        context_binding_policy._safe_source_roots(repo_root),
    )
    if not violations:
        print("Aggregate callback ABI source policy passed")
        return 0

    for violation in violations:
        print(
            f"{violation.path}:{violation.line}: {violation.function} passes or returns "
            "ImVec2/ImVec4 by value across extern C; use a C++ pointer/out-parameter bridge",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
