"""Composable Windows native dependency contracts for repository CI."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePath, PurePosixPath, PureWindowsPath
from typing import Callable, TypeAlias

from _process import CommandError, run


PathInput: TypeAlias = str | os.PathLike[str]
Runner: TypeAlias = Callable[..., subprocess.CompletedProcess[str]]

_MSVC_ARCHITECTURES = {
    "aarch64-pc-windows-msvc": "arm64",
    "i686-pc-windows-msvc": "x86",
    "thumbv7a-pc-windows-msvc": "arm",
    "x86_64-pc-windows-msvc": "x64",
}
_CRT_SUFFIXES = {"md": "-static-md", "mt": "-static"}
_ENVIRONMENT_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
_DLL_IMPORT = re.compile(r"^\s*DLL Name:\s*(.*?)\s*$", re.MULTILINE)
_FORBIDDEN_CPP_RUNTIME = "libstdc++-6.dll"


class WindowsNativeError(RuntimeError):
    """A Windows native CI contract could not be satisfied."""


class VcpkgRootError(WindowsNativeError):
    """No candidate contains a vcpkg root marker."""


class VcpkgStatusError(WindowsNativeError):
    """A vcpkg installation has no usable package status data."""


class MissingTestBinaryError(WindowsNativeError):
    """The MinGW test build produced no matching executable."""


class ForbiddenImportError(WindowsNativeError):
    """A MinGW executable dynamically imports the GNU C++ runtime."""

    def __init__(self, inspection: "ImportInspection") -> None:
        self.inspection = inspection
        binaries = ", ".join(
            evidence.binary.name for evidence in inspection.forbidden_evidence
        )
        super().__init__(
            f"forbidden import {_FORBIDDEN_CPP_RUNTIME!r} found in: {binaries}\n"
            f"{inspection.evidence_text.rstrip()}"
        )


@dataclass(frozen=True)
class VcpkgTriplet:
    """A supported Rust MSVC target mapped to its static vcpkg triplet."""

    rust_target: str
    crt: str
    architecture: str
    name: str

    @classmethod
    def from_target(cls, rust_target: str, crt: str) -> "VcpkgTriplet":
        """Resolve a Rust MSVC target and MD/MT runtime selection."""
        try:
            architecture = _MSVC_ARCHITECTURES[rust_target]
        except KeyError as error:
            supported = ", ".join(sorted(_MSVC_ARCHITECTURES))
            raise WindowsNativeError(
                f"unsupported MSVC target {rust_target!r}; expected one of: {supported}"
            ) from error
        try:
            suffix = _CRT_SUFFIXES[crt]
        except KeyError as error:
            raise WindowsNativeError(
                f"unsupported MSVC CRT {crt!r}; expected 'md' or 'mt'"
            ) from error
        return cls(
            rust_target=rust_target,
            crt=crt,
            architecture=architecture,
            name=f"{architecture}-windows{suffix}",
        )

    def package(self, name: str) -> str:
        """Attach this triplet to a vcpkg package name."""
        if not name or any(character.isspace() for character in name):
            raise WindowsNativeError(f"invalid vcpkg package name: {name!r}")
        return f"{name}:{self.name}"


@dataclass(frozen=True)
class VcpkgRootCandidate:
    """A possible vcpkg root and the input that supplied it."""

    path: Path
    source: str


@dataclass(frozen=True)
class VcpkgStatus:
    """Observed status inputs used by vcpkg-rs 0.2.x."""

    root: Path
    marker: Path
    status_file: Path
    updates_directory: Path
    status_bytes: int
    update_files: tuple[Path, ...]
    update_bytes: int
    needs_updates_directory: bool

    @property
    def has_status_data(self) -> bool:
        """Return whether either status representation contains data."""
        return self.status_bytes > 0 or self.update_bytes > 0


@dataclass(frozen=True)
class CommandContract:
    """A shell-free command and its owned working-directory contract."""

    arguments: tuple[str, ...]
    cwd: Path

    def execute(self, *, runner: Runner = run) -> subprocess.CompletedProcess[str]:
        """Execute the command while preserving runner errors and exit codes."""
        return runner(self.arguments, cwd=self.cwd)


@dataclass(frozen=True)
class Sdl3Consumer:
    """Files and command for the temporary SDL3 vcpkg consumer."""

    root: Path
    manifest: Path
    build_script: Path
    main_source: Path
    command: CommandContract


@dataclass(frozen=True)
class MinGwEnvironment:
    """Derived MinGW tool location and process PATH."""

    msys2_root: PurePath
    bin_directory: PurePath
    path: str
    path_separator: str

    @property
    def github_environment(self) -> tuple[tuple[str, str], ...]:
        """Return the environment assignment published between workflow steps."""
        return (("MINGW_BIN", os.fspath(self.bin_directory)),)

    @property
    def github_path(self) -> tuple[str, ...]:
        """Return the PATH entry published between workflow steps."""
        return (os.fspath(self.bin_directory),)

    def tool(self, executable: str) -> PurePath:
        """Return a tool path rooted in this MinGW installation."""
        return self.bin_directory / executable


@dataclass(frozen=True)
class ObjdumpEvidence:
    """Raw objdump evidence and parsed imports for one executable."""

    binary: Path
    command: tuple[str, ...]
    output: str
    imports: tuple[str, ...]

    @property
    def forbidden_imports(self) -> tuple[str, ...]:
        """Return forbidden imports using Windows case-insensitive matching."""
        return tuple(
            imported
            for imported in self.imports
            if imported.casefold() == _FORBIDDEN_CPP_RUNTIME.casefold()
        )


@dataclass(frozen=True)
class ImportInspection:
    """Stable objdump evidence for every matching MinGW test executable."""

    evidence: tuple[ObjdumpEvidence, ...]

    @property
    def forbidden_evidence(self) -> tuple[ObjdumpEvidence, ...]:
        """Return executable evidence containing a forbidden import."""
        return tuple(item for item in self.evidence if item.forbidden_imports)

    @property
    def evidence_text(self) -> str:
        """Render complete raw evidence without discarding objdump diagnostics."""
        sections = []
        for item in self.evidence:
            output = item.output
            if output and not output.endswith("\n"):
                output += "\n"
            sections.append(f"Checking imports for {item.binary}\n{output}")
        return "".join(sections)


def _looks_like_windows_path(value: str) -> bool:
    return bool(re.match(r"^(?:[A-Za-z]:[\\/]|\\\\)", value)) or "\\" in value


def _pure_path(value: PathInput) -> PurePath:
    if isinstance(value, PurePath):
        return value
    text = os.fspath(value)
    if _looks_like_windows_path(text):
        return PureWindowsPath(text)
    return PurePosixPath(text)


def _executable_parent(executable: PathInput) -> Path:
    text = os.fspath(executable)
    if _looks_like_windows_path(text):
        return Path(os.fspath(PureWindowsPath(text).parent))
    return Path(text).parent


def _candidate_key(path: Path) -> str:
    text = os.fspath(path).replace("\\", "/").rstrip("/") or "/"
    if _looks_like_windows_path(os.fspath(path)) or os.name == "nt":
        return text.casefold()
    return text


def vcpkg_root_candidates(
    environment: Mapping[str, str], vcpkg_executable: PathInput
) -> tuple[VcpkgRootCandidate, ...]:
    """Return ordered, de-duplicated roots from environment and executable."""
    raw_candidates = (
        ("VCPKG_ROOT", environment.get("VCPKG_ROOT", "")),
        (
            "VCPKG_INSTALLATION_ROOT",
            environment.get("VCPKG_INSTALLATION_ROOT", ""),
        ),
        ("vcpkg executable", os.fspath(_executable_parent(vcpkg_executable))),
    )
    candidates = []
    seen = set()
    for source, raw_path in raw_candidates:
        if not raw_path or not raw_path.strip():
            continue
        path = Path(raw_path)
        key = _candidate_key(path)
        if key in seen:
            continue
        seen.add(key)
        candidates.append(VcpkgRootCandidate(path=path, source=source))
    return tuple(candidates)


def resolve_vcpkg_root(
    candidates: Sequence[VcpkgRootCandidate],
) -> VcpkgRootCandidate:
    """Choose the first candidate containing the canonical root marker."""
    for candidate in candidates:
        if (candidate.path / ".vcpkg-root").is_file():
            return candidate
    attempted = ", ".join(
        f"{candidate.source}={candidate.path}" for candidate in candidates
    )
    if not attempted:
        attempted = "no candidates"
    raise VcpkgRootError(f"vcpkg root not found ({attempted})")


def locate_vcpkg_executable(
    *, which: Callable[[str], str | None] = shutil.which
) -> Path:
    """Find vcpkg without invoking a shell."""
    executable = which("vcpkg")
    if not executable:
        raise VcpkgRootError("vcpkg executable was not found on PATH")
    return Path(executable)


def inspect_vcpkg_status(root: PathInput) -> VcpkgStatus:
    """Inspect the marker and non-empty status data without mutating the root."""
    root_path = Path(root)
    marker = root_path / ".vcpkg-root"
    status_directory = root_path / "installed" / "vcpkg"
    status_file = status_directory / "status"
    updates_directory = status_directory / "updates"
    status_bytes = status_file.stat().st_size if status_file.is_file() else 0
    update_files = ()
    if updates_directory.is_dir():
        update_files = tuple(
            sorted(
                (path for path in updates_directory.iterdir() if path.is_file()),
                key=lambda path: (path.name.casefold(), path.name),
            )
        )
    update_bytes = sum(path.stat().st_size for path in update_files)
    return VcpkgStatus(
        root=root_path,
        marker=marker,
        status_file=status_file,
        updates_directory=updates_directory,
        status_bytes=status_bytes,
        update_files=update_files,
        update_bytes=update_bytes,
        needs_updates_directory=not updates_directory.is_dir(),
    )


def ensure_vcpkg_status_compatibility(root: PathInput) -> VcpkgStatus:
    """Create only the missing updates directory and require real status data."""
    status = inspect_vcpkg_status(root)
    if not status.marker.is_file():
        raise VcpkgStatusError(f"vcpkg root marker not found: {status.marker}")
    if status.needs_updates_directory:
        status.updates_directory.mkdir(parents=True, exist_ok=True)
        status = inspect_vcpkg_status(root)
    if not status.has_status_data:
        raise VcpkgStatusError(
            f"vcpkg status data is missing or empty under "
            f"{status.status_file.parent}"
        )
    return status


def install_vcpkg_packages(
    packages: Sequence[str],
    triplet: VcpkgTriplet,
    *,
    runner: Runner = run,
) -> subprocess.CompletedProcess[str]:
    """Install packages for one explicit triplet without shell interpolation."""
    if not packages:
        raise WindowsNativeError("at least one vcpkg package is required")
    command = ("vcpkg", "install", *(triplet.package(name) for name in packages))
    return runner(command)


def github_assignment_bytes(
    assignments: Mapping[str, PathInput] | Iterable[tuple[str, PathInput]],
) -> bytes:
    """Encode exact UTF-8 GitHub assignments with LF and no BOM."""
    items = assignments.items() if isinstance(assignments, Mapping) else assignments
    lines = []
    for name, raw_value in items:
        value = os.fspath(raw_value)
        if not _ENVIRONMENT_NAME.fullmatch(name):
            raise WindowsNativeError(f"invalid GitHub environment name: {name!r}")
        if any(character in value for character in ("\0", "\r", "\n")):
            raise WindowsNativeError(
                f"GitHub environment value for {name} must fit on one line"
            )
        lines.append(f"{name}={value}\n")
    return "".join(lines).encode("utf-8")


def github_path_bytes(paths: Iterable[PathInput]) -> bytes:
    """Encode exact UTF-8 GitHub PATH entries with LF and no BOM."""
    lines = []
    for raw_path in paths:
        path = os.fspath(raw_path)
        if not path or any(character in path for character in ("\0", "\r", "\n")):
            raise WindowsNativeError("GitHub PATH entries must be non-empty lines")
        lines.append(f"{path}\n")
    return "".join(lines).encode("utf-8")


def append_github_assignments(
    destination: PathInput,
    assignments: Mapping[str, PathInput] | Iterable[tuple[str, PathInput]],
) -> None:
    """Append assignments in binary mode so Python cannot rewrite newlines."""
    with Path(destination).open("ab") as output:
        output.write(github_assignment_bytes(assignments))


def append_github_paths(destination: PathInput, paths: Iterable[PathInput]) -> None:
    """Append PATH entries in binary mode so Python cannot add a BOM."""
    with Path(destination).open("ab") as output:
        output.write(github_path_bytes(paths))


def vcpkg_github_environment(
    root: PathInput, triplet: VcpkgTriplet, runner_temp: PathInput
) -> tuple[tuple[str, str], ...]:
    """Return the exact environment needed for vcpkg-rs fallback discovery."""
    missing_pkg_config = Path(runner_temp) / "missing-pkg-config.exe"
    return (
        ("VCPKG_ROOT", os.fspath(root)),
        ("VCPKGRS_TRIPLET", triplet.name),
        ("PKG_CONFIG", os.fspath(missing_pkg_config)),
        ("PKG_CONFIG_PATH", ""),
    )


def _toml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def _write_utf8_lf(path: Path, content: str) -> None:
    normalized = content.replace("\r\n", "\n").replace("\r", "\n")
    if not normalized.endswith("\n"):
        normalized += "\n"
    path.write_bytes(normalized.encode("utf-8"))


def create_sdl3_vcpkg_consumer(
    workspace: PathInput, repository_root: PathInput
) -> Sdl3Consumer:
    """Write a caller-owned temporary consumer without deleting prior paths."""
    root = Path(workspace)
    source_directory = root / "src"
    source_directory.mkdir(parents=True, exist_ok=True)
    manifest = root / "Cargo.toml"
    build_script = root / "build.rs"
    main_source = source_directory / "main.rs"
    build_support = _pure_path(repository_root) / "tools" / "build-support"
    manifest_text = f"""[package]
name = "dear-imgui-native-deps-smoke"
version = "0.0.0"
edition = "2024"

[workspace]

[build-dependencies]
build-support = {{ package = "dear-imgui-build-support", path = {_toml_string(os.fspath(build_support))}, features = ["vcpkg"] }}
"""
    build_script_text = """fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let found = build_support::find_sdl3_include_paths(build_support::Sdl3SearchConfig {
        out_dir: &out_dir,
        target_os: "windows",
        use_pkg_config: false,
        use_vcpkg: true,
    })
    .expect("SDL3 headers should be discoverable through vcpkg");
    assert!(
        !found.include_paths.is_empty(),
        "vcpkg SDL3 discovery returned no include paths"
    );
}
"""
    _write_utf8_lf(manifest, manifest_text)
    _write_utf8_lf(build_script, build_script_text)
    _write_utf8_lf(main_source, "fn main() {}\n")
    command = CommandContract(
        arguments=("cargo", "check", "--manifest-path", os.fspath(manifest)),
        cwd=root,
    )
    return Sdl3Consumer(
        root=root,
        manifest=manifest,
        build_script=build_script,
        main_source=main_source,
        command=command,
    )


def check_sdl3_vcpkg_consumer(
    workspace: PathInput,
    repository_root: PathInput,
    *,
    runner: Runner = run,
) -> subprocess.CompletedProcess[str]:
    """Create and execute the temporary SDL3 consumer contract."""
    consumer = create_sdl3_vcpkg_consumer(workspace, repository_root)
    return consumer.command.execute(runner=runner)


def calculate_mingw_environment(
    msys2_root: PathInput,
    current_path: str = "",
    *,
    path_separator: str | None = None,
) -> MinGwEnvironment:
    """Prepend mingw64/bin once while preserving the remaining PATH order."""
    root = _pure_path(msys2_root)
    bin_directory = root / "mingw64" / "bin"
    separator = path_separator
    if separator is None:
        separator = ";" if isinstance(root, PureWindowsPath) else os.pathsep
    if not separator or len(separator) != 1:
        raise WindowsNativeError("PATH separator must be exactly one character")
    bin_text = os.fspath(bin_directory)
    windows = isinstance(root, PureWindowsPath)

    def key(value: str) -> str:
        normalized = value.replace("\\", "/").rstrip("/")
        return normalized.casefold() if windows else normalized

    existing = [part for part in current_path.split(separator) if part]
    filtered = [part for part in existing if key(part) != key(bin_text)]
    combined = separator.join((bin_text, *filtered))
    return MinGwEnvironment(
        msys2_root=root,
        bin_directory=bin_directory,
        path=combined,
        path_separator=separator,
    )


def find_mingw_test_executables(
    deps_directory: PathInput,
    pattern: str = "dear_imgui_sys-*.exe",
) -> tuple[Path, ...]:
    """Find only files and sort them deterministically across host filesystems."""
    directory = Path(deps_directory)
    return tuple(
        sorted(
            (path for path in directory.glob(pattern) if path.is_file()),
            key=lambda path: (path.name.casefold(), path.name),
        )
    )


def parse_objdump_imports(output: str) -> tuple[str, ...]:
    """Parse PE import names while retaining their objdump order."""
    return tuple(match.group(1) for match in _DLL_IMPORT.finditer(output))


def inspect_mingw_imports(
    deps_directory: PathInput,
    objdump_executable: PathInput,
    *,
    runner: Runner = run,
) -> ImportInspection:
    """Run objdump for every test executable and retain complete raw output."""
    directory = Path(deps_directory)
    binaries = find_mingw_test_executables(directory)
    if not binaries:
        raise MissingTestBinaryError(
            f"no dear_imgui_sys test binaries found in {directory}"
        )
    evidence = []
    for binary in binaries:
        command = (os.fspath(objdump_executable), "-p", os.fspath(binary))
        result = runner(
            command,
            capture_output=True,
            combine_output=True,
            accepted_returncodes=(0,),
        )
        output = result.stdout or ""
        if result.stderr:
            output += result.stderr
        if result.returncode != 0:
            raise CommandError(command, result.returncode, output)
        evidence.append(
            ObjdumpEvidence(
                binary=binary,
                command=command,
                output=output,
                imports=parse_objdump_imports(output),
            )
        )
    return ImportInspection(tuple(evidence))


def verify_mingw_imports(
    deps_directory: PathInput,
    objdump_executable: PathInput,
    *,
    runner: Runner = run,
) -> ImportInspection:
    """Reject any case variant of the dynamic GNU C++ runtime import."""
    inspection = inspect_mingw_imports(
        deps_directory, objdump_executable, runner=runner
    )
    if inspection.forbidden_evidence:
        raise ForbiddenImportError(inspection)
    return inspection
