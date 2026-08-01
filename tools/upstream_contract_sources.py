"""Source adapters for the upstream API contract audit."""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
from typing import Any, Mapping, Sequence

from upstream_contract_model import (
    FACT_KINDS,
    SHA1_RE,
    ApiFact,
    ApiSnapshot,
    ContractInputError,
    JsonReader,
    json_array,
    json_mapping,
    read_json,
    reject_duplicate_keys,
    string,
)


_RUST_IDENTIFIER_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
_RUST_STRUCT_RE = re.compile(r"pub struct ([A-Za-z_][A-Za-z0-9_]*) \{\Z")
_RUST_CONST_RE = re.compile(
    r"pub const ([A-Za-z_][A-Za-z0-9_]*): (.+) = (.+);\Z", re.DOTALL
)
_RUST_TYPE_RE = re.compile(r"pub type ([A-Za-z_][A-Za-z0-9_]*) = (.+);\Z", re.DOTALL)
_RUST_FUNCTION_RE = re.compile(
    r"pub fn ([A-Za-z_][A-Za-z0-9_]*)\((.*)\)(?: -> (.+))?\Z", re.DOTALL
)


def _public_location(location: Any, api_locations: Sequence[str]) -> bool:
    if not isinstance(location, str):
        return False
    return any(location == prefix or location.startswith(f"{prefix}:") for prefix in api_locations)


def _load_optional_generator_json(output_root: pathlib.Path, name: str, default: Any) -> Any:
    path = output_root / name
    return read_json(path) if path.is_file() else default


def _deduplicate_facts(facts: Sequence[ApiFact], context: str) -> tuple[ApiFact, ...]:
    by_id: dict[str, ApiFact] = {}
    for fact in facts:
        if fact.kind not in FACT_KINDS:
            raise ContractInputError(f"unsupported fact kind {fact.kind!r}")
        previous = by_id.get(fact.id)
        if previous is not None:
            raise ContractInputError(
                f"{context} defines duplicate fact {fact.id!r}: "
                f"{previous.value!r} and {fact.value!r}"
            )
        by_id[fact.id] = fact
    return tuple(sorted(by_id.values(), key=lambda item: item.id))


def collect_generator_facts(
    source_id: str,
    output_root: pathlib.Path,
    api_locations: Sequence[str],
    *,
    json_reader: JsonReader | None = None,
) -> tuple[ApiFact, ...]:
    """Collect stable public facts from one cimgui-style generator output."""

    if not api_locations:
        raise ContractInputError(f"{source_id} has no generator API locations to audit")
    reader = json_reader or read_json

    def read_required(name: str) -> Any:
        path = output_root / name
        try:
            return reader(path)
        except ContractInputError:
            raise
        except (OSError, json.JSONDecodeError) as error:
            raise ContractInputError(f"could not read {path}: {error}") from error

    definitions = json_mapping(read_required("definitions.json"), "definitions.json")
    structs_and_enums = json_mapping(
        read_required("structs_and_enums.json"), "structs_and_enums.json"
    )
    if json_reader is None:
        constants = _load_optional_generator_json(output_root, "constants.json", {})
        typedefs = _load_optional_generator_json(output_root, "typedefs_dict.json", {})
    else:
        try:
            constants = reader(output_root / "constants.json")
        except (FileNotFoundError, KeyError):
            constants = {}
        try:
            typedefs = reader(output_root / "typedefs_dict.json")
        except (FileNotFoundError, KeyError):
            typedefs = {}
    constants = json_mapping(constants, "constants.json")
    typedefs = json_mapping(typedefs, "typedefs_dict.json")

    required_struct_keys = {"enums", "enumtypes", "locations", "structs"}
    missing_struct_keys = required_struct_keys - set(structs_and_enums)
    if missing_struct_keys:
        raise ContractInputError(
            "structs_and_enums.json is missing " + ", ".join(sorted(missing_struct_keys))
        )
    enums = json_mapping(structs_and_enums["enums"], "structs_and_enums.enums")
    raw_enum_types = structs_and_enums["enumtypes"]
    enum_types = (
        {}
        if raw_enum_types == []
        else json_mapping(raw_enum_types, "structs_and_enums.enumtypes")
    )
    locations = json_mapping(
        structs_and_enums["locations"], "structs_and_enums.locations"
    )
    structs = json_mapping(structs_and_enums["structs"], "structs_and_enums.structs")

    facts: list[ApiFact] = []
    function_traits = (
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
    for declaration_name, raw_declarations in definitions.items():
        for index, declaration in enumerate(
            json_array(raw_declarations, f"definitions.{declaration_name}")
        ):
            record = json_mapping(
                declaration, f"definitions.{declaration_name}[{index}]"
            )
            if not _public_location(record.get("location"), api_locations):
                continue
            symbol = record.get("ov_cimguiname") or record.get("cimguiname")
            symbol = string(symbol, f"definitions.{declaration_name}[{index}].symbol")
            value = {
                "cimgui_name": record.get("cimguiname"),
                "function": record.get("funcname"),
                "namespace": record.get("namespace"),
                "struct": record.get("stname"),
                "return_type": record.get("ret", "void"),
                "arguments": record.get("argsT", []),
                "signature": record.get("signature"),
                "defaults": record.get("defaults", {}),
                "traits": {
                    trait: bool(record[trait])
                    for trait in function_traits
                    if trait in record
                },
            }
            facts.append(ApiFact(source_id, "function", symbol, value))

    for name, value in constants.items():
        facts.append(ApiFact(source_id, "constant", name, value))

    def public_type(name: str) -> bool:
        candidates = (name, name.removesuffix("_"), f"{name}_")
        return any(
            _public_location(locations.get(candidate), api_locations)
            for candidate in candidates
        )

    for enum_name, raw_variants in enums.items():
        if not public_type(enum_name):
            continue
        underlying_type = enum_types.get(enum_name.removesuffix("_"))
        facts.append(
            ApiFact(
                source_id,
                "enum",
                enum_name,
                {"underlying_type": underlying_type},
            )
        )
        for index, raw_variant in enumerate(
            json_array(raw_variants, f"structs_and_enums.enums.{enum_name}")
        ):
            variant = json_mapping(
                raw_variant, f"structs_and_enums.enums.{enum_name}[{index}]"
            )
            variant_name = string(
                variant.get("name"),
                f"structs_and_enums.enums.{enum_name}[{index}].name",
            )
            facts.append(
                ApiFact(
                    source_id,
                    "enum-variant",
                    f"{enum_name}::{variant_name}",
                    {
                        "value": variant.get("value"),
                        "calculated_value": variant.get("calc_value"),
                    },
                )
            )

    for struct_name, raw_fields in structs.items():
        if not public_type(struct_name):
            continue
        fields = json_array(raw_fields, f"structs_and_enums.structs.{struct_name}")
        layout: list[dict[str, Any]] = []
        for index, raw_field in enumerate(fields):
            field = json_mapping(
                raw_field, f"structs_and_enums.structs.{struct_name}[{index}]"
            )
            raw_field_name = field.get("name")
            if raw_field_name is None:
                raise ContractInputError(
                    f"structs_and_enums.structs.{struct_name}[{index}].name is missing"
                )
            field_name = (
                f"@anonymous[{index}]"
                if raw_field_name == ""
                else string(
                    raw_field_name,
                    f"structs_and_enums.structs.{struct_name}[{index}].name",
                )
            )
            field_value = {
                "type": field.get("type"),
                "template_type": field.get("template_type"),
                "size": field.get("size"),
                "bitfield": field.get("bitfield"),
            }
            layout.append({"name": field_name, **field_value})
            facts.append(
                ApiFact(source_id, "field", f"{struct_name}::{field_name}", field_value)
            )
        facts.append(ApiFact(source_id, "layout", struct_name, layout))

    # Generator typedefs are part of the raw C contract even when their source
    # declaration has no location entry (e.g. transitive/public aliases).
    for typedef_name, value in typedefs.items():
        facts.append(ApiFact(source_id, "typedef", typedef_name, value))

    return _deduplicate_facts(facts, "generator output")


def _normalize_rust_fragment(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip()


def _split_top_level(value: str, delimiter: str) -> list[str]:
    parts: list[str] = []
    start = 0
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}", "<": ">"}
    closers = set(pairs.values())
    for index, character in enumerate(value):
        if character in pairs:
            stack.append(pairs[character])
        elif character in closers and not (character == ">" and index > 0 and value[index - 1] == "-"):
            if not stack or stack.pop() != character:
                raise ContractInputError(f"malformed Rust binding declaration: {value!r}")
        elif character == delimiter and not stack:
            parts.append(value[start:index])
            start = index + 1
    if stack:
        raise ContractInputError(f"unterminated Rust binding declaration: {value!r}")
    parts.append(value[start:])
    return parts


def _parse_rust_arguments(source_id: str, function_name: str, raw_arguments: str) -> list[dict[str, str]]:
    if not raw_arguments.strip():
        return []
    arguments: list[dict[str, str]] = []
    for index, raw_argument in enumerate(_split_top_level(raw_arguments, ",")):
        argument = _normalize_rust_fragment(raw_argument)
        if not argument:
            continue
        name, separator, argument_type = argument.partition(":")
        if not separator or _RUST_IDENTIFIER_RE.fullmatch(name) is None or not argument_type.strip():
            raise ContractInputError(
                f"{source_id} Rust binding function {function_name!r} has unsupported "
                f"argument {index}: {argument!r}"
            )
        arguments.append({"name": name, "type": _normalize_rust_fragment(argument_type)})
    return arguments


def _read_rust_declaration(lines: Sequence[str], start: int, terminator: str) -> tuple[str, int]:
    chunks: list[str] = []
    index = start
    while index < len(lines):
        chunks.append(lines[index].strip())
        merged = _normalize_rust_fragment(" ".join(chunks))
        if merged.endswith(terminator):
            return merged, index + 1
        index += 1
    raise ContractInputError("unterminated Rust binding declaration")


def _read_rust_struct(
    source_id: str, lines: Sequence[str], start: int, name: str
) -> tuple[list[ApiFact], int]:
    fields: list[dict[str, str]] = []
    index = start
    while index < len(lines):
        raw = lines[index].strip()
        index += 1
        if not raw or raw.startswith("//"):
            continue
        if raw == "}":
            facts: list[ApiFact] = []
            for field in fields:
                facts.append(
                    ApiFact(source_id, "field", f"{name}::{field['name']}", {"type": field["type"]})
                )
            facts.append(ApiFact(source_id, "layout", name, fields))
            return facts, index
        if not raw.endswith(","):
            raise ContractInputError(
                f"{source_id} Rust binding struct {name!r} has unsupported field syntax: {raw!r}"
            )
        declaration = raw.removesuffix(",").strip()
        if declaration.startswith("pub "):
            declaration = declaration.removeprefix("pub ")
        field_name, separator, field_type = declaration.partition(":")
        if (
            not separator
            or _RUST_IDENTIFIER_RE.fullmatch(field_name) is None
            or not field_type.strip()
        ):
            raise ContractInputError(
                f"{source_id} Rust binding struct {name!r} has unsupported field syntax: {raw!r}"
            )
        fields.append({"name": field_name, "type": _normalize_rust_fragment(field_type)})
    raise ContractInputError(f"{source_id} Rust binding struct {name!r} is not closed")


def _read_rust_extern_block(
    source_id: str, lines: Sequence[str], start: int
) -> tuple[list[ApiFact], int]:
    facts: list[ApiFact] = []
    index = start
    while index < len(lines):
        raw = lines[index].strip()
        if not raw or raw.startswith("//"):
            index += 1
            continue
        if raw == "}":
            return facts, index + 1
        if not raw.startswith("pub fn "):
            raise ContractInputError(
                f"{source_id} Rust binding extern block has unsupported declaration: {raw!r}"
            )
        declaration, index = _read_rust_declaration(lines, index, ";")
        function = declaration.removesuffix(";")
        match = _RUST_FUNCTION_RE.fullmatch(function)
        if match is None:
            raise ContractInputError(
                f"{source_id} Rust binding extern block has unsupported function: {function!r}"
            )
        name, raw_arguments, return_type = match.groups()
        facts.append(
            ApiFact(
                source_id,
                "function",
                name,
                {
                    "abi": "C",
                    "arguments": _parse_rust_arguments(source_id, name, raw_arguments),
                    "return_type": _normalize_rust_fragment(return_type or "()"),
                },
            )
        )
    raise ContractInputError(f"{source_id} Rust binding extern block is not closed")


def collect_rust_bindings_facts(
    source_id: str,
    binding_path: pathlib.Path,
    *,
    source_text: str | None = None,
) -> tuple[ApiFact, ...]:
    """Collect the final public raw Rust surface of a maintained binding file.

    The parser intentionally accepts only the small, generated bindgen grammar
    we support. A newly generated public syntax form is therefore an audit
    failure instead of an unobserved API change.
    """

    if source_text is None:
        try:
            source_text = binding_path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise ContractInputError(f"could not read Rust bindings {binding_path}: {error}") from error
    lines = source_text.splitlines()
    if not lines or not lines[0].startswith("// dear-imgui-rs-binding-provenance-v1 "):
        raise ContractInputError(
            f"{source_id} Rust binding {binding_path} lacks a binding provenance header"
        )

    facts: list[ApiFact] = []
    index = 1
    in_block_comment = False
    while index < len(lines):
        raw = lines[index].strip()
        index += 1
        if in_block_comment:
            if "*/" in raw:
                in_block_comment = False
            continue
        if not raw or raw.startswith("//"):
            continue
        if raw.startswith("/*"):
            in_block_comment = "*/" not in raw
            continue
        if raw.startswith("#["):
            continue
        struct_match = _RUST_STRUCT_RE.fullmatch(raw)
        if struct_match is not None:
            struct_facts, index = _read_rust_struct(source_id, lines, index, struct_match.group(1))
            facts.extend(struct_facts)
            continue
        if raw.startswith("pub const "):
            declaration, index = _read_rust_declaration(lines, index - 1, ";")
            match = _RUST_CONST_RE.fullmatch(declaration)
            if match is None:
                raise ContractInputError(
                    f"{source_id} Rust binding has unsupported public constant: {declaration!r}"
                )
            name, constant_type, value = match.groups()
            facts.append(
                ApiFact(
                    source_id,
                    "constant",
                    name,
                    {
                        "type": _normalize_rust_fragment(constant_type),
                        "value": _normalize_rust_fragment(value),
                    },
                )
            )
            continue
        if raw.startswith("pub type "):
            declaration, index = _read_rust_declaration(lines, index - 1, ";")
            match = _RUST_TYPE_RE.fullmatch(declaration)
            if match is None:
                raise ContractInputError(
                    f"{source_id} Rust binding has unsupported public typedef: {declaration!r}"
                )
            name, alias = match.groups()
            facts.append(
                ApiFact(source_id, "typedef", name, {"type": _normalize_rust_fragment(alias)})
            )
            continue
        if raw == 'unsafe extern "C" {':
            extern_facts, index = _read_rust_extern_block(source_id, lines, index)
            facts.extend(extern_facts)
            continue
        if raw.startswith("pub "):
            raise ContractInputError(
                f"{source_id} Rust binding has unsupported public syntax: {raw!r}"
            )
        raise ContractInputError(
            f"{source_id} Rust binding has unsupported top-level syntax: {raw!r}"
        )
    return _deduplicate_facts(facts, "Rust binding")


def _git_revision(path: pathlib.Path) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(path), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise ContractInputError(f"could not resolve source revision at {path}: {error}") from error
    revision = result.stdout.strip()
    if SHA1_RE.fullmatch(revision) is None:
        raise ContractInputError(f"source revision at {path} is not a full SHA-1: {revision!r}")
    return revision


def _source_crate_root(repo_root: pathlib.Path, source: Any) -> pathlib.Path:
    return repo_root / pathlib.Path(source.crate_root.as_posix())


def _binding_path(repo_root: pathlib.Path, source: Any) -> pathlib.Path:
    contract = source.api_contract
    if contract.path is None:
        raise ContractInputError(f"{source.id} has no Rust binding path")
    crate_root = _source_crate_root(repo_root, source).resolve()
    candidate = crate_root / pathlib.Path(contract.path.as_posix())
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise ContractInputError(f"could not resolve Rust binding {candidate}: {error}") from error
    if not resolved.is_relative_to(crate_root) or not resolved.is_file():
        raise ContractInputError(
            f"{source.id} Rust binding path {contract.path.as_posix()} escapes its crate root"
        )
    return resolved


def _collect_source_facts(repo_root: pathlib.Path, source: Any) -> tuple[ApiFact, ...]:
    contract = source.api_contract
    if contract.kind == "cimgui-generator":
        source_root = _source_crate_root(repo_root, source) / pathlib.Path(source.source_root.as_posix())
        return collect_generator_facts(
            source.id,
            source_root / "generator" / "output",
            contract.locations,
        )
    if contract.kind == "rust-bindings":
        return collect_rust_bindings_facts(source.id, _binding_path(repo_root, source))
    raise ContractInputError(
        f"{source.id} has unsupported API contract provider {contract.kind!r}"
    )


def collect_repository_snapshot(
    repo_root: pathlib.Path,
    inventory_path: pathlib.Path,
) -> ApiSnapshot:
    """Collect live source revisions and every maintained public API fact."""

    from _source_inventory import load_inventory_file

    inventory = load_inventory_file(inventory_path)
    facts: list[ApiFact] = []
    revisions: dict[str, str] = {}
    for source in inventory.sources:
        source_root = _source_crate_root(repo_root, source) / pathlib.Path(source.source_root.as_posix())
        revisions[f"source:{source.id}"] = _git_revision(source_root)
        facts.extend(_collect_source_facts(repo_root, source))
    for nested in inventory.nested_submodules:
        nested_root = (
            repo_root
            / pathlib.Path(nested.parent.as_posix())
            / pathlib.Path(nested.path.as_posix())
        )
        key = f"nested:{nested.parent.as_posix()}/{nested.path.as_posix()}"
        revisions[key] = _git_revision(nested_root)
    return ApiSnapshot(revisions, _deduplicate_facts(facts, "repository snapshot"))


def _git_show_json(repository: pathlib.Path, revision: str, relative_path: pathlib.PurePosixPath) -> Any:
    try:
        result = subprocess.run(
            ["git", "-C", str(repository), "show", f"{revision}:{relative_path.as_posix()}"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise FileNotFoundError(
            f"{relative_path.as_posix()} is absent at {revision} in {repository}"
        ) from error
    try:
        return json.loads(result.stdout, object_pairs_hook=reject_duplicate_keys)
    except (ContractInputError, json.JSONDecodeError) as error:
        raise ContractInputError(
            f"could not parse {relative_path.as_posix()} at {revision} in {repository}: {error}"
        ) from error


def collect_snapshot_at_source_revisions(
    source_revisions: Mapping[str, str],
    *,
    repo_root: pathlib.Path,
    inventory_path: pathlib.Path,
) -> ApiSnapshot:
    """Collect cimgui-generator facts from recorded commits without checking out.

    Rust binding contracts describe the checked-in generated artifact, whose
    provenance is verified by ``xtask verify-bindings``. Reconstructing that
    artifact from only an upstream source SHA would be unsound, so the helper
    fails closed when such a provider is present.
    """

    from _source_inventory import load_inventory_file

    inventory = load_inventory_file(inventory_path)
    facts: list[ApiFact] = []
    for source in inventory.sources:
        revision_key = f"source:{source.id}"
        revision = source_revisions.get(revision_key)
        if revision is None:
            raise ContractInputError(f"missing source revision {revision_key!r}")
        if SHA1_RE.fullmatch(revision) is None:
            raise ContractInputError(f"source revision {revision_key!r} is not a full SHA-1")
        contract = source.api_contract
        if contract.kind == "rust-bindings":
            raise ContractInputError(
                "cannot reconstruct Rust binding API facts from only upstream source revisions; "
                "audit the checked-in generated binding instead"
            )
        if contract.kind != "cimgui-generator":
            raise ContractInputError(
                f"{source.id} has unsupported API contract provider {contract.kind!r}"
            )
        source_root = _source_crate_root(repo_root, source) / pathlib.Path(source.source_root.as_posix())

        def read_at_revision(path: pathlib.Path, *, root: pathlib.Path = source_root) -> Any:
            try:
                relative_path = pathlib.PurePosixPath(path.relative_to(root).as_posix())
            except ValueError as error:
                raise ContractInputError(
                    f"generator path {path} escapes maintained source {root}"
                ) from error
            return _git_show_json(root, revision, relative_path)

        facts.extend(
            collect_generator_facts(
                source.id,
                source_root / "generator" / "output",
                contract.locations,
                json_reader=read_at_revision,
            )
        )
    return ApiSnapshot(dict(source_revisions), _deduplicate_facts(facts, "historical snapshot"))


def _git_submodule_revision(
    repository: pathlib.Path, revision: str, path: pathlib.PurePosixPath
) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(repository), "ls-tree", revision, "--", path.as_posix()],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise ContractInputError(
            f"could not inspect submodule {path.as_posix()} at {revision} in {repository}: {error}"
        ) from error
    fields = result.stdout.strip().split()
    if len(fields) < 3 or fields[0] != "160000" or SHA1_RE.fullmatch(fields[2]) is None:
        raise ContractInputError(
            f"{path.as_posix()} is not a gitlink at {revision} in {repository}"
        )
    return fields[2]


def resolve_nested_source_revisions(
    top_level_revisions: Mapping[str, str],
    *,
    repo_root: pathlib.Path,
    inventory_path: pathlib.Path,
) -> dict[str, str]:
    """Resolve every nested gitlink at a recorded top-level source revision."""

    from _source_inventory import load_inventory_file

    inventory = load_inventory_file(inventory_path)
    revisions = dict(top_level_revisions)
    repository_revisions: dict[pathlib.Path, str] = {}
    for source in inventory.sources:
        key = f"source:{source.id}"
        revision = revisions.get(key)
        if revision is None:
            raise ContractInputError(f"missing top-level source revision {key!r}")
        repository_revisions[
            _source_crate_root(repo_root, source) / pathlib.Path(source.source_root.as_posix())
        ] = revision

    pending = list(inventory.nested_submodules)
    while pending:
        remaining = []
        progressed = False
        for nested in pending:
            parent_root = repo_root / pathlib.Path(nested.parent.as_posix())
            parent_revision = repository_revisions.get(parent_root)
            if parent_revision is None:
                remaining.append(nested)
                continue
            nested_revision = _git_submodule_revision(parent_root, parent_revision, nested.path)
            nested_root = parent_root / pathlib.Path(nested.path.as_posix())
            repository_revisions[nested_root] = nested_revision
            revisions[f"nested:{nested.parent.as_posix()}/{nested.path.as_posix()}"] = nested_revision
            progressed = True
        if not progressed:
            unresolved = ", ".join(
                f"{nested.parent.as_posix()}/{nested.path.as_posix()}" for nested in remaining
            )
            raise ContractInputError(
                "could not resolve nested source revisions because parent source revisions "
                f"are missing: {unresolved}"
            )
        pending = remaining
    return revisions
