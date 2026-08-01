"""Strict reader for the repository's maintained-source inventory."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any, Iterable


INVENTORY_SCHEMA = "dear-imgui-maintained-sources-v2"
INVENTORY_RELATIVE_PATH = Path("tools/build-support/maintained_sources.json")


class SourceInventoryError(ValueError):
    """Raised when the checked-in source inventory is malformed or ambiguous."""


@dataclass(frozen=True)
class MaintainedSourceFile:
    id: str
    canonical: PurePosixPath
    alternates: tuple[PurePosixPath, ...]
    provider_transform: str | None

    @property
    def candidates(self) -> tuple[PurePosixPath, ...]:
        return (self.canonical, *self.alternates)


@dataclass(frozen=True)
class WasmProviderSpec:
    wasm_bindings: PurePosixPath
    symbol_prefixes: tuple[str, ...]
    required_exports: tuple[str, ...]
    include_dirs: tuple[PurePosixPath, ...]
    source_files: tuple[str, ...]


@dataclass(frozen=True)
class ApiContractSpec:
    kind: str
    locations: tuple[str, ...] = ()
    path: PurePosixPath | None = None


@dataclass(frozen=True)
class MaintainedSource:
    id: str
    crate_name: str
    crate_root: PurePosixPath
    source_root: PurePosixPath
    api_contract: ApiContractSpec
    files: tuple[MaintainedSourceFile, ...]
    native_required_files: tuple[str, ...]
    archive_sentinels: tuple[str, ...]
    provider: WasmProviderSpec | None

    def file(self, file_id: str) -> MaintainedSourceFile:
        matching = [source_file for source_file in self.files if source_file.id == file_id]
        if len(matching) != 1:
            raise SourceInventoryError(
                f"maintained source {self.id!r} does not define file id {file_id!r}"
            )
        return matching[0]

    def resolve_file(self, crate_root: Path, file_id: str) -> Path:
        candidates = [crate_root / Path(path.as_posix()) for path in self.file(file_id).candidates]
        existing = [candidate for candidate in candidates if candidate.is_file()]
        if len(existing) == 1:
            return existing[0]
        paths = ", ".join(str(candidate) for candidate in (existing or candidates))
        if existing:
            raise SourceInventoryError(
                f"maintained source {self.id!r} file {file_id!r} is ambiguous; "
                f"found multiple supported paths: {paths}"
            )
        raise SourceInventoryError(
            f"maintained source {self.id!r} file {file_id!r} is missing; "
            f"expected exactly one of: {paths}"
        )

    def resolve_source_root(self, crate_root: Path) -> Path:
        source_root = crate_root / Path(self.source_root.as_posix())
        if not source_root.is_dir():
            raise SourceInventoryError(
                f"maintained source {self.id!r} is missing directory {source_root}"
            )
        return source_root

    def resolve_native_sources(self, crate_root: Path) -> tuple[Path, ...]:
        self.resolve_source_root(crate_root)
        return tuple(
            self.resolve_file(crate_root, file_id)
            for file_id in self.native_required_files
        )

    def resolve_archive_sentinels(self, repo_root: Path) -> tuple[str, ...]:
        crate_root = repo_root / Path(self.crate_root.as_posix())
        return tuple(
            self.resolve_file(crate_root, file_id)
            .relative_to(crate_root)
            .as_posix()
            for file_id in self.archive_sentinels
        )


@dataclass(frozen=True)
class NestedSubmodule:
    parent: PurePosixPath
    path: PurePosixPath
    shallow: bool
    package: bool
    package_order: int | None

    def update_command(self) -> tuple[str, ...]:
        command = [
            "git",
            "-C",
            self.parent.as_posix(),
            "submodule",
            "update",
            "--init",
        ]
        if self.shallow:
            command.append("--depth=1")
        command.append(self.path.as_posix())
        return tuple(command)


@dataclass(frozen=True)
class SourceInventory:
    schema: str
    wasm_import_module: str
    sources: tuple[MaintainedSource, ...]
    nested_submodules: tuple[NestedSubmodule, ...]

    def source_by_id(self, source_id: str) -> MaintainedSource:
        matching = [source for source in self.sources if source.id == source_id]
        if len(matching) != 1:
            raise SourceInventoryError(
                f"maintained-source inventory does not define source id {source_id!r}"
            )
        return matching[0]

    def source_by_crate(self, crate_name: str) -> MaintainedSource:
        matching = [source for source in self.sources if source.crate_name == crate_name]
        if len(matching) != 1:
            raise SourceInventoryError(
                f"maintained-source inventory does not define crate {crate_name!r}"
            )
        return matching[0]

    def archive_sentinels(self, repo_root: Path) -> dict[str, tuple[str, ...]]:
        return {
            source.crate_name: source.resolve_archive_sentinels(repo_root)
            for source in self.sources
        }

    def package_submodules(self) -> tuple[NestedSubmodule, ...]:
        packaged = [submodule for submodule in self.nested_submodules if submodule.package]
        return tuple(
            sorted(packaged, key=lambda submodule: _required_package_order(submodule))
        )


def load_inventory(repo_root: Path | None = None) -> SourceInventory:
    if repo_root is None:
        repo_root = Path(__file__).resolve().parents[2]
    return load_inventory_file(repo_root / INVENTORY_RELATIVE_PATH)


def load_inventory_file(inventory_path: Path) -> SourceInventory:
    """Load and validate one explicit maintained-source inventory file."""
    inventory_path = inventory_path.resolve()
    try:
        with inventory_path.open(encoding="utf-8") as inventory_file:
            raw = json.load(inventory_file, object_pairs_hook=_reject_duplicate_keys)
    except SourceInventoryError:
        raise
    except (OSError, json.JSONDecodeError) as error:
        raise SourceInventoryError(
            f"could not read maintained-source inventory {inventory_path}: {error}"
        ) from error
    inventory = _parse_inventory(raw)
    _validate_inventory(inventory)
    return inventory


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise SourceInventoryError(f"duplicate JSON key in source inventory: {key!r}")
        result[key] = value
    return result


def _parse_inventory(raw: Any) -> SourceInventory:
    data = _object(
        raw,
        {"schema", "wasm_import_module", "sources", "nested_submodules"},
        "inventory",
    )
    return SourceInventory(
        schema=_string(data["schema"], "inventory.schema"),
        wasm_import_module=_identifier(
            data["wasm_import_module"], "inventory.wasm_import_module"
        ),
        sources=tuple(
            _parse_source(item, index)
            for index, item in enumerate(_array(data["sources"], "inventory.sources"))
        ),
        nested_submodules=tuple(
            _parse_nested_submodule(item, index)
            for index, item in enumerate(
                _array(data["nested_submodules"], "inventory.nested_submodules")
            )
        ),
    )


def _parse_source(raw: Any, index: int) -> MaintainedSource:
    context = f"inventory.sources[{index}]"
    data = _object(
        raw,
        {
            "id",
            "crate_name",
            "crate_root",
            "source_root",
            "api_contract",
            "files",
            "native_required_files",
            "archive_sentinels",
            "provider",
        },
        context,
    )
    provider_raw = data["provider"]
    return MaintainedSource(
        id=_identifier(data["id"], f"{context}.id"),
        crate_name=_identifier(data["crate_name"], f"{context}.crate_name"),
        crate_root=_relative_path(data["crate_root"], f"{context}.crate_root"),
        source_root=_relative_path(data["source_root"], f"{context}.source_root"),
        api_contract=_parse_api_contract(data["api_contract"], context),
        files=tuple(
            _parse_source_file(item, context, file_index)
            for file_index, item in enumerate(_array(data["files"], f"{context}.files"))
        ),
        native_required_files=_identifier_tuple(
            data["native_required_files"], f"{context}.native_required_files"
        ),
        archive_sentinels=_identifier_tuple(
            data["archive_sentinels"], f"{context}.archive_sentinels"
        ),
        provider=None if provider_raw is None else _parse_provider(provider_raw, context),
    )


def _parse_api_contract(raw: Any, source_context: str) -> ApiContractSpec:
    context = f"{source_context}.api_contract"
    if not isinstance(raw, dict):
        raise SourceInventoryError(f"{context} must be a JSON object")
    kind = _string(raw.get("kind"), f"{context}.kind")
    if kind == "cimgui-generator":
        data = _object(raw, {"kind", "locations"}, context)
        locations = _c_symbol_tuple(data["locations"], f"{context}.locations")
        if not locations:
            raise SourceInventoryError(f"{context}.locations must not be empty")
        _require_unique(locations, f"{context}.locations entry")
        return ApiContractSpec(kind=kind, locations=locations)
    if kind == "rust-bindings":
        data = _object(raw, {"kind", "path"}, context)
        path = _relative_path(data["path"], f"{context}.path")
        if path.suffix != ".rs":
            raise SourceInventoryError(f"{context}.path must name a .rs file")
        return ApiContractSpec(kind=kind, path=path)
    raise SourceInventoryError(f"{context}.kind has unsupported value {kind!r}")


def _parse_source_file(raw: Any, source_context: str, index: int) -> MaintainedSourceFile:
    context = f"{source_context}.files[{index}]"
    data = _object(
        raw,
        {"id", "canonical", "alternates", "provider_transform"},
        context,
    )
    transform = data["provider_transform"]
    if transform is not None:
        transform = _string(transform, f"{context}.provider_transform")
        if transform not in {"direct", "patch-imgui-core", "patch-imgui-demo"}:
            raise SourceInventoryError(
                f"{context}.provider_transform has unsupported value {transform!r}"
            )
    return MaintainedSourceFile(
        id=_identifier(data["id"], f"{context}.id"),
        canonical=_relative_path(data["canonical"], f"{context}.canonical"),
        alternates=tuple(
            _relative_path(item, f"{context}.alternates[{alternate_index}]")
            for alternate_index, item in enumerate(
                _array(data["alternates"], f"{context}.alternates")
            )
        ),
        provider_transform=transform,
    )


def _parse_provider(raw: Any, source_context: str) -> WasmProviderSpec:
    context = f"{source_context}.provider"
    data = _object(
        raw,
        {
            "wasm_bindings",
            "symbol_prefixes",
            "required_exports",
            "include_dirs",
            "source_files",
        },
        context,
    )
    return WasmProviderSpec(
        wasm_bindings=_relative_path(data["wasm_bindings"], f"{context}.wasm_bindings"),
        symbol_prefixes=_c_symbol_tuple(
            data["symbol_prefixes"], f"{context}.symbol_prefixes"
        ),
        required_exports=_c_symbol_tuple(
            data["required_exports"], f"{context}.required_exports"
        ),
        include_dirs=tuple(
            _relative_path(item, f"{context}.include_dirs[{index}]")
            for index, item in enumerate(
                _array(data["include_dirs"], f"{context}.include_dirs")
            )
        ),
        source_files=_identifier_tuple(
            data["source_files"], f"{context}.source_files"
        ),
    )


def _parse_nested_submodule(raw: Any, index: int) -> NestedSubmodule:
    context = f"inventory.nested_submodules[{index}]"
    data = _object(
        raw,
        {"parent", "path", "shallow", "package", "package_order"},
        context,
    )
    package_order = data["package_order"]
    if package_order is not None and (
        not isinstance(package_order, int)
        or isinstance(package_order, bool)
        or package_order < 0
    ):
        raise SourceInventoryError(
            f"{context}.package_order must be a non-negative integer or null"
        )
    return NestedSubmodule(
        parent=_relative_path(data["parent"], f"{context}.parent"),
        path=_relative_path(data["path"], f"{context}.path"),
        shallow=_boolean(data["shallow"], f"{context}.shallow"),
        package=_boolean(data["package"], f"{context}.package"),
        package_order=package_order,
    )


def _validate_inventory(inventory: SourceInventory) -> None:
    if inventory.schema != INVENTORY_SCHEMA:
        raise SourceInventoryError(
            f"unsupported source inventory schema {inventory.schema!r}; "
            f"expected {INVENTORY_SCHEMA!r}"
        )
    if not inventory.sources:
        raise SourceInventoryError("source inventory must contain at least one source")
    _require_unique((source.id for source in inventory.sources), "source id")
    _require_unique((source.crate_name for source in inventory.sources), "crate name")
    _require_unique((source.crate_root for source in inventory.sources), "crate root")
    for source in inventory.sources:
        _validate_source(source)

    locations = tuple(
        PurePosixPath(submodule.parent, submodule.path)
        for submodule in inventory.nested_submodules
    )
    _require_unique(locations, "nested submodule location")
    package_orders = []
    for submodule in inventory.nested_submodules:
        if submodule.package != (submodule.package_order is not None):
            raise SourceInventoryError(
                f"nested submodule {submodule.parent / submodule.path} must set "
                "package_order exactly when package is true"
            )
        if submodule.package_order is not None:
            package_orders.append(submodule.package_order)
    _require_unique(package_orders, "nested submodule package order")
    if sorted(package_orders) != list(range(len(package_orders))):
        raise SourceInventoryError(
            "nested submodule package_order values must be contiguous from zero"
        )


def _validate_source(source: MaintainedSource) -> None:
    if not source.files:
        raise SourceInventoryError(
            f"maintained source {source.id!r} must define at least one file"
        )
    _require_unique((source_file.id for source_file in source.files), "file id")
    _require_unique(
        (path for source_file in source.files for path in source_file.candidates),
        "source file path",
    )
    files_by_id = {source_file.id: source_file for source_file in source.files}
    for field, references in (
        ("native_required_files", source.native_required_files),
        ("archive_sentinels", source.archive_sentinels),
    ):
        _validate_file_references(source, field, references, files_by_id)

    transformed = {
        source_file.id
        for source_file in source.files
        if source_file.provider_transform is not None
    }
    if source.provider is None:
        if transformed:
            raise SourceInventoryError(
                f"maintained source {source.id!r} declares provider transforms "
                "without a provider"
            )
        return
    provider = source.provider
    _validate_file_references(
        source, "provider.source_files", provider.source_files, files_by_id
    )
    provider_files = set(provider.source_files)
    if provider_files != transformed:
        raise SourceInventoryError(
            f"maintained source {source.id!r} provider source files differ from "
            "files carrying provider transforms"
        )
    _require_unique(provider.symbol_prefixes, "provider symbol prefix")
    _require_unique(provider.required_exports, "provider required export")
    _require_unique(provider.include_dirs, "provider include directory")


def _validate_file_references(
    source: MaintainedSource,
    field: str,
    references: Iterable[str],
    files_by_id: dict[str, MaintainedSourceFile],
) -> None:
    references = tuple(references)
    _require_unique(references, field)
    for file_id in references:
        if file_id not in files_by_id:
            raise SourceInventoryError(
                f"maintained source {source.id!r} {field} references unknown "
                f"file id {file_id!r}"
            )


def _object(raw: Any, expected_keys: set[str], context: str) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise SourceInventoryError(f"{context} must be a JSON object")
    actual_keys = set(raw)
    if actual_keys != expected_keys:
        missing = sorted(expected_keys - actual_keys)
        unexpected = sorted(actual_keys - expected_keys)
        raise SourceInventoryError(
            f"{context} keys differ from the schema; missing={missing}, "
            f"unexpected={unexpected}"
        )
    return raw


def _object_with_optional(
    raw: Any, allowed_keys: set[str], context: str
) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise SourceInventoryError(f"{context} must be a JSON object")
    unexpected = sorted(set(raw) - allowed_keys)
    if unexpected:
        raise SourceInventoryError(
            f"{context} contains unexpected keys: {unexpected}"
        )
    return raw


def _array(raw: Any, context: str) -> list[Any]:
    if not isinstance(raw, list):
        raise SourceInventoryError(f"{context} must be a JSON array")
    return raw


def _string(raw: Any, context: str) -> str:
    if not isinstance(raw, str):
        raise SourceInventoryError(f"{context} must be a string")
    return raw


def _identifier(raw: Any, context: str) -> str:
    value = _string(raw, context)
    if not value or any(character.isspace() for character in value):
        raise SourceInventoryError(
            f"{context} must be a non-empty identifier without whitespace"
        )
    return value


def _identifier_tuple(raw: Any, context: str) -> tuple[str, ...]:
    return tuple(
        _identifier(item, f"{context}[{index}]")
        for index, item in enumerate(_array(raw, context))
    )


def _c_symbol_tuple(raw: Any, context: str) -> tuple[str, ...]:
    return tuple(
        _c_symbol(item, f"{context}[{index}]")
        for index, item in enumerate(_array(raw, context))
    )


def _c_symbol(raw: Any, context: str) -> str:
    value = _string(raw, context)
    valid_start = bool(value) and (
        value[0] == "_" or value[0].isascii() and value[0].isalpha()
    )
    valid_rest = all(
        character == "_" or character.isascii() and character.isalnum()
        for character in value[1:]
    )
    if (
        not valid_start
        or not valid_rest
    ):
        raise SourceInventoryError(
            f"{context} must be a portable C symbol, got {value!r}"
        )
    return value


def _relative_path(raw: Any, context: str) -> PurePosixPath:
    value = _string(raw, context)
    windows_path = PureWindowsPath(value)
    parts = value.split("/")
    if (
        not value
        or "\\" in value
        or ":" in value
        or windows_path.drive
        or any(part in {"", ".", ".."} for part in parts)
    ):
        raise SourceInventoryError(
            f"{context} must be a normalized forward-slash relative path, got {value!r}"
        )
    return PurePosixPath(value)


def _boolean(raw: Any, context: str) -> bool:
    if not isinstance(raw, bool):
        raise SourceInventoryError(f"{context} must be a boolean")
    return raw


def _require_unique(values: Iterable[Any], description: str) -> None:
    seen = set()
    for value in values:
        if value in seen:
            raise SourceInventoryError(f"duplicate {description}: {value!r}")
        seen.add(value)


def _required_package_order(submodule: NestedSubmodule) -> int:
    if submodule.package_order is None:
        raise AssertionError("non-packaged submodule reached package ordering")
    return submodule.package_order
