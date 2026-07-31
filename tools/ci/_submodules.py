"""Authoritative nested submodule topology used by repository CI tools."""

from __future__ import annotations

from pathlib import Path

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
