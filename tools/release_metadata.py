"""Locked Cargo workspace metadata used by release tooling.

Cargo resolves ``version.workspace = true`` and inherited workspace dependencies
for us.  Release scripts therefore consume one locked metadata snapshot instead
of reparsing individual manifests with subtly different rules.
"""

from __future__ import annotations

import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence


CORE_PACKAGE = "dear-imgui-rs"
EXPECTED_PUBLISHABLE_COUNT = 27
PRIVATE_PACKAGE_VERSIONS = {
    "dear-imgui-examples": "0.1.0",
    "dear-imgui-web-demo": "0.1.0",
    "xtask": "0.1.0",
}
PUBLISH_ORDER = [
    ("dear-imgui-build-support", "tools/build-support"),
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-imgui-rs", "dear-imgui"),
    ("dear-imgui-winit", "backends/dear-imgui-winit"),
    ("dear-imgui-wgpu", "backends/dear-imgui-wgpu"),
    ("dear-imgui-glow", "backends/dear-imgui-glow"),
    ("dear-imgui-ash", "backends/dear-imgui-ash"),
    ("dear-imgui-sdl3", "backends/dear-imgui-sdl3"),
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-node-editor-sys", "extensions/dear-node-editor-sys"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
    ("dear-imgui-test-engine-sys", "extensions/dear-imgui-test-engine-sys"),
    ("dear-implot", "extensions/dear-implot"),
    ("dear-imnodes", "extensions/dear-imnodes"),
    ("dear-node-editor", "extensions/dear-node-editor"),
    ("dear-imguizmo", "extensions/dear-imguizmo"),
    ("dear-implot3d", "extensions/dear-implot3d"),
    ("dear-imguizmo-quat", "extensions/dear-imguizmo-quat"),
    ("dear-imgui-test-engine", "extensions/dear-imgui-test-engine"),
    ("dear-file-browser", "extensions/dear-file-browser"),
    ("dear-imgui-reflect-derive", "extensions/dear-imgui-reflect-derive"),
    ("dear-imgui-reflect", "extensions/dear-imgui-reflect"),
    ("dear-imgui-bevy", "backends/dear-imgui-bevy"),
    ("dear-app", "dear-app"),
]
SEMVER_RE = re.compile(
    r"^(?P<major>0|[1-9]\d*)\."
    r"(?P<minor>0|[1-9]\d*)\."
    r"(?P<patch>0|[1-9]\d*)"
    r"(?P<prerelease>-[0-9A-Za-z.-]+)?"
    r"(?P<build>\+[0-9A-Za-z.-]+)?$"
)
METADATA_COMMAND = (
    "cargo",
    "metadata",
    "--locked",
    "--no-deps",
    "--format-version",
    "1",
)


class MetadataError(ValueError):
    """Raised when Cargo metadata cannot describe a valid release workspace."""


@dataclass(frozen=True)
class WorkspaceDependency:
    """A dependency entry resolved by Cargo metadata."""

    name: str
    requirement: str
    path: Path | None
    kind: str | None


@dataclass(frozen=True)
class WorkspacePackage:
    """A workspace package relevant to release automation."""

    package_id: str
    name: str
    version: str
    manifest_path: Path
    publish_registries: tuple[str, ...] | None
    dependencies: tuple[WorkspaceDependency, ...]

    @property
    def publishable(self) -> bool:
        return self.publish_registries != ()

    def relative_directory(self, repo_root: Path) -> str:
        """Return the package directory using repository-relative POSIX syntax."""
        return self.manifest_path.parent.resolve().relative_to(
            repo_root.resolve()
        ).as_posix()


@dataclass(frozen=True)
class WorkspaceMetadata:
    """The workspace-member subset of one ``cargo metadata --locked`` result."""

    packages: tuple[WorkspacePackage, ...]

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> WorkspaceMetadata:
        """Build a validated workspace view from Cargo metadata JSON."""
        try:
            workspace_members = set(payload["workspace_members"])
            raw_packages = payload["packages"]
        except (KeyError, TypeError) as error:
            raise MetadataError("cargo metadata is missing workspace package data") from error

        packages = []
        for raw in raw_packages:
            if raw.get("id") not in workspace_members:
                continue
            try:
                dependencies = tuple(
                    WorkspaceDependency(
                        name=dependency["name"],
                        requirement=dependency["req"],
                        path=(
                            Path(dependency["path"])
                            if dependency.get("path") is not None
                            else None
                        ),
                        kind=dependency.get("kind"),
                    )
                    for dependency in raw.get("dependencies", ())
                )
                raw_publish = raw.get("publish")
                if raw_publish is None:
                    publish_registries = None
                elif isinstance(raw_publish, list) and all(
                    isinstance(registry, str) for registry in raw_publish
                ):
                    publish_registries = tuple(raw_publish)
                else:
                    raise TypeError("invalid Cargo publish policy")
                packages.append(
                    WorkspacePackage(
                        package_id=raw["id"],
                        name=raw["name"],
                        version=raw["version"],
                        manifest_path=Path(raw["manifest_path"]),
                        publish_registries=publish_registries,
                        dependencies=dependencies,
                    )
                )
            except (KeyError, TypeError) as error:
                raise MetadataError("cargo metadata contains a malformed package") from error

        if len(packages) != len(workspace_members):
            raise MetadataError(
                "cargo metadata did not return every workspace member "
                f"({len(packages)} of {len(workspace_members)})"
            )

        names = [package.name for package in packages]
        duplicate_names = sorted(
            name for name in set(names) if names.count(name) > 1
        )
        if duplicate_names:
            raise MetadataError(
                "workspace package names must be unique: " + ", ".join(duplicate_names)
            )

        return cls(tuple(packages))

    @property
    def by_name(self) -> dict[str, WorkspacePackage]:
        return {package.name: package for package in self.packages}

    @property
    def publishable_packages(self) -> tuple[WorkspacePackage, ...]:
        return tuple(package for package in self.packages if package.publishable)

    @property
    def private_packages(self) -> tuple[WorkspacePackage, ...]:
        return tuple(package for package in self.packages if not package.publishable)

    def package(self, name: str) -> WorkspacePackage:
        try:
            return self.by_name[name]
        except KeyError as error:
            raise MetadataError(f"workspace package not found: {name}") from error

    @property
    def release_version(self) -> str:
        return self.package(CORE_PACKAGE).version


CommandRunner = Callable[..., subprocess.CompletedProcess[str]]


def load_workspace_metadata(
    repo_root: Path,
    *,
    runner: CommandRunner = subprocess.run,
) -> WorkspaceMetadata:
    """Run Cargo once and return its locked workspace metadata."""
    try:
        result = runner(
            list(METADATA_COMMAND),
            cwd=repo_root,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except OSError as error:
        raise MetadataError(f"could not run cargo metadata: {error}") from error

    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "unknown Cargo error"
        raise MetadataError(f"cargo metadata --locked failed: {detail}")

    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise MetadataError(f"cargo metadata returned invalid JSON: {error}") from error
    return WorkspaceMetadata.from_json(payload)


def expected_internal_requirement(version: str) -> str:
    """Return the canonical Cargo requirement for an internal release edge."""
    match = SEMVER_RE.fullmatch(version)
    if match is None:
        raise MetadataError(f"workspace package has an invalid semantic version: {version}")
    if match.group("build") is not None:
        raise MetadataError(
            f"workspace release versions cannot contain build metadata: {version}"
        )
    if match.group("prerelease") is not None:
        return f"={version}"
    return f"^{match.group('major')}.{match.group('minor')}"


def validate_release_workspace(metadata: WorkspaceMetadata) -> list[str]:
    """Validate release versions and every internal path requirement."""
    errors = []
    packages = metadata.by_name

    try:
        core_package = metadata.package(CORE_PACKAGE)
        release_version = core_package.version
        expected_internal_requirement(release_version)
    except MetadataError as error:
        return [str(error)]

    if not core_package.publishable:
        errors.append(f"release package {CORE_PACKAGE} must be publishable")

    publishable_count = len(metadata.publishable_packages)
    if publishable_count != EXPECTED_PUBLISHABLE_COUNT:
        errors.append(
            f"workspace has {publishable_count} publishable packages; "
            f"expected {EXPECTED_PUBLISHABLE_COUNT}"
        )

    private_packages = {package.name: package for package in metadata.private_packages}
    expected_private_names = set(PRIVATE_PACKAGE_VERSIONS)
    actual_private_names = set(private_packages)
    missing_private = sorted(expected_private_names - actual_private_names)
    unexpected_private = sorted(actual_private_names - expected_private_names)
    if missing_private:
        errors.append("workspace private package set is missing: " + ", ".join(missing_private))
    if unexpected_private:
        errors.append(
            "workspace has unexpected private package(s): "
            + ", ".join(unexpected_private)
        )
    for name, expected_version in PRIVATE_PACKAGE_VERSIONS.items():
        package = private_packages.get(name)
        if package is not None and package.version != expected_version:
            errors.append(
                f"private package {name} uses {package.version}; "
                f"expected {expected_version}"
            )

    for package in sorted(metadata.publishable_packages, key=lambda item: item.name):
        if package.publish_registries not in (None, ("crates-io",)):
            errors.append(
                f"publishable package {package.name} uses unsupported registry policy "
                f"{list(package.publish_registries or ())}; release tooling only targets "
                "crates.io"
            )
        if package.version != release_version:
            errors.append(
                f"publishable package {package.name} uses {package.version}; "
                f"expected release version {release_version}"
            )

    for package in sorted(metadata.packages, key=lambda item: item.name):
        for dependency in package.dependencies:
            if dependency.name not in packages:
                continue
            target = packages[dependency.name]
            if dependency.path is None:
                errors.append(
                    f"{package.name} internal dependency {dependency.name} must use "
                    f"the local workspace path {target.manifest_path.parent}"
                )
            elif dependency.path.resolve() != target.manifest_path.parent.resolve():
                errors.append(
                    f"{package.name} path dependency {dependency.name} points to "
                    f"{dependency.path}, expected {target.manifest_path.parent}"
                )
            if package.publishable and not target.publishable:
                errors.append(
                    f"publishable package {package.name} depends on private workspace "
                    f"package {target.name}"
                )
            try:
                expected = expected_internal_requirement(target.version)
            except MetadataError as error:
                errors.append(str(error))
                continue
            if dependency.requirement != expected:
                kind = dependency.kind or "normal"
                errors.append(
                    f"{package.name} {kind} dependency {target.name} uses "
                    f"{dependency.requirement}; expected {expected} for {target.version}"
                )

    return errors


def validate_publish_order(
    metadata: WorkspaceMetadata,
    publish_order: Sequence[tuple[str, str]],
    repo_root: Path,
) -> list[str]:
    """Require the configured order to cover publishable packages exactly once."""
    errors = []
    ordered_names = [name for name, _path in publish_order]
    ordered_paths = [path for _name, path in publish_order]
    publishable = {package.name: package for package in metadata.publishable_packages}

    duplicate_names = sorted(
        name for name in set(ordered_names) if ordered_names.count(name) > 1
    )
    if duplicate_names:
        errors.append("PUBLISH_ORDER repeats package(s): " + ", ".join(duplicate_names))
    duplicate_paths = sorted(
        path for path in set(ordered_paths) if ordered_paths.count(path) > 1
    )
    if duplicate_paths:
        errors.append("PUBLISH_ORDER repeats path(s): " + ", ".join(duplicate_paths))

    configured_names = set(ordered_names)
    missing = sorted(set(publishable) - configured_names)
    extra = sorted(configured_names - set(publishable))
    if missing:
        errors.append("PUBLISH_ORDER is missing: " + ", ".join(missing))
    if extra:
        errors.append("PUBLISH_ORDER contains non-publishable/unknown: " + ", ".join(extra))

    for name, configured_path in publish_order:
        package = publishable.get(name)
        if package is None:
            continue
        try:
            actual_path = package.relative_directory(repo_root)
        except ValueError:
            errors.append(
                f"publishable package {name} is outside the repository: "
                f"{package.manifest_path}"
            )
            continue
        if configured_path != actual_path:
            errors.append(
                f"PUBLISH_ORDER path for {name} is {configured_path}; "
                f"metadata reports {actual_path}"
            )

    positions = {name: index for index, name in enumerate(ordered_names)}
    for package in metadata.publishable_packages:
        owner_position = positions.get(package.name)
        if owner_position is None:
            continue
        for dependency in package.dependencies:
            target = publishable.get(dependency.name)
            target_position = positions.get(dependency.name)
            if target is None or target_position is None:
                continue
            if target_position >= owner_position:
                errors.append(
                    f"PUBLISH_ORDER places {package.name} before internal dependency "
                    f"{target.name}"
                )

    return errors
