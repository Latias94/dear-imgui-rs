#!/usr/bin/env python3
"""Read, update, and verify authoritative core and binding source revisions."""

from __future__ import annotations

import argparse
import os
import re
import stat
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

CI_DIR = Path(__file__).resolve().parent / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _source_inventory import load_inventory  # noqa: E402


SOURCE_METADATA_SECTION = "package.metadata.dear-imgui-sources"
SOURCE_METADATA_KEYS = frozenset({"cimgui-revision", "imgui-revision"})
BINDING_SOURCE_METADATA_SECTION = "package.metadata.dear-imgui-binding"
BINDING_SOURCE_METADATA_KEY = "source-revision"
GIT_REVISION_RE = re.compile(r"^[0-9a-fA-F]{40}$")


@dataclass(frozen=True)
class SourceSpec:
    """One vendored Git worktree represented in package metadata."""

    label: str
    relative_path: Path
    metadata_key: str


@dataclass(frozen=True)
class BindingSourceSpec:
    """One sys crate and the top-level submodule that owns its bindings."""

    crate_name: str
    label: str
    manifest_path: Path
    relative_path: Path


SOURCE_INVENTORY = load_inventory(Path(__file__).resolve().parents[1])
CORE_SOURCE = SOURCE_INVENTORY.source_by_id("core")
CORE_ROOT = Path(CORE_SOURCE.crate_root.as_posix())
CORE_VENDORED_ROOT = CORE_ROOT / Path(CORE_SOURCE.source_root.as_posix())
CORE_MANIFEST_PATH = CORE_ROOT / "Cargo.toml"
CORE_IMGUI_SUBMODULE = next(
    submodule
    for submodule in SOURCE_INVENTORY.nested_submodules
    if Path(submodule.parent.as_posix()) == CORE_VENDORED_ROOT
    and submodule.path.as_posix() == "imgui"
)

CORE_SOURCE_SPECS = (
    SourceSpec(
        label="cimgui",
        relative_path=CORE_VENDORED_ROOT,
        metadata_key="cimgui-revision",
    ),
    SourceSpec(
        label="Dear ImGui",
        relative_path=CORE_VENDORED_ROOT / Path(CORE_IMGUI_SUBMODULE.path.as_posix()),
        metadata_key="imgui-revision",
    ),
)

BINDING_SOURCE_LABELS = {
    "test-engine": "Dear ImGui Test Engine",
    "implot": "cimplot",
    "implot3d": "cimplot3d",
    "imnodes": "cimnodes",
    "node-editor": "cimnodes_editor",
    "imguizmo": "cimguizmo",
    "imguizmo-quat": "cimguizmo_quat",
}
BINDING_SOURCE_SPECS = tuple(
    BindingSourceSpec(
        crate_name=source.crate_name,
        label=BINDING_SOURCE_LABELS[source.id],
        manifest_path=Path(source.crate_root.as_posix()) / "Cargo.toml",
        relative_path=(
            Path(source.crate_root.as_posix()) / Path(source.source_root.as_posix())
        ),
    )
    for source in SOURCE_INVENTORY.sources
    if source.id != "core"
)


class SourceMetadataError(RuntimeError):
    """A fail-closed source metadata validation failure."""

    def __init__(self, errors: str | Iterable[str]):
        if isinstance(errors, str):
            errors = (errors,)
        self.errors = tuple(errors)
        super().__init__("\n".join(self.errors))


@dataclass(frozen=True)
class MetadataUpdate:
    """Result of synchronizing metadata with clean vendored worktrees."""

    previous: dict[str, str]
    revisions: dict[str, str]
    changed: bool
    written: bool


def _parse_core_source_metadata(
    manifest_text: str, manifest_path: Path
) -> dict[str, str]:
    try:
        data = tomllib.loads(manifest_text)
    except tomllib.TOMLDecodeError as error:
        raise SourceMetadataError(f"invalid TOML in {manifest_path}: {error}") from error

    try:
        metadata = data["package"]["metadata"]["dear-imgui-sources"]
    except (KeyError, TypeError) as error:
        raise SourceMetadataError(
            f"missing [{SOURCE_METADATA_SECTION}] in {manifest_path}"
        ) from error

    if not isinstance(metadata, dict) or set(metadata) != SOURCE_METADATA_KEYS:
        found = sorted(metadata) if isinstance(metadata, dict) else type(metadata).__name__
        raise SourceMetadataError(
            f"[{SOURCE_METADATA_SECTION}] must contain exactly "
            f"{sorted(SOURCE_METADATA_KEYS)}, found {found}"
        )

    revisions: dict[str, str] = {}
    for key, value in metadata.items():
        if not isinstance(value, str) or GIT_REVISION_RE.fullmatch(value) is None:
            raise SourceMetadataError(
                f"{key} must be exactly 40 ASCII hexadecimal characters"
            )
        revisions[key] = value
    return revisions


def read_core_source_metadata(manifest_path: Path) -> dict[str, str]:
    """Read the exact source provenance schema from a Cargo manifest."""
    try:
        manifest_text = manifest_path.read_text(encoding="utf-8")
    except OSError as error:
        raise SourceMetadataError(f"could not read {manifest_path}: {error}") from error
    return _parse_core_source_metadata(manifest_text, manifest_path)


def _parse_binding_source_metadata(manifest_text: str, manifest_path: Path) -> str:
    try:
        data = tomllib.loads(manifest_text)
    except tomllib.TOMLDecodeError as error:
        raise SourceMetadataError(f"invalid TOML in {manifest_path}: {error}") from error

    try:
        metadata = data["package"]["metadata"]["dear-imgui-binding"]
    except (KeyError, TypeError) as error:
        raise SourceMetadataError(
            f"missing [{BINDING_SOURCE_METADATA_SECTION}] in {manifest_path}"
        ) from error

    expected_keys = {BINDING_SOURCE_METADATA_KEY}
    if not isinstance(metadata, dict) or set(metadata) != expected_keys:
        found = sorted(metadata) if isinstance(metadata, dict) else type(metadata).__name__
        raise SourceMetadataError(
            f"[{BINDING_SOURCE_METADATA_SECTION}] must contain exactly "
            f"{sorted(expected_keys)}, found {found}"
        )

    revision = metadata[BINDING_SOURCE_METADATA_KEY]
    if not isinstance(revision, str) or GIT_REVISION_RE.fullmatch(revision) is None:
        raise SourceMetadataError(
            f"{BINDING_SOURCE_METADATA_KEY} must be exactly 40 ASCII hexadecimal characters"
        )
    return revision


def read_binding_source_metadata(manifest_path: Path) -> str:
    """Read one crate-local binding source revision from a Cargo manifest."""
    try:
        manifest_text = manifest_path.read_text(encoding="utf-8")
    except OSError as error:
        raise SourceMetadataError(f"could not read {manifest_path}: {error}") from error
    return _parse_binding_source_metadata(manifest_text, manifest_path)


def _git_output(path: Path, arguments: Sequence[str]) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(path), *arguments],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise SourceMetadataError(f"could not run git for {path}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "git command failed"
        raise SourceMetadataError(f"could not inspect vendored source {path}: {detail}")
    return result.stdout


def _same_path(left: Path, right: Path) -> bool:
    return os.path.normcase(os.path.realpath(left)) == os.path.normcase(
        os.path.realpath(right)
    )


def inspect_core_sources(repo_root: Path) -> dict[str, str]:
    """Return revisions only when every expected vendored worktree is clean."""
    errors: list[str] = []
    revisions: dict[str, str] = {}

    for source in CORE_SOURCE_SPECS:
        source_path = repo_root / source.relative_path
        if not source_path.is_dir():
            errors.append(f"vendored source path is missing: {source_path}")
            continue

        try:
            top_level = _git_output(
                source_path, ("rev-parse", "--show-toplevel")
            ).strip()
        except SourceMetadataError as error:
            errors.extend(error.errors)
            continue
        if not top_level or not _same_path(Path(top_level), source_path):
            errors.append(
                f"vendored source is not the expected Git worktree: {source_path} "
                f"(git top-level: {top_level or '<empty>'})"
            )
            continue

        try:
            status = _git_output(
                source_path,
                (
                    "status",
                    "--porcelain=v1",
                    "--untracked-files=all",
                    "--ignore-submodules=none",
                ),
            )
        except SourceMetadataError as error:
            errors.extend(error.errors)
            continue
        if status:
            errors.append(
                f"{source.label} source tree is dirty: {source_path}\n"
                f"{status.rstrip()}"
            )
            continue

        try:
            revision = _git_output(source_path, ("rev-parse", "HEAD")).strip()
        except SourceMetadataError as error:
            errors.extend(error.errors)
            continue
        if GIT_REVISION_RE.fullmatch(revision) is None:
            errors.append(
                f"invalid {source.label} revision from {source_path}: {revision!r}"
            )
            continue
        revisions[source.metadata_key] = revision

    if errors:
        raise SourceMetadataError(errors)
    if set(revisions) != SOURCE_METADATA_KEYS:
        raise SourceMetadataError(
            "source inspection did not produce every required revision: "
            f"found {sorted(revisions)}"
        )
    return revisions


def verify_core_source_metadata(repo_root: Path) -> dict[str, str]:
    """Verify schema, fixed vendored paths, cleanliness, and exact HEAD parity."""
    metadata = read_core_source_metadata(repo_root / CORE_MANIFEST_PATH)
    revisions = inspect_core_sources(repo_root)
    errors = []
    for source in CORE_SOURCE_SPECS:
        recorded = metadata[source.metadata_key]
        actual = revisions[source.metadata_key]
        if recorded != actual:
            errors.append(
                f"{source.label} revision mismatch: metadata {recorded}, HEAD {actual}"
            )
    if errors:
        raise SourceMetadataError(errors)
    return revisions


def inspect_binding_source(repo_root: Path, spec: BindingSourceSpec) -> str:
    """Return one exact revision only when its owning submodule is clean."""
    source_path = repo_root / spec.relative_path
    if not source_path.is_dir():
        raise SourceMetadataError(f"vendored source path is missing: {source_path}")

    top_level = _git_output(source_path, ("rev-parse", "--show-toplevel")).strip()
    if not top_level or not _same_path(Path(top_level), source_path):
        raise SourceMetadataError(
            f"vendored source is not the expected Git worktree: {source_path} "
            f"(git top-level: {top_level or '<empty>'})"
        )

    status = _git_output(
        source_path,
        (
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ),
    )
    if status:
        raise SourceMetadataError(
            f"{spec.label} source tree is dirty: {source_path}\n{status.rstrip()}"
        )

    revision = _git_output(source_path, ("rev-parse", "HEAD")).strip()
    if GIT_REVISION_RE.fullmatch(revision) is None:
        raise SourceMetadataError(
            f"invalid {spec.label} revision from {source_path}: {revision!r}"
        )
    return revision


def verify_binding_source_metadata(
    repo_root: Path, spec: BindingSourceSpec
) -> str:
    """Verify one crate-local revision against its clean owning submodule."""
    recorded = read_binding_source_metadata(repo_root / spec.manifest_path)
    actual = inspect_binding_source(repo_root, spec)
    if recorded != actual:
        raise SourceMetadataError(
            f"{spec.label} revision mismatch: metadata {recorded}, HEAD {actual}"
        )
    return actual


def verify_all_binding_source_metadata(repo_root: Path) -> dict[str, str]:
    """Verify every Test Engine and extension binding source independently."""
    revisions: dict[str, str] = {}
    errors: list[str] = []
    for spec in BINDING_SOURCE_SPECS:
        try:
            revisions[spec.crate_name] = verify_binding_source_metadata(repo_root, spec)
        except SourceMetadataError as error:
            errors.extend(error.errors)
    if errors:
        raise SourceMetadataError(errors)
    return revisions


def _replace_metadata_values(
    manifest_text: str, manifest_path: Path, revisions: dict[str, str]
) -> str:
    lines = manifest_text.splitlines(keepends=True)
    section_header = f"[{SOURCE_METADATA_SECTION}]"
    try:
        section_start = next(
            index for index, line in enumerate(lines) if line.strip() == section_header
        )
    except StopIteration as error:
        raise SourceMetadataError(
            f"missing {section_header} in {manifest_path}"
        ) from error

    section_end = next(
        (
            index
            for index in range(section_start + 1, len(lines))
            if lines[index].lstrip().startswith("[")
        ),
        len(lines),
    )
    found: set[str] = set()
    assignment = re.compile(r"^(\s*)([A-Za-z0-9_-]+)(\s*=\s*).*$")
    for index in range(section_start + 1, section_end):
        match = assignment.match(lines[index])
        if match is None or match.group(2) not in revisions:
            continue
        key = match.group(2)
        newline = "\n" if lines[index].endswith("\n") else ""
        lines[index] = (
            f'{match.group(1)}{key}{match.group(3)}"{revisions[key]}"{newline}'
        )
        found.add(key)

    if found != SOURCE_METADATA_KEYS:
        raise SourceMetadataError(
            f"could not update all source metadata keys in {manifest_path}: "
            f"found {sorted(found)}"
        )
    updated = "".join(lines)
    if _parse_core_source_metadata(updated, manifest_path) != revisions:
        raise SourceMetadataError(
            "source metadata update did not round-trip through TOML parsing"
        )
    return updated


def _replace_binding_metadata_value(
    manifest_text: str, manifest_path: Path, revision: str
) -> str:
    lines = manifest_text.splitlines(keepends=True)
    section_header = f"[{BINDING_SOURCE_METADATA_SECTION}]"
    try:
        section_start = next(
            index for index, line in enumerate(lines) if line.strip() == section_header
        )
    except StopIteration as error:
        raise SourceMetadataError(
            f"missing {section_header} in {manifest_path}"
        ) from error

    section_end = next(
        (
            index
            for index in range(section_start + 1, len(lines))
            if lines[index].lstrip().startswith("[")
        ),
        len(lines),
    )
    assignment = re.compile(r"^(\s*)([A-Za-z0-9_-]+)(\s*=\s*).*$")
    found = False
    for index in range(section_start + 1, section_end):
        match = assignment.match(lines[index])
        if match is None or match.group(2) != BINDING_SOURCE_METADATA_KEY:
            continue
        newline = "\n" if lines[index].endswith("\n") else ""
        lines[index] = (
            f'{match.group(1)}{BINDING_SOURCE_METADATA_KEY}{match.group(3)}'
            f'"{revision}"{newline}'
        )
        found = True

    if not found:
        raise SourceMetadataError(
            f"could not update {BINDING_SOURCE_METADATA_KEY} in {manifest_path}"
        )
    updated = "".join(lines)
    if _parse_binding_source_metadata(updated, manifest_path) != revision:
        raise SourceMetadataError(
            "binding source metadata update did not round-trip through TOML parsing"
        )
    return updated


def _atomic_write_text(path: Path, content: str) -> None:
    mode = stat.S_IMODE(path.stat().st_mode)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(temporary_path, mode)
        os.replace(temporary_path, path)
    finally:
        temporary_path.unlink(missing_ok=True)


def update_core_source_metadata(
    repo_root: Path, *, dry_run: bool = False
) -> MetadataUpdate:
    """Synchronize the manifest with clean, exact vendored source worktrees."""
    manifest_path = repo_root / CORE_MANIFEST_PATH
    previous = read_core_source_metadata(manifest_path)
    revisions = inspect_core_sources(repo_root)
    changed = previous != revisions
    if not changed or dry_run:
        return MetadataUpdate(previous, revisions, changed, False)

    try:
        manifest_text = manifest_path.read_text(encoding="utf-8")
    except OSError as error:
        raise SourceMetadataError(f"could not read {manifest_path}: {error}") from error
    updated = _replace_metadata_values(manifest_text, manifest_path, revisions)
    try:
        _atomic_write_text(manifest_path, updated)
    except OSError as error:
        raise SourceMetadataError(f"could not update {manifest_path}: {error}") from error
    return MetadataUpdate(previous, revisions, True, True)


def update_binding_source_metadata(
    repo_root: Path, spec: BindingSourceSpec, *, dry_run: bool = False
) -> MetadataUpdate:
    """Synchronize only the manifest owned by one binding source."""
    manifest_path = repo_root / spec.manifest_path
    previous_revision = read_binding_source_metadata(manifest_path)
    revision = inspect_binding_source(repo_root, spec)
    changed = previous_revision != revision
    previous = {BINDING_SOURCE_METADATA_KEY: previous_revision}
    revisions = {BINDING_SOURCE_METADATA_KEY: revision}
    if not changed or dry_run:
        return MetadataUpdate(previous, revisions, changed, False)

    try:
        manifest_text = manifest_path.read_text(encoding="utf-8")
    except OSError as error:
        raise SourceMetadataError(f"could not read {manifest_path}: {error}") from error
    updated = _replace_binding_metadata_value(manifest_text, manifest_path, revision)
    try:
        _atomic_write_text(manifest_path, updated)
    except OSError as error:
        raise SourceMetadataError(f"could not update {manifest_path}: {error}") from error
    return MetadataUpdate(previous, revisions, True, True)


def update_all_binding_source_metadata(
    repo_root: Path, *, dry_run: bool = False
) -> dict[str, MetadataUpdate]:
    """Synchronize every crate-local binding source manifest independently."""
    return {
        spec.crate_name: update_binding_source_metadata(
            repo_root, spec, dry_run=dry_run
        )
        for spec in BINDING_SOURCE_SPECS
    }


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Manage exact Dear ImGui vendored source provenance"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    verify_parser = subparsers.add_parser(
        "verify", help="Verify manifest metadata against clean vendored worktrees"
    )
    verify_parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (defaults to the parent of tools/)",
    )
    binding_parser = subparsers.add_parser(
        "verify-bindings",
        help="Verify Test Engine and extension binding source provenance",
    )
    binding_parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (defaults to the parent of tools/)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        if args.command == "verify":
            revisions = verify_core_source_metadata(args.repo_root.resolve())
        elif args.command == "verify-bindings":
            binding_revisions = verify_all_binding_source_metadata(
                args.repo_root.resolve()
            )
        else:
            raise AssertionError(f"unhandled command: {args.command}")
    except SourceMetadataError as error:
        for message in error.errors:
            print(f"error: {message}", file=sys.stderr)
        return 1
    if args.command == "verify":
        for source in CORE_SOURCE_SPECS:
            print(f"{source.metadata_key}={revisions[source.metadata_key]}")
    else:
        for spec in BINDING_SOURCE_SPECS:
            print(f"{spec.crate_name}={binding_revisions[spec.crate_name]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
