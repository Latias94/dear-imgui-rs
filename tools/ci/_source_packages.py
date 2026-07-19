"""Create and consume publishable source packages from a clean clone."""

from __future__ import annotations

import json
import os
from collections.abc import Sequence
from pathlib import Path, PurePosixPath

from _archive import (
    PackageRecord,
    require_file,
    safe_extract_tar,
    verify_core_sys_archive,
    verify_source_archives,
)
from _prebuilt import build_host_prebuilt_packages, verify_prebuilt_packages
from _process import environment, github_group, run
from _submodules import PACKAGE_NESTED_SUBMODULES
from _verification import VerificationError, temporary_workspace
from release_metadata import (
    PUBLISH_ORDER,
    load_workspace_metadata,
    validate_publish_order,
    validate_release_workspace,
)
from source_metadata import read_core_source_metadata


WORKSPACE_ROOT = Path(__file__).resolve().parents[2]


def _git_submodule_declarations(repo: Path) -> dict[str, str]:
    result = run(
        (
            "git",
            "-C",
            repo,
            "config",
            "-f",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.path$",
        ),
        capture_output=True,
        accepted_returncodes=(0, 1),
    )
    declarations = {}
    for line in (result.stdout or "").splitlines():
        try:
            key, relative_path = line.split(None, 1)
        except ValueError as error:
            raise VerificationError(
                f"invalid .gitmodules entry in {repo}: {line!r}"
            ) from error
        prefix = "submodule."
        suffix = ".path"
        if not key.startswith(prefix) or not key.endswith(suffix):
            raise VerificationError(f"unexpected .gitmodules key in {repo}: {key!r}")
        name = key[len(prefix) : -len(suffix)]
        if relative_path in declarations:
            raise VerificationError(
                f"duplicate submodule path in {repo}: {relative_path}"
            )
        declarations[relative_path] = name
    return declarations


def _is_git_worktree(path: Path) -> bool:
    result = run(
        ("git", "-C", path, "rev-parse", "--git-dir"),
        capture_output=True,
        accepted_returncodes=None,
    )
    return result.returncode == 0


def _configure_local_submodule(
    source_root: Path,
    clone_root: Path,
    relative_path: PurePosixPath | str,
    name: str,
) -> None:
    source_path = source_root.joinpath(PurePosixPath(relative_path))
    if _is_git_worktree(source_path):
        run(
            (
                "git",
                "-C",
                clone_root,
                "config",
                f"submodule.{name}.url",
                source_path,
            )
        )


def initialize_package_submodules(source_root: Path, package_workspace: Path) -> None:
    """Initialize exactly the top-level and nested sources needed by packaging."""
    for relative_path, name in _git_submodule_declarations(package_workspace).items():
        _configure_local_submodule(
            source_root,
            package_workspace,
            PurePosixPath(relative_path),
            name,
        )
    run(
        (
            "git",
            "-C",
            package_workspace,
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
        )
    )

    for nested in PACKAGE_NESTED_SUBMODULES:
        source_parent = source_root.joinpath(nested.parent)
        clone_parent = package_workspace.joinpath(nested.parent)
        nested_path = nested.path.as_posix()
        declarations = _git_submodule_declarations(clone_parent)
        try:
            nested_name = declarations[nested_path]
        except KeyError as error:
            raise VerificationError(
                f"nested submodule {nested.parent}/{nested.path} is not declared"
            ) from error
        _configure_local_submodule(
            source_parent,
            clone_parent,
            nested.path,
            nested_name,
        )
        run(
            (
                "git",
                "-C",
                clone_parent,
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "update",
                "--init",
                nested.path.as_posix(),
            )
        )


def _git_status(repo: Path, *, include_all_untracked: bool) -> str:
    command = [
        "git",
        "-C",
        os.fspath(repo),
        "status",
        "--porcelain=v1",
    ]
    if include_all_untracked:
        command.append("--untracked-files=all")
    command.append("--ignore-submodules=none")
    return run(command, capture_output=True).stdout or ""


def _require_clean_source_checkout(repo: Path) -> None:
    status = _git_status(repo, include_all_untracked=True)
    if status:
        print("::error::package verification requires a clean source checkout")
        run(("git", "-C", repo, "status", "--short", "--ignore-submodules=none"))
        raise VerificationError("package verification requires a clean source checkout")


def _require_clean_package_workspace(repo: Path, message: str) -> None:
    status = _git_status(repo, include_all_untracked=False)
    if status:
        print(f"::error::{message}")
        run(("git", "-C", repo, "status", "--short", "--ignore-submodules=none"))
        raise VerificationError(message)


def _validate_release_packages(repo_root: Path) -> tuple[PackageRecord, ...]:
    metadata = load_workspace_metadata(repo_root)
    errors = [
        *validate_release_workspace(metadata),
        *validate_publish_order(metadata, PUBLISH_ORDER, repo_root),
    ]
    if errors:
        detail = "\n".join(f"  - {error}" for error in errors)
        raise VerificationError(f"release workspace validation failed:\n{detail}")
    return tuple(
        PackageRecord(name, Path(path), metadata.package(name).version)
        for name, path in PUBLISH_ORDER
    )


def _package_by_name(packages: Sequence[PackageRecord], name: str) -> PackageRecord:
    for package in packages:
        if package.name == name:
            return package
    raise VerificationError(f"release package is missing from publish order: {name}")


def cargo_path_patch(package_name: str, path: Path) -> str:
    """Render a Cargo --config path patch with portable TOML string escaping."""
    return (
        f"patch.crates-io.{package_name}.path={json.dumps(os.fspath(path.resolve()))}"
    )


def _commit_lockfile_if_changed(
    repo: Path, commit_message: str, dirty_error: str
) -> None:
    run(("git", "-C", repo, "add", "Cargo.lock"))
    diff = run(
        ("git", "-C", repo, "diff", "--cached", "--quiet"),
        accepted_returncodes=(0, 1),
    )
    if diff.returncode == 1:
        run(
            (
                "git",
                "-C",
                repo,
                "-c",
                "user.name=Dear ImGui CI",
                "-c",
                "user.email=ci@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                commit_message,
            ),
            quiet_stdout=True,
        )
    _require_clean_package_workspace(repo, dirty_error)


def _cargo_patch_arguments(
    package_workspace: Path, packages: Sequence[PackageRecord]
) -> list[str]:
    arguments = []
    for package in packages:
        arguments.extend(
            (
                "--config",
                cargo_path_patch(package.name, package_workspace / package.path),
            )
        )
    return arguments


def host_target() -> str:
    """Return the single host target reported by rustc."""
    result = run(("rustc", "-vV"), capture_output=True)
    hosts = [
        line.removeprefix("host: ")
        for line in (result.stdout or "").splitlines()
        if line.startswith("host: ")
    ]
    if len(hosts) != 1 or not hosts[0]:
        raise VerificationError("rustc -vV did not report exactly one host target")
    return hosts[0]


def verify_packaged_core(workspace_root: Path = WORKSPACE_ROOT) -> None:
    """Run the complete source-package and host-prebuilt release gate."""
    workspace_root = workspace_root.resolve()
    _require_clean_source_checkout(workspace_root)
    candidate_result = run(
        ("git", "-C", workspace_root, "rev-parse", "HEAD"), capture_output=True
    )
    candidate_sha = (candidate_result.stdout or "").strip()
    packages = _validate_release_packages(workspace_root)
    with temporary_workspace("dear-imgui-package.") as work_dir:
        target_dir = work_dir / "target"
        package_workspace = work_dir / "repository"
        run(
            (
                "git",
                "clone",
                "--quiet",
                "--local",
                "--no-hardlinks",
                workspace_root,
                package_workspace,
            )
        )
        initialize_package_submodules(workspace_root, package_workspace)

        helper = _package_by_name(packages, "dear-imgui-build-support")
        core_sys = _package_by_name(packages, "dear-imgui-sys")
        package_archive_dir = target_dir / "package"

        with github_group("Package dear-imgui-build-support"):
            run(
                ("cargo", "package", "-p", helper.name, "--locked"),
                cwd=package_workspace,
                env=environment({"CARGO_TARGET_DIR": target_dir}),
            )
        helper_archive = package_archive_dir / f"{helper.name}-{helper.version}.crate"
        require_file(helper_archive, "dear-imgui-build-support source archive")
        helper_destination = work_dir / "helper"
        helper_members = safe_extract_tar(helper_archive, helper_destination)
        helper_path = helper_destination / f"{helper.name}-{helper.version}"
        require_file(helper_path / "Cargo.toml", "unpacked build helper manifest")
        helper_patch = cargo_path_patch(helper.name, helper_path)

        run(
            (
                "cargo",
                "metadata",
                "--format-version",
                "1",
                "--config",
                helper_patch,
            ),
            cwd=package_workspace,
            quiet_stdout=True,
        )
        _commit_lockfile_if_changed(
            package_workspace,
            "ci: lock packaged build helper patch",
            "temporary package workspace is not clean",
        )

        with github_group("Package dear-imgui-sys with the packaged build helper"):
            run(
                (
                    "cargo",
                    "package",
                    "-p",
                    core_sys.name,
                    "--locked",
                    "--config",
                    helper_patch,
                ),
                cwd=package_workspace,
                env=environment({"CARGO_TARGET_DIR": target_dir}),
            )
        sys_archive = package_archive_dir / f"{core_sys.name}-{core_sys.version}.crate"
        require_file(sys_archive, "dear-imgui-sys source archive")
        sys_path, sys_members = verify_core_sys_archive(
            sys_archive, core_sys.version, work_dir
        )

        expected_sources = read_core_source_metadata(
            package_workspace / "dear-imgui-sys" / "Cargo.toml"
        )
        packaged_sources = read_core_source_metadata(sys_path / "Cargo.toml")
        if packaged_sources != expected_sources:
            raise VerificationError(
                "packaged [package.metadata.dear-imgui-sources] differs from the "
                f"source manifest: expected {expected_sources}, found {packaged_sources}"
            )

        with github_group("Consume unpacked dear-imgui-sys offline"):
            run(
                (
                    "cargo",
                    "check",
                    "--manifest-path",
                    sys_path / "Cargo.toml",
                    "--offline",
                    "--locked",
                    "--config",
                    helper_patch,
                ),
                env=environment({"CARGO_TARGET_DIR": target_dir / "offline-consumer"}),
            )

        cargo_patch_args = _cargo_patch_arguments(package_workspace, packages)
        run(
            (
                "cargo",
                "metadata",
                "--quiet",
                "--format-version",
                "1",
                *cargo_patch_args,
            ),
            cwd=package_workspace,
            quiet_stdout=True,
        )
        _commit_lockfile_if_changed(
            package_workspace,
            "ci: lock unpublished workspace package patches",
            "temporary package workspace is not clean after locking patches",
        )
        run(
            ("cargo", "fetch", "--quiet", "--locked", *cargo_patch_args),
            cwd=package_workspace,
        )

        with github_group("Create every publishable workspace source archive"):
            for package in packages:
                if package.name not in (helper.name, core_sys.name):
                    print(f"Packaging source archive: {package.name}")
                    run(
                        (
                            "cargo",
                            "package",
                            "-p",
                            package.name,
                            "--quiet",
                            "--offline",
                            "--locked",
                            *cargo_patch_args,
                        ),
                        cwd=package_workspace,
                        env=environment({"CARGO_TARGET_DIR": target_dir}),
                    )
                require_file(
                    package_archive_dir / f"{package.name}-{package.version}.crate",
                    f"source archive for {package.name}",
                )

        verify_source_archives(
            package_archive_dir,
            packages,
            known_archive_members={
                helper_archive: helper_members,
                sys_archive: sys_members,
            },
        )
        native_package_dir = work_dir / "native-packages"
        native_package_dir.mkdir()
        native_crt = build_host_prebuilt_packages(
            package_workspace, target_dir, native_package_dir, candidate_sha
        )
        verify_prebuilt_packages(
            native_package_dir,
            host_target(),
            candidate_sha,
            crt=native_crt,
            source_root=package_workspace,
            profile_scope="base",
        )
    print(
        "Verified packaged core consumers and "
        f"{len(packages)} publishable source archives."
    )
