"""Audit cimgui's public API against dear-imgui-rs safe API decisions.

The audit has two independent layers:

* a reviewable snapshot of every public ``imgui:*`` generator declaration,
  including methods and overload signatures from all namespaces;
* semantic coverage decisions for top-level ``ImGui`` functions, backed by a
  rustdoc alias on a public safe Rust item or an explicit policy rationale.

Generator drift is expected during an upstream update, but it must be reviewed
and committed explicitly with ``--update-snapshot``. Input/schema failures use
exit code 2, while expected audit drift uses exit code 1.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib
from dataclasses import dataclass
from typing import Any, Iterable, Sequence


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFS_JSON = (
    REPO_ROOT
    / "dear-imgui-sys"
    / "third-party"
    / "cimgui"
    / "generator"
    / "output"
    / "definitions.json"
)
DEAR_IMGUI_SRC = REPO_ROOT / "dear-imgui" / "src"
MANIFEST_TOML = REPO_ROOT / "dear-imgui-sys" / "Cargo.toml"
POLICY_JSON = REPO_ROOT / "tools" / "api_surface_policy.json"
SNAPSHOT_JSON = REPO_ROOT / "tools" / "api_surface_snapshot.json"

POLICY_CLASSIFICATIONS = frozenset(
    {"intentional-sys-only", "unsafe-wrapper", "deferred-design"}
)
POLICY_KEYS = frozenset({"schema_version", "scope", "groups"})
POLICY_GROUP_KEYS = frozenset({"classification", "name", "reason", "functions"})
SNAPSHOT_KEYS = frozenset({"schema_version", "source_revisions", "declarations"})
REVISION_KEYS = frozenset({"cimgui", "imgui"})
DECLARATION_KEYS = frozenset(
    {
        "symbol",
        "cimgui_name",
        "namespace",
        "struct",
        "function",
        "c_arguments",
        "signature",
        "return_type",
        "arguments",
        "defaults",
        "traits",
    }
)
DECLARATION_TRAITS = (
    "constructor",
    "conv",
    "destructor",
    "is_static_function",
    "isvararg",
    "manual",
    "nonUDT",
    "realdestructor",
    "retref",
    "templated",
)
REVISION_RE = re.compile(r"[0-9a-f]{40}")


class InputError(ValueError):
    """A malformed or unreadable audit input."""


@dataclass(frozen=True)
class PublicDeclaration:
    symbol: str
    cimgui_name: str
    namespace: str
    struct_name: str
    funcname: str
    c_arguments: str
    signature: str
    return_type: str
    arguments: tuple[tuple[tuple[str, Any], ...], ...]
    defaults: tuple[tuple[str, Any], ...]
    traits: tuple[tuple[str, Any], ...]

    def snapshot_value(self) -> dict[str, Any]:
        return {
            "symbol": self.symbol,
            "cimgui_name": self.cimgui_name,
            "namespace": self.namespace,
            "struct": self.struct_name,
            "function": self.funcname,
            "c_arguments": self.c_arguments,
            "signature": self.signature,
            "return_type": self.return_type,
            "arguments": [dict(argument) for argument in self.arguments],
            "defaults": dict(self.defaults),
            "traits": dict(self.traits),
        }


@dataclass(frozen=True)
class PublicFuncGroup:
    funcname: str
    declarations: tuple[PublicDeclaration, ...]

    @property
    def symbols(self) -> tuple[str, ...]:
        return tuple(declaration.symbol for declaration in self.declarations)


@dataclass(frozen=True)
class PolicyDecision:
    classification: str
    group: str
    reason: str


@dataclass(frozen=True)
class SurfaceAudit:
    aliased: frozenset[str]
    policy_decided: frozenset[str]
    unexpected: frozenset[str]
    stale_policy: frozenset[str]


@dataclass(frozen=True)
class SnapshotDrift:
    revision_mismatches: tuple[str, ...]
    added: tuple[str, ...]
    removed: tuple[str, ...]
    changed: tuple[str, ...]

    def has_drift(self) -> bool:
        return bool(
            self.revision_mismatches or self.added or self.removed or self.changed
        )


@dataclass(frozen=True)
class _RustToken:
    kind: str
    value: str
    start: int
    end: int


def _read_json(path: pathlib.Path, description: str) -> Any:
    try:
        text = path.read_text(encoding="utf-8")
        return json.loads(text)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise InputError(f"failed to read {description} {path}: {error}") from error


def _exact_keys(value: dict[str, Any], expected: frozenset[str], context: str) -> None:
    actual = frozenset(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        details: list[str] = []
        if missing:
            details.append(f"missing keys: {', '.join(missing)}")
        if unknown:
            details.append(f"unknown keys: {', '.join(unknown)}")
        raise InputError(f"{context} has invalid keys ({'; '.join(details)})")


def _require_string(value: Any, context: str, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not allow_empty and not value):
        qualifier = "a string" if allow_empty else "a non-empty string"
        raise InputError(f"{context} must be {qualifier}")
    return value


def _normalize_cpp(value: str) -> str:
    value = re.sub(r"\s+", " ", value.strip())
    return re.sub(r"\s*([(),\[\]<>*&])\s*", r"\1", value)


def _canonical_value(value: Any, context: str) -> Any:
    if value is None or isinstance(value, (bool, int, float)):
        return value
    if isinstance(value, str):
        return _normalize_cpp(value)
    if isinstance(value, list):
        return [_canonical_value(item, f"{context}[]") for item in value]
    if isinstance(value, dict):
        if not all(isinstance(key, str) for key in value):
            raise InputError(f"{context} contains a non-string object key")
        return {
            key: _canonical_value(value[key], f"{context}.{key}")
            for key in sorted(value)
        }
    raise InputError(f"{context} contains unsupported JSON value {type(value).__name__}")


def _load_public_declarations(path: pathlib.Path) -> list[PublicDeclaration]:
    obj = _read_json(path, "cimgui definitions")
    if not isinstance(obj, dict) or not obj:
        raise InputError("cimgui definitions must be a non-empty object")

    declarations: list[PublicDeclaration] = []
    seen_symbols: set[str] = set()
    for family, overloads in obj.items():
        if not isinstance(family, str) or not isinstance(overloads, list):
            raise InputError("cimgui definitions must map string names to overload arrays")
        for index, record in enumerate(overloads):
            context = f"cimgui definition {family}[{index}]"
            if not isinstance(record, dict):
                raise InputError(f"{context} must be an object")
            location = _require_string(
                record.get("location"), f"{context}.location", allow_empty=True
            )
            if not location.startswith("imgui:"):
                continue

            required = {
                "args",
                "argsT",
                "cimguiname",
                "defaults",
                "location",
                "ov_cimguiname",
                "signature",
                "stname",
            }
            missing = sorted(required - set(record))
            if missing:
                raise InputError(f"{context} is missing keys: {', '.join(missing)}")

            symbol = _require_string(record["ov_cimguiname"], f"{context}.ov_cimguiname")
            if symbol in seen_symbols:
                raise InputError(f"duplicate public cimgui symbol {symbol!r}")
            seen_symbols.add(symbol)

            arguments_raw = record["argsT"]
            if not isinstance(arguments_raw, list):
                raise InputError(f"{context}.argsT must be an array")
            arguments: list[tuple[tuple[str, Any], ...]] = []
            for arg_index, argument in enumerate(arguments_raw):
                if not isinstance(argument, dict) or not all(
                    isinstance(key, str) for key in argument
                ):
                    raise InputError(f"{context}.argsT[{arg_index}] must be an object")
                canonical = _canonical_value(argument, f"{context}.argsT[{arg_index}]")
                arguments.append(tuple(canonical.items()))

            defaults_raw = record["defaults"]
            if not isinstance(defaults_raw, dict) or not all(
                isinstance(key, str) for key in defaults_raw
            ):
                raise InputError(f"{context}.defaults must be an object")
            defaults = _canonical_value(defaults_raw, f"{context}.defaults")
            traits = {
                key: _canonical_value(record[key], f"{context}.{key}")
                for key in DECLARATION_TRAITS
                if key in record
            }

            declarations.append(
                PublicDeclaration(
                    symbol=symbol,
                    cimgui_name=_require_string(
                        record["cimguiname"], f"{context}.cimguiname"
                    ),
                    namespace=_require_string(
                        record.get("namespace", ""),
                        f"{context}.namespace",
                        allow_empty=True,
                    ),
                    struct_name=_require_string(
                        record["stname"], f"{context}.stname", allow_empty=True
                    ),
                    funcname=_require_string(
                        record.get("funcname", ""),
                        f"{context}.funcname",
                        allow_empty=True,
                    ),
                    c_arguments=_normalize_cpp(
                        _require_string(record["args"], f"{context}.args")
                    ),
                    signature=_normalize_cpp(
                        _require_string(record["signature"], f"{context}.signature")
                    ),
                    return_type=_normalize_cpp(
                        _require_string(
                            record.get("ret", ""),
                            f"{context}.ret",
                            allow_empty=True,
                        )
                    ),
                    arguments=tuple(arguments),
                    defaults=tuple(defaults.items()),
                    traits=tuple(sorted(traits.items())),
                )
            )

    if not declarations:
        raise InputError("cimgui definitions contain no public imgui declarations")
    return sorted(declarations, key=lambda declaration: declaration.symbol)


def _load_source_revisions(path: pathlib.Path) -> dict[str, str]:
    try:
        with path.open("rb") as manifest_file:
            manifest = tomllib.load(manifest_file)
        metadata = manifest["package"]["metadata"]["dear-imgui-sources"]
        revisions = {
            "cimgui": metadata["cimgui-revision"],
            "imgui": metadata["imgui-revision"],
        }
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise InputError(f"failed to read source revisions from {path}: {error}") from error

    for name, revision in revisions.items():
        if not isinstance(revision, str) or REVISION_RE.fullmatch(revision) is None:
            raise InputError(f"{path} has invalid {name} revision {revision!r}")
    return revisions


def _snapshot_document(
    declarations: Sequence[PublicDeclaration], revisions: dict[str, str]
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "source_revisions": revisions,
        "declarations": [declaration.snapshot_value() for declaration in declarations],
    }


def _write_snapshot(
    path: pathlib.Path,
    declarations: Sequence[PublicDeclaration],
    revisions: dict[str, str],
) -> None:
    document = _snapshot_document(declarations, revisions)
    path.write_text(
        json.dumps(document, indent=2, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )


def _load_snapshot(path: pathlib.Path) -> tuple[dict[str, str], list[dict[str, Any]]]:
    obj = _read_json(path, "API surface snapshot")
    if not isinstance(obj, dict):
        raise InputError("API surface snapshot must be an object")
    _exact_keys(obj, SNAPSHOT_KEYS, "API surface snapshot")
    if isinstance(obj["schema_version"], bool) or obj["schema_version"] != 1:
        raise InputError("API surface snapshot must use integer schema_version 1")

    revisions = obj["source_revisions"]
    if not isinstance(revisions, dict):
        raise InputError("API surface snapshot source_revisions must be an object")
    _exact_keys(revisions, REVISION_KEYS, "API surface snapshot source_revisions")
    for name, revision in revisions.items():
        if not isinstance(revision, str) or REVISION_RE.fullmatch(revision) is None:
            raise InputError(f"API surface snapshot has invalid {name} revision")

    values = obj["declarations"]
    if not isinstance(values, list) or not values:
        raise InputError("API surface snapshot declarations must be a non-empty array")
    declarations: list[dict[str, Any]] = []
    seen_symbols: set[str] = set()
    for index, value in enumerate(values):
        if not isinstance(value, dict):
            raise InputError(f"API surface snapshot declaration {index} must be an object")
        _exact_keys(value, DECLARATION_KEYS, f"API surface snapshot declaration {index}")
        symbol = _require_string(value["symbol"], f"snapshot declaration {index}.symbol")
        if symbol in seen_symbols:
            raise InputError(f"API surface snapshot repeats symbol {symbol!r}")
        seen_symbols.add(symbol)
        for key in (
            "cimgui_name",
            "namespace",
            "struct",
            "function",
            "c_arguments",
            "signature",
            "return_type",
        ):
            _require_string(
                value[key],
                f"snapshot declaration {index}.{key}",
                allow_empty=key not in {"cimgui_name", "c_arguments", "signature"},
            )
        if not isinstance(value["arguments"], list) or not all(
            isinstance(argument, dict) for argument in value["arguments"]
        ):
            raise InputError(
                f"snapshot declaration {index}.arguments must be an array of objects"
            )
        if not isinstance(value["defaults"], dict) or not isinstance(value["traits"], dict):
            raise InputError(
                f"snapshot declaration {index}.defaults and traits must be objects"
            )
        _canonical_value(value["arguments"], f"snapshot declaration {index}.arguments")
        _canonical_value(value["defaults"], f"snapshot declaration {index}.defaults")
        _canonical_value(value["traits"], f"snapshot declaration {index}.traits")
        declarations.append(value)
    return dict(revisions), declarations


def _compare_snapshot(
    actual: Sequence[PublicDeclaration],
    actual_revisions: dict[str, str],
    expected_values: Sequence[dict[str, Any]],
    expected_revisions: dict[str, str],
) -> SnapshotDrift:
    actual_by_symbol = {
        declaration.symbol: declaration.snapshot_value() for declaration in actual
    }
    expected_by_symbol = {value["symbol"]: value for value in expected_values}
    actual_symbols = set(actual_by_symbol)
    expected_symbols = set(expected_by_symbol)
    revision_mismatches = tuple(
        f"{name}: audited={expected_revisions[name]} current={actual_revisions[name]}"
        for name in sorted(REVISION_KEYS)
        if expected_revisions[name] != actual_revisions[name]
    )
    return SnapshotDrift(
        revision_mismatches=revision_mismatches,
        added=tuple(sorted(actual_symbols - expected_symbols)),
        removed=tuple(sorted(expected_symbols - actual_symbols)),
        changed=tuple(
            symbol
            for symbol in sorted(actual_symbols & expected_symbols)
            if actual_by_symbol[symbol] != expected_by_symbol[symbol]
        ),
    )


def _iter_rs_files(root: pathlib.Path) -> Iterable[pathlib.Path]:
    yield from sorted(root.rglob("*.rs"))


def _consume_quoted(source: str, start: int, quote: str) -> int:
    index = start + 1
    while index < len(source):
        if source[index] == "\\":
            index += 2
            continue
        if source[index] == quote:
            return index + 1
        index += 1
    return len(source)


def _raw_string_end(source: str, start: int) -> int | None:
    index = start
    if source.startswith("br", index) or source.startswith("rb", index):
        index += 2
    elif source.startswith("r", index):
        index += 1
    else:
        return None
    hashes = 0
    while index < len(source) and source[index] == "#":
        hashes += 1
        index += 1
    if index >= len(source) or source[index] != '"':
        return None
    terminator = '"' + ("#" * hashes)
    end = source.find(terminator, index + 1)
    return len(source) if end < 0 else end + len(terminator)


def _decode_rust_string(literal: str) -> str:
    prefix_len = 1 if literal.startswith("b\"") else 0
    raw_start = prefix_len
    if literal.startswith(("r", "br", "rb")):
        marker = literal.find('"')
        hashes = marker - raw_start - 1
        if literal.startswith(("br", "rb")):
            hashes -= 1
        terminator_len = hashes + 1
        return literal[marker + 1 : -terminator_len]
    quoted = literal[prefix_len:]
    quoted = re.sub(r"\\\r?\n[ \t]*", "", quoted)
    try:
        return json.loads(quoted)
    except json.JSONDecodeError:
        # Token boundaries are what matter for source reachability. Rust has a
        # few escapes (for example ``\u{...}``) that JSON does not understand;
        # doc aliases in this crate use ordinary quoted or raw strings.
        return quoted[1:-1]


def _tokenize_rust(source: str) -> list[_RustToken]:
    tokens: list[_RustToken] = []
    index = 0
    while index < len(source):
        char = source[index]
        if char.isspace():
            index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            index = cursor
            continue

        raw_end = _raw_string_end(source, index)
        if raw_end is not None:
            literal = source[index:raw_end]
            tokens.append(_RustToken("string", _decode_rust_string(literal), index, raw_end))
            index = raw_end
            continue
        if char == '"' or (char == "b" and index + 1 < len(source) and source[index + 1] == '"'):
            quote_start = index + 1 if char == "b" else index
            end = _consume_quoted(source, quote_start, '"')
            literal = source[index:end]
            tokens.append(_RustToken("string", _decode_rust_string(literal), index, end))
            index = end
            continue
        if char == "'" and index + 2 < len(source):
            is_simple_char = source[index + 2] == "'"
            is_escaped_char = source[index + 1] == "\\"
            if is_simple_char or is_escaped_char:
                end = _consume_quoted(source, index, "'")
                tokens.append(_RustToken("char", "", index, end))
                index = end
                continue
        if char.isalpha() or char == "_":
            end = index + 1
            while end < len(source) and (source[end].isalnum() or source[end] == "_"):
                end += 1
            tokens.append(_RustToken("ident", source[index:end], index, end))
            index = end
            continue
        tokens.append(_RustToken("punct", char, index, index + 1))
        index += 1
    return tokens


def _matching_braces(tokens: Sequence[_RustToken]) -> dict[int, int]:
    stack: list[int] = []
    matches: dict[int, int] = {}
    for index, token in enumerate(tokens):
        if token.value == "{":
            stack.append(index)
        elif token.value == "}" and stack:
            opening = stack.pop()
            matches[opening] = index
    return matches


def _attribute_end(tokens: Sequence[_RustToken], start: int) -> int | None:
    if start + 1 >= len(tokens) or tokens[start].value != "#" or tokens[start + 1].value != "[":
        return None
    depth = 1
    index = start + 2
    while index < len(tokens):
        if tokens[index].value == "[":
            depth += 1
        elif tokens[index].value == "]":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return None


def _is_cfg_test(attribute: Sequence[_RustToken]) -> bool:
    values = [token.value for token in attribute]
    return values == ["cfg", "(", "test", ")"]


def _is_doc_hidden(attribute: Sequence[_RustToken]) -> bool:
    values = [token.value for token in attribute]
    return values == ["doc", "(", "hidden", ")"]


def _doc_aliases(attribute: Sequence[_RustToken]) -> set[str]:
    if not attribute or attribute[0].value != "doc":
        return set()
    aliases: set[str] = set()
    for index in range(1, len(attribute) - 2):
        if (
            attribute[index].value == "alias"
            and attribute[index + 1].value == "="
            and attribute[index + 2].kind == "string"
        ):
            aliases.add(attribute[index + 2].value)
    return aliases


def _private_module_ranges(tokens: Sequence[_RustToken]) -> list[tuple[int, int]]:
    brace_matches = _matching_braces(tokens)
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
        public = any(
            header[pos].value == "pub"
            and (pos + 1 >= len(header) or header[pos + 1].value != "(")
            for pos in range(len(header))
        )
        cfg_test = False
        attr_index = 0
        while attr_index < len(header):
            end = _attribute_end(header, attr_index)
            if end is None:
                attr_index += 1
                continue
            cfg_test |= _is_cfg_test(header[attr_index + 2 : end - 1])
            attr_index = end
        if not public or cfg_test:
            ranges.append((tokens[cursor].start, tokens[brace_matches[cursor]].end))
    return ranges


def _inside_ranges(offset: int, ranges: Sequence[tuple[int, int]]) -> bool:
    return any(start <= offset < end for start, end in ranges)


def _collect_doc_aliases(rs_files: Iterable[pathlib.Path]) -> set[str]:
    aliases: set[str] = set()
    item_keywords = {"const", "enum", "fn", "macro", "mod", "static", "struct", "trait", "type", "union", "use"}
    for path in rs_files:
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            raise InputError(f"failed to read Rust source {path}: {error}") from error
        tokens = _tokenize_rust(source)
        private_ranges = _private_module_ranges(tokens)
        index = 0
        while index < len(tokens):
            first_end = _attribute_end(tokens, index)
            if first_end is None:
                index += 1
                continue

            candidate_aliases: set[str] = set()
            cfg_test = False
            hidden = False
            cursor = index
            while True:
                end = _attribute_end(tokens, cursor)
                if end is None:
                    break
                attribute = tokens[cursor + 2 : end - 1]
                candidate_aliases.update(_doc_aliases(attribute))
                cfg_test |= _is_cfg_test(attribute)
                hidden |= _is_doc_hidden(attribute)
                cursor = end

            if not candidate_aliases:
                index = first_end
                continue
            if cfg_test or hidden or _inside_ranges(tokens[index].start, private_ranges):
                index = cursor
                continue
            if cursor >= len(tokens) or tokens[cursor].value != "pub":
                index = cursor
                continue
            cursor += 1
            if cursor < len(tokens) and tokens[cursor].value == "(":
                index = cursor
                continue

            modifiers: set[str] = set()
            while cursor < len(tokens) and tokens[cursor].value not in item_keywords:
                modifiers.add(tokens[cursor].value)
                cursor += 1
            if (
                cursor < len(tokens)
                and tokens[cursor].value in item_keywords
                and "unsafe" not in modifiers
            ):
                aliases.update(candidate_aliases)
            index = cursor + 1
    return aliases


def _collect_sys_usages(rs_files: Iterable[pathlib.Path]) -> set[str]:
    used: set[str] = set()
    pattern = re.compile(r"\b(?:crate::)?sys::(ig[A-Za-z0-9_]+)\b")
    for path in rs_files:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            raise InputError(f"failed to read Rust source {path}: {error}") from error
        used.update(pattern.findall(text))
    return used


def _group_top_level_imgui(
    declarations: Sequence[PublicDeclaration],
) -> dict[str, PublicFuncGroup]:
    grouped: dict[str, list[PublicDeclaration]] = {}
    for declaration in declarations:
        if declaration.namespace == "ImGui" and declaration.funcname:
            grouped.setdefault(declaration.funcname, []).append(declaration)
    return {
        funcname: PublicFuncGroup(
            funcname,
            tuple(sorted(values, key=lambda declaration: declaration.symbol)),
        )
        for funcname, values in sorted(grouped.items())
    }


def _load_policy(path: pathlib.Path) -> dict[str, PolicyDecision]:
    obj = _read_json(path, "API surface policy")
    if not isinstance(obj, dict):
        raise InputError("API surface policy must be an object")
    _exact_keys(obj, POLICY_KEYS, "API surface policy")
    if isinstance(obj["schema_version"], bool) or obj["schema_version"] != 2:
        raise InputError("API surface policy must use integer schema_version 2")
    if obj["scope"] != "ImGui":
        raise InputError("API surface policy scope must be 'ImGui'")
    groups = obj["groups"]
    if not isinstance(groups, list):
        raise InputError("API surface policy groups must be an array")

    decisions: dict[str, PolicyDecision] = {}
    group_names: set[str] = set()
    for index, group in enumerate(groups):
        context = f"API surface policy group {index}"
        if not isinstance(group, dict):
            raise InputError(f"{context} must be an object")
        _exact_keys(group, POLICY_GROUP_KEYS, context)
        classification = _require_string(group["classification"], f"{context}.classification")
        name = _require_string(group["name"], f"{context}.name")
        reason = _require_string(group["reason"], f"{context}.reason")
        if classification not in POLICY_CLASSIFICATIONS:
            expected = ", ".join(sorted(POLICY_CLASSIFICATIONS))
            raise InputError(
                f"{context} has classification {classification!r}; expected one of: {expected}"
            )
        if name in group_names:
            raise InputError(f"API surface policy repeats group name {name!r}")
        group_names.add(name)
        functions = group["functions"]
        if not isinstance(functions, list) or not functions:
            raise InputError(f"{context}.functions must be a non-empty array")
        if not all(isinstance(function, str) and function for function in functions):
            raise InputError(f"{context}.functions must contain non-empty strings")
        if len(functions) != len(set(functions)):
            raise InputError(f"{context}.functions contains duplicates")
        for function in functions:
            if function in decisions:
                previous = decisions[function]
                raise InputError(
                    f"API surface policy lists {function!r} in both "
                    f"{previous.group!r} and {name!r}"
                )
            decisions[function] = PolicyDecision(classification, name, reason)
    return decisions


def _audit_surface(
    groups: dict[str, PublicFuncGroup],
    aliases: set[str],
    policy: dict[str, PolicyDecision],
) -> SurfaceAudit:
    public = set(groups)
    aliased = public & aliases
    unaliased = public - aliased
    policy_names = set(policy)
    return SurfaceAudit(
        aliased=frozenset(aliased),
        policy_decided=frozenset(unaliased & policy_names),
        unexpected=frozenset(unaliased - policy_names),
        stale_policy=frozenset(policy_names - unaliased),
    )


def _sys_symbol(group: PublicFuncGroup) -> str:
    symbol = group.symbols[0] if group.symbols else "?"
    if len(group.symbols) > 1:
        symbol = f"{symbol} (+{len(group.symbols) - 1})"
    return symbol


def _print_table(rows: list[tuple[PublicFuncGroup, PolicyDecision | None]]) -> None:
    print("| ImGui func | sys symbol | classification | policy group |")
    print("|---|---|---|---|")
    for group, decision in rows:
        classification = decision.classification if decision else "unexpected"
        policy_group = decision.group if decision else "-"
        print(
            f"| `{group.funcname}` | `{_sys_symbol(group)}` | `{classification}` | "
            f"`{policy_group}` |"
        )


def _print_snapshot_drift(drift: SnapshotDrift) -> None:
    if drift.revision_mismatches:
        print("API surface snapshot source revision drift:", file=sys.stderr)
        for mismatch in drift.revision_mismatches:
            print(f"- {mismatch}", file=sys.stderr)
    for label, symbols in (
        ("added", drift.added),
        ("removed", drift.removed),
        ("changed", drift.changed),
    ):
        if symbols:
            print(f"API surface snapshot {label} declarations:", file=sys.stderr)
            for symbol in symbols:
                print(f"- {symbol}", file=sys.stderr)


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--format", choices=["md", "plain"], default="plain")
    parser.add_argument("--limit", type=int, default=200)
    parser.add_argument("--definitions", type=pathlib.Path, default=DEFS_JSON)
    parser.add_argument("--rust-source", type=pathlib.Path, default=DEAR_IMGUI_SRC)
    parser.add_argument("--manifest", type=pathlib.Path, default=MANIFEST_TOML)
    parser.add_argument("--policy", type=pathlib.Path, default=POLICY_JSON)
    parser.add_argument("--snapshot", type=pathlib.Path, default=SNAPSHOT_JSON)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail on generator drift or an unreviewed top-level ImGui function",
    )
    parser.add_argument(
        "--update-snapshot",
        action="store_true",
        help="write the current reviewed generator surface and source revisions",
    )
    args = parser.parse_args(argv)
    if args.check and args.update_snapshot:
        parser.error("--check and --update-snapshot cannot be used together")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        declarations = _load_public_declarations(args.definitions)
        revisions = _load_source_revisions(args.manifest)
        if args.update_snapshot:
            _write_snapshot(args.snapshot, declarations, revisions)
            print(
                f"Updated {args.snapshot} with {len(declarations)} public declarations."
            )
            return 0

        expected_revisions, expected_declarations = _load_snapshot(args.snapshot)
        drift = _compare_snapshot(
            declarations,
            revisions,
            expected_declarations,
            expected_revisions,
        )
        groups = _group_top_level_imgui(declarations)
        rs_files = list(_iter_rs_files(args.rust_source))
        aliases = _collect_doc_aliases(rs_files)
        policy = _load_policy(args.policy)
        audit = _audit_surface(groups, aliases, policy)
    except InputError as error:
        print(f"API surface input error: {error}", file=sys.stderr)
        return 2

    unaliased = [groups[name] for name in sorted(set(groups) - set(audit.aliased))]
    shown = unaliased[: max(0, args.limit)]

    if args.check:
        print(f"Public generator declarations (all namespaces): {len(declarations)}")
        print(f"Top-level ImGui funcnames: {len(groups)}")
        print(f"Covered via public safe rustdoc aliases: {len(audit.aliased)}")
        print(f"Classified via explicit policy: {len(audit.policy_decided)}")
        if drift.has_drift():
            _print_snapshot_drift(drift)
        if audit.unexpected:
            print("Unexpected missing safe API decisions:", file=sys.stderr)
            for name in sorted(audit.unexpected):
                print(f"- {name}", file=sys.stderr)
        if audit.stale_policy:
            print("Stale API surface policy entries:", file=sys.stderr)
            for name in sorted(audit.stale_policy):
                print(f"- {name}", file=sys.stderr)
        if drift.has_drift() or audit.unexpected or audit.stale_policy:
            return 1
        print("API surface policy and generator snapshot checks passed.")
        return 0

    if args.format == "md":
        _print_table([(group, policy.get(group.funcname)) for group in shown])
        return 0

    used_sys = _collect_sys_usages(rs_files)
    covered_by_sys = sum(
        bool(set(group.symbols) & used_sys) for group in groups.values()
    )
    print(f"Repo root: {REPO_ROOT}")
    print(f"Public generator declarations (all namespaces): {len(declarations)}")
    print(f"Top-level ImGui funcnames: {len(groups)}")
    print(f"Covered via public safe rustdoc aliases: {len(audit.aliased)}")
    print(f"Classified via explicit policy: {len(audit.policy_decided)}")
    print(f"Referenced via sys usage (informational): {covered_by_sys}")
    print(f"Generator snapshot drift: {drift.has_drift()}")
    print(f"Unexpected missing decisions: {len(audit.unexpected)}")
    print(f"Stale policy entries: {len(audit.stale_policy)}")
    print(f"Unaliased policy report: {len(unaliased)} (showing at most {args.limit})")
    for group in shown:
        decision = policy.get(group.funcname)
        disposition = (
            "unexpected"
            if decision is None
            else f"{decision.classification}/{decision.group}"
        )
        print(
            f"- {group.funcname}  sys={_sys_symbol(group)}  policy={disposition}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
