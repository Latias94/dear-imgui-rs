"""Authoritative nested submodule topology used by repository CI tools."""

from __future__ import annotations

from pathlib import Path, PurePosixPath

from _source_inventory import NestedSubmodule, load_inventory


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_INVENTORY = load_inventory(REPO_ROOT)

SELECTIVE_NESTED_SUBMODULES: tuple[NestedSubmodule, ...] = (
    SOURCE_INVENTORY.nested_submodules
)
PACKAGE_NESTED_SUBMODULES: tuple[NestedSubmodule, ...] = (
    SOURCE_INVENTORY.package_submodules()
)
SUBMODULE_COMMANDS = tuple(
    submodule.update_command() for submodule in SELECTIVE_NESTED_SUBMODULES
)


def _source_root(source_id: str) -> PurePosixPath:
    source = SOURCE_INVENTORY.source_by_id(source_id)
    return source.crate_root / source.source_root


def _top_level_update_command(path: PurePosixPath) -> tuple[str, ...]:
    return (
        "git",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        path.as_posix(),
    )


RUNTIME_CORE_SOURCE = _source_root("core")
RUNTIME_TEST_ENGINE_SOURCE = _source_root("test-engine")
RUNTIME_CORE_NESTED_COMMANDS = tuple(
    submodule.update_command()
    for submodule in SELECTIVE_NESTED_SUBMODULES
    if submodule.parent == RUNTIME_CORE_SOURCE
)
RUNTIME_CORE_COMMANDS = (
    _top_level_update_command(RUNTIME_CORE_SOURCE),
    *RUNTIME_CORE_NESTED_COMMANDS,
)
RUNTIME_TEST_ENGINE_COMMANDS = (
    _top_level_update_command(RUNTIME_CORE_SOURCE),
    _top_level_update_command(RUNTIME_TEST_ENGINE_SOURCE),
    *RUNTIME_CORE_NESTED_COMMANDS,
)
SUBMODULE_PROFILES = {
    "all": SUBMODULE_COMMANDS,
    "runtime-core": RUNTIME_CORE_COMMANDS,
    "runtime-test-engine": RUNTIME_TEST_ENGINE_COMMANDS,
}
