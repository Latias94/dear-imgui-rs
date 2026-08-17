"""Locked Cargo workspace metadata used by release tooling.

Cargo resolves ``version.workspace = true`` and inherited workspace dependencies
for us.  Release scripts therefore consume one locked metadata snapshot instead
of reparsing individual manifests with subtly different rules.
"""

from __future__ import annotations

import json
import re
import subprocess
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence


PUBLISH_ORDER = [
    ("dear-imgui-build-support", "tools/build-support"),
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-imgui-rs", "dear-imgui"),
    ("dear-imgui-winit", "backends/dear-imgui-winit"),
    ("dear-imgui-sdl3", "backends/dear-imgui-sdl3"),
    ("dear-imgui-wgpu", "backends/dear-imgui-wgpu"),
    ("dear-imgui-glow", "backends/dear-imgui-glow"),
    ("dear-imgui-ash", "backends/dear-imgui-ash"),
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-node-editor-sys", "extensions/dear-node-editor-sys"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
    ("dear-imgui-cte-sys", "extensions/dear-imgui-cte-sys"),
    ("dear-imgui-test-engine-sys", "extensions/dear-imgui-test-engine-sys"),
    ("dear-implot", "extensions/dear-implot"),
    ("dear-imnodes", "extensions/dear-imnodes"),
    ("dear-node-editor", "extensions/dear-node-editor"),
    ("dear-imguizmo", "extensions/dear-imguizmo"),
    ("dear-implot3d", "extensions/dear-implot3d"),
    ("dear-imguizmo-quat", "extensions/dear-imguizmo-quat"),
    ("dear-imgui-cte", "extensions/dear-imgui-cte"),
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


def _duplicates(values: Sequence[str]) -> list[str]:
    return sorted(value for value, count in Counter(values).items() if count > 1)


class MetadataError(ValueError):
    """Raised when Cargo metadata cannot describe a valid release workspace."""


@dataclass(frozen=True)
class PrivatePackagePolicy:
    """One private workspace package declared by the release policy."""

    name: str
    relative_path: Path
    version: str


@dataclass(frozen=True)
class ReleasePolicy:
    """Release topology policy read from workspace Cargo metadata."""

    core_package: str
    private_packages: tuple[PrivatePackagePolicy, ...]

    @property
    def private_by_name(self) -> dict[str, PrivatePackagePolicy]:
        return {package.name: package for package in self.private_packages}


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
    release_policy: ReleasePolicy
    workspace_root: Path

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> WorkspaceMetadata:
        """Build a validated workspace view from Cargo metadata JSON."""
        try:
            workspace_members = set(payload["workspace_members"])
            raw_packages = payload["packages"]
            workspace_root = Path(payload["workspace_root"])
        except (KeyError, TypeError) as error:
            raise MetadataError("cargo metadata is missing workspace package data") from error
        release_policy = _release_policy_from_json(payload.get("metadata"))

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
        duplicate_names = _duplicates(names)
        if duplicate_names:
            raise MetadataError(
                "workspace package names must be unique: " + ", ".join(duplicate_names)
            )

        return cls(tuple(packages), release_policy, workspace_root)

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
        return self.package(self.release_policy.core_package).version


def _release_policy_from_json(raw_metadata: Any) -> ReleasePolicy:
    if not isinstance(raw_metadata, Mapping):
        raise MetadataError(
            "cargo metadata is missing workspace.metadata.dear-imgui-release"
        )
    raw_policy = raw_metadata.get("dear-imgui-release")
    if not isinstance(raw_policy, Mapping):
        raise MetadataError(
            "cargo metadata is missing workspace.metadata.dear-imgui-release"
        )
    unknown_fields = sorted(set(raw_policy) - {"core-package", "private-packages"})
    if unknown_fields:
        raise MetadataError(
            "release policy contains unknown field(s): " + ", ".join(unknown_fields)
        )
    core_package = raw_policy.get("core-package")
    if not isinstance(core_package, str) or not core_package:
        raise MetadataError("release policy core-package must be a non-empty string")
    raw_private = raw_policy.get("private-packages")
    if not isinstance(raw_private, Mapping) or not raw_private:
        raise MetadataError("release policy private-packages must be a non-empty table")

    private_packages = []
    for name, raw_package in sorted(raw_private.items()):
        if not isinstance(name, str) or not name:
            raise MetadataError("release policy private package names must be non-empty")
        if not isinstance(raw_package, Mapping):
            raise MetadataError(f"private package policy {name} must be a table")
        unknown_package_fields = sorted(set(raw_package) - {"path", "version"})
        if unknown_package_fields:
            raise MetadataError(
                f"private package policy {name} contains unknown field(s): "
                + ", ".join(unknown_package_fields)
            )
        raw_path = raw_package.get("path")
        if not isinstance(raw_path, str) or not raw_path:
            raise MetadataError(f"private package policy {name} is missing path")
        relative_path = Path(raw_path)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            raise MetadataError(
                f"private package policy {name} path must stay within the workspace"
            )
        version = raw_package.get("version")
        if not isinstance(version, str) or SEMVER_RE.fullmatch(version) is None:
            raise MetadataError(
                f"private package policy {name} has an invalid semantic version"
            )
        private_packages.append(
            PrivatePackagePolicy(
                name=name,
                relative_path=relative_path,
                version=version,
            )
        )

    if core_package in raw_private:
        raise MetadataError(f"release core package {core_package} cannot be private")
    return ReleasePolicy(core_package, tuple(private_packages))


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
    policy = metadata.release_policy

    try:
        core_package = metadata.package(policy.core_package)
        release_version = core_package.version
        expected_internal_requirement(release_version)
    except MetadataError as error:
        return [str(error)]

    if not core_package.publishable:
        errors.append(f"release package {policy.core_package} must be publishable")

    private_packages = {package.name: package for package in metadata.private_packages}
    expected_private = policy.private_by_name
    expected_private_names = set(expected_private)
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
    for name, private_policy in expected_private.items():
        package = private_packages.get(name)
        if package is None:
            continue
        if package.version != private_policy.version:
            errors.append(
                f"private package {name} uses {package.version}; "
                f"expected {private_policy.version}"
            )
        expected_directory = (
            metadata.workspace_root / private_policy.relative_path
        ).resolve()
        if package.manifest_path.parent.resolve() != expected_directory:
            errors.append(
                f"private package {name} is at {package.manifest_path.parent}; "
                f"expected {expected_directory}"
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

    duplicate_names = _duplicates(ordered_names)
    if duplicate_names:
        errors.append("PUBLISH_ORDER repeats package(s): " + ", ".join(duplicate_names))
    duplicate_paths = _duplicates(ordered_paths)
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
