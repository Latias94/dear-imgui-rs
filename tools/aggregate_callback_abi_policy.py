"""Reject Rust callback items that expose Dear ImGui aggregates by value over C ABI."""

from __future__ import annotations

import argparse
import bisect
import pathlib
import sys
from dataclasses import dataclass
from typing import Iterable, Mapping, Sequence

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


def _matching_group(
    tokens: Sequence[api_surface_report._RustToken], opening: int
) -> int | None:
    closing_for = {"(": ")", "[": "]", "<": ">"}
    opening_value = tokens[opening].value
    closing_value = closing_for.get(opening_value)
    if closing_value is None:
        return None

    depth = 0
    for index in range(opening, len(tokens)):
        value = tokens[index].value
        if value == opening_value:
            depth += 1
        elif value == closing_value:
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
    aliases: Mapping[str, Sequence[Sequence[api_surface_report._RustToken]]],
    resolving: frozenset[str] = frozenset(),
) -> bool:
    while (
        len(type_tokens) >= 2
        and type_tokens[0].value == "("
        and _matching_parenthesis(type_tokens, 0) == len(type_tokens) - 1
    ):
        type_tokens = type_tokens[1:-1]

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
        elif depth == 0:
            if token.value in AGGREGATE_TYPES:
                return True
            if token.value not in resolving:
                for alias in aliases.get(token.value, ()):
                    if _contains_by_value_aggregate(
                        alias,
                        aliases,
                        resolving | {token.value},
                    ):
                        return True

    index = 0
    while index < len(type_tokens):
        if type_tokens[index].value not in {"(", "[", "<"}:
            index += 1
            continue
        closing_index = _matching_group(type_tokens, index)
        if closing_index is None:
            index += 1
            continue
        for segment in _top_level_segments(type_tokens[index + 1 : closing_index]):
            if _contains_by_value_aggregate(segment, aliases, resolving):
                return True
        index = closing_index + 1
    return False


def _signature_has_by_value_aggregate(
    tokens: Sequence[api_surface_report._RustToken],
    opening: int,
    closing: int,
    aliases: Mapping[str, Sequence[Sequence[api_surface_report._RustToken]]],
) -> bool:
    for parameter in _top_level_segments(tokens[opening + 1 : closing]):
        colon = next(
            (index for index, token in enumerate(parameter) if token.value == ":"),
            None,
        )
        if colon is not None and _contains_by_value_aggregate(
            parameter[colon + 1 :], aliases
        ):
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
        if _contains_by_value_aggregate(tokens[cursor:end], aliases):
            return True
    return False


def _aggregate_type_aliases(
    tokens: Sequence[api_surface_report._RustToken],
    ignored_ranges: Sequence[tuple[int, int]],
) -> dict[str, list[Sequence[api_surface_report._RustToken]]]:
    """Collect type/import aliases for conservative workspace-wide resolution."""

    aliases: dict[str, list[Sequence[api_surface_report._RustToken]]] = {}

    def add_alias(name: str, source: Sequence[api_surface_report._RustToken]) -> None:
        aliases.setdefault(name, []).append(source)

    for index, token in enumerate(tokens):
        if context_binding_policy._inside_ranges(token.start, ignored_ranges):
            continue

        if token.value == "type" and index + 1 < len(tokens):
            name = tokens[index + 1]
            if name.kind != "ident":
                continue
            equals = index + 2
            while equals < len(tokens) and tokens[equals].value not in {"=", ";"}:
                equals += 1
            if equals >= len(tokens) or tokens[equals].value != "=":
                continue
            end = equals + 1
            while end < len(tokens) and tokens[end].value != ";":
                end += 1
            add_alias(name.value, tokens[equals + 1 : end])
            continue

        if token.value != "use":
            continue
        end = index + 1
        while end < len(tokens) and tokens[end].value != ";":
            end += 1
        statement = tokens[index + 1 : end]
        for alias_index, alias_token in enumerate(statement[:-1]):
            if alias_token.value != "as" or statement[alias_index + 1].kind != "ident":
                continue
            source_start = alias_index
            while source_start > 0 and statement[source_start - 1].value not in {"{", ","}:
                source_start -= 1
            source = statement[source_start:alias_index]
            add_alias(statement[alias_index + 1].value, source)
    return aliases


def audit_sources(
    repo_root: pathlib.Path,
    roots: Iterable[pathlib.Path],
) -> tuple[AggregateCallbackViolation, ...]:
    violations: list[AggregateCallbackViolation] = []
    resolved_root = repo_root.resolve()
    source_records = []
    aliases: dict[str, list[Sequence[api_surface_report._RustToken]]] = {}

    for path in context_binding_policy._iter_rust_sources(roots):
        source = path.read_text(encoding="utf-8")
        tokens = api_surface_report._tokenize_rust(source)
        standalone_test = context_binding_policy._is_standalone_test_source(path)
        test_ranges = (
            ()
            if standalone_test
            else context_binding_policy._cfg_test_module_ranges(tokens)
        )
        local_aliases = _aggregate_type_aliases(tokens, test_ranges)
        for name, definitions in local_aliases.items():
            aliases.setdefault(name, []).extend(definitions)
        line_starts = [0]
        line_starts.extend(index + 1 for index, char in enumerate(source) if char == "\n")
        relative = path.resolve().relative_to(resolved_root).as_posix()
        source_records.append((tokens, standalone_test, test_ranges, line_starts, relative))

    for tokens, standalone_test, test_ranges, line_starts, relative in source_records:
        for index, token in enumerate(tokens):
            if token.value != "fn" or index < 2:
                continue
            if not (
                tokens[index - 2].value == "extern"
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
                tokens, opening, closing, aliases
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
