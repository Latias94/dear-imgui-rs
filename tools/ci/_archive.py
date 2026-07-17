"""Safe archive handling and source-package content verification."""

from __future__ import annotations

import os
import shutil
import tarfile
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath, PureWindowsPath

from _verification import VerificationError


@dataclass(frozen=True)
class PackageRecord:
    """A publishable workspace package in release order."""

    name: str
    path: Path
    version: str


REQUIRED_CORE_BINDINGS = (
    "src/bindings_pregenerated.rs",
    "src/bindings_pregenerated_windows.rs",
    "src/wasm_bindings_pregenerated.rs",
)
SYS_SENTINELS = {
    "dear-imgui-sys": (
        "src/platform_io_hooks.cpp",
        "third-party/cimgui/cimgui.cpp",
        "third-party/cimgui/imgui/imgui.cpp",
    ),
    "dear-implot-sys": (
        "third-party/cimplot/cimplot.cpp",
        "third-party/cimplot/implot/implot.cpp",
    ),
    "dear-imnodes-sys": (
        "shim/imnodes_extra.cpp",
        "third-party/cimnodes/cimnodes.cpp",
        "third-party/cimnodes/imnodes/imnodes.cpp",
    ),
    "dear-node-editor-sys": (
        "shim/node_editor_extra.cpp",
        "third-party/cimnodes_editor/cimnodes_editor.cpp",
        "third-party/cimnodes_editor/imgui-node-editor/imgui_node_editor.cpp",
    ),
    "dear-imguizmo-sys": (
        "third-party/cimguizmo/cimguizmo.cpp",
        "third-party/cimguizmo/ImGuizmo/src/ImGuizmo.cpp",
    ),
    "dear-implot3d-sys": (
        "third-party/cimplot3d/cimplot3d.cpp",
        "third-party/cimplot3d/implot3d/implot3d.cpp",
    ),
    "dear-imguizmo-quat-sys": (
        "third-party/cimguizmo_quat/cimguizmo_quat.cpp",
        "third-party/cimguizmo_quat/imGuIZMO.quat/imguizmo_quat/imguizmo_quat.cpp",
    ),
    "dear-imgui-test-engine-sys": (
        "shim/cimgui_test_engine.cpp",
        "third-party/imgui_test_engine/imgui_test_engine/imgui_te_engine.cpp",
    ),
}


def _normalized_archive_path(name: str, archive: Path) -> PurePosixPath:
    if "\\" in name:
        raise VerificationError(f"unsafe archive member in {archive}: {name!r}")
    path = PurePosixPath(name)
    windows_path = PureWindowsPath(name)
    if (
        path.is_absolute()
        or windows_path.drive
        or any(part == ".." for part in path.parts)
    ):
        raise VerificationError(f"unsafe archive member in {archive}: {name!r}")
    parts = tuple(part for part in path.parts if part not in ("", "."))
    if not parts:
        raise VerificationError(f"unsafe archive member in {archive}: {name!r}")
    return PurePosixPath(*parts)


def _validated_tar_members(
    package: tarfile.TarFile, archive: Path
) -> list[tuple[tarfile.TarInfo, PurePosixPath]]:
    validated = []
    for member in package.getmembers():
        path = _normalized_archive_path(member.name, archive)
        if member.issym() or member.islnk():
            raise VerificationError(
                f"unsafe archive link in {archive}: {member.name!r}"
            )
        if not member.isdir() and not member.isfile():
            raise VerificationError(
                f"unsupported archive member type in {archive}: {member.name!r}"
            )
        validated.append((member, path))
    return validated


def archive_member_names(archive: Path) -> set[str]:
    """Return normalized member names after validating the complete archive."""
    try:
        with tarfile.open(archive, "r:*") as package:
            return {
                path.as_posix()
                for _member, path in _validated_tar_members(package, archive)
            }
    except VerificationError:
        raise
    except (OSError, tarfile.TarError) as error:
        raise VerificationError(
            f"could not inspect archive {archive}: {error}"
        ) from error


def read_unique_root_file(archive: Path, name: str, *, mode: str = "r:*") -> bytes:
    """Read one regular root file after validating every archive member."""
    expected_path = PurePosixPath(name)
    if len(expected_path.parts) != 1 or expected_path.name != name:
        raise ValueError(f"root archive member name required, got {name!r}")
    try:
        with tarfile.open(archive, mode) as package:
            validated = _validated_tar_members(package, archive)
            matching = [
                member
                for member, path in validated
                if path == expected_path and member.isfile()
            ]
            if len(matching) != 1:
                raise VerificationError(
                    f"{archive} must contain exactly one root {name}"
                )
            extracted = package.extractfile(matching[0])
            if extracted is None:
                raise VerificationError(f"could not read {name} from {archive}")
            with extracted:
                return extracted.read()
    except VerificationError:
        raise
    except (OSError, tarfile.TarError) as error:
        raise VerificationError(
            f"could not inspect archive {archive}: {error}"
        ) from error


def safe_extract_tar(archive: Path, destination: Path) -> set[str]:
    """Extract regular files and directories without trusting paths or links."""
    destination.mkdir(parents=True, exist_ok=True)
    try:
        with tarfile.open(archive, "r:*") as package:
            members = _validated_tar_members(package, archive)
            member_names = {path.as_posix() for _member, path in members}
            for member, member_path in members:
                target = destination.joinpath(*member_path.parts)
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                source = package.extractfile(member)
                if source is None:
                    raise VerificationError(
                        f"could not read archive member {member.name!r} from {archive}"
                    )
                with source, target.open("wb") as output:
                    shutil.copyfileobj(source, output)
                os.chmod(target, member.mode & 0o777)
            return member_names
    except VerificationError:
        raise
    except (OSError, tarfile.TarError) as error:
        raise VerificationError(
            f"could not extract archive {archive}: {error}"
        ) from error


def require_file(path: Path, description: str) -> None:
    """Require one regular file with a release-gate-specific diagnostic."""
    if not path.is_file():
        raise VerificationError(f"missing {description}: {path}")


def verify_core_sys_archive(
    archive: Path, version: str, work_dir: Path
) -> tuple[Path, set[str]]:
    """Extract and audit the core sys source archive."""
    destination = work_dir / "sys"
    members = safe_extract_tar(archive, destination)
    archive_root = f"dear-imgui-sys-{version}"
    for required_path in REQUIRED_CORE_BINDINGS:
        expected = f"{archive_root}/{required_path}"
        if expected not in members:
            raise VerificationError(f"packaged dear-imgui-sys is missing {expected}")
    git_entries = sorted(
        member for member in members if ".git" in PurePosixPath(member).parts
    )
    if git_entries:
        raise VerificationError(
            "packaged dear-imgui-sys contains a .git entry: " + ", ".join(git_entries)
        )
    sys_path = destination / archive_root
    require_file(sys_path / "Cargo.toml", "unpacked dear-imgui-sys manifest")
    return sys_path, members


def verify_source_archives(
    archive_dir: Path,
    packages: Sequence[PackageRecord],
    *,
    sys_sentinels: Mapping[str, Sequence[str]] = SYS_SENTINELS,
    known_archive_members: Mapping[Path, set[str]] | None = None,
) -> None:
    """Audit every source archive and all native-source sentinel contracts."""
    expected_sys_packages = {
        package.name for package in packages if package.name.endswith("-sys")
    }
    if set(sys_sentinels) != expected_sys_packages:
        raise VerificationError(
            "native source sentinel map differs from publishable sys crates: "
            f"expected {sorted(expected_sys_packages)}, "
            f"found {sorted(sys_sentinels)}"
        )
    known_archive_members = known_archive_members or {}
    for package in packages:
        archive = archive_dir / f"{package.name}-{package.version}.crate"
        require_file(archive, f"source archive for {package.name}")
        members = known_archive_members.get(archive)
        if members is None:
            members = archive_member_names(archive)
        git_entries = sorted(
            member for member in members if ".git" in PurePosixPath(member).parts
        )
        if git_entries:
            raise VerificationError(f"{archive} contains a .git entry")
        root = f"{package.name}-{package.version}"
        for sentinel in sys_sentinels.get(package.name, ()):
            required = f"{root}/{sentinel}"
            if required not in members:
                raise VerificationError(
                    f"{archive} is missing native source sentinel {required}"
                )
    print(
        f"Verified {len(packages)} source archives and native source sentinels for "
        f"{len(sys_sentinels)} sys crates."
    )
