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
_PE_MACHINE = re.compile(r"\bfile format\s+(\S+)", re.IGNORECASE)
_SDL3_RUNTIME_NAME = "SDL3.dll"


class WindowsNativeError(RuntimeError):
    """A Windows native CI contract could not be satisfied."""


class VcpkgRootError(WindowsNativeError):
    """No candidate contains a vcpkg root marker."""


class VcpkgStatusError(WindowsNativeError):
    """A vcpkg installation has no usable package status data."""


class MissingTestBinaryError(WindowsNativeError):
    """A Windows PE contract produced no matching sentinel executable."""


class ImportPolicyError(WindowsNativeError):
    """A PE executable violated required or forbidden import policy."""

    def __init__(
        self,
        inspection: "PeInspection",
        *,
        missing_imports: Sequence[str],
        forbidden_imports: Sequence[str],
    ) -> None:
        self.inspection = inspection
        self.missing_imports = tuple(missing_imports)
        self.forbidden_imports = tuple(forbidden_imports)
        details = []
        if self.missing_imports:
            details.append(f"missing required imports: {', '.join(self.missing_imports)}")
        if self.forbidden_imports:
            details.append(f"forbidden imports: {', '.join(self.forbidden_imports)}")
        super().__init__(
            f"PE import policy failed ({'; '.join(details)})\n"
            f"{inspection.evidence_text.rstrip()}"
        )


class MachineTypeError(WindowsNativeError):
    """A PE executable has a missing or unexpected COFF machine type."""

    def __init__(
        self,
        inspection: "PeInspection",
        expected_machine: str,
        actual_machines: Sequence[str],
    ) -> None:
        self.inspection = inspection
        self.expected_machine = expected_machine
        self.actual_machines = tuple(actual_machines)
        actual = ", ".join(self.actual_machines)
        super().__init__(
            f"PE machine mismatch: expected {expected_machine}, found {actual}\n"
            f"{inspection.evidence_text.rstrip()}"
        )


class InspectionCommandError(CommandError):
    """PE inspection failed after retaining all available command evidence."""

    def __init__(
        self,
        inspection: "PeInspection",
        command: Sequence[str],
        returncode: int,
        output: str,
    ) -> None:
        self.inspection = inspection
        super().__init__(command, returncode, output)


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

    @property
    def rust_environment(self) -> tuple[tuple[str, str], ...]:
        """Return Rust environment overrides required by this CRT profile."""
        if self.crt == "mt":
            return (("RUSTFLAGS", "-C target-feature=+crt-static"),)
        return ()


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
    machine: str | None
    returncode: int = 0

@dataclass(frozen=True)
class PeInspection:
    """Stable objdump evidence for every matching Windows PE sentinel."""

    evidence: tuple[ObjdumpEvidence, ...]

    @property
    def evidence_text(self) -> str:
        """Render complete raw evidence without discarding objdump diagnostics."""
        sections = []
        for item in self.evidence:
            output = item.output
            if output and not output.endswith("\n"):
                output += "\n"
            imports = ", ".join(item.imports) if item.imports else "<none>"
            machine = item.machine or "<missing>"
            sections.append(
                f"Checking PE evidence for {item.binary}\n"
                f"Command: {subprocess.list2cmdline(item.command)}\n"
                f"Exit code: {item.returncode}\n"
                f"Parsed machine: {machine}\n"
                f"Parsed imports: {imports}\n"
                f"Raw output:\n{output}"
            )
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
    executable: PathInput = "vcpkg",
    runner: Runner = run,
) -> subprocess.CompletedProcess[str]:
    """Install packages for one explicit triplet without shell interpolation."""
    if not packages:
        raise WindowsNativeError("at least one vcpkg package is required")
    command = (
        os.fspath(executable),
        "install",
        *(triplet.package(name) for name in packages),
    )
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
    missing_pkg_config = _pure_path(runner_temp) / "missing-pkg-config.exe"
    return (
        ("VCPKG_ROOT", os.fspath(root)),
        ("VCPKGRS_TRIPLET", triplet.name),
        ("PKG_CONFIG", os.fspath(missing_pkg_config)),
        ("PKG_CONFIG_PATH", ""),
        *triplet.rust_environment,
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


def restore_cached_sdl3_runtime(
    target_directory: PathInput,
    profile: str = "debug",
) -> Path:
    """Restore SDL3's runtime DLL after Cargo restores build-script output from cache."""
    if not profile or Path(profile).name != profile:
        raise WindowsNativeError(f"invalid Cargo profile directory: {profile!r}")

    profile_directory = Path(target_directory) / profile
    candidates = tuple(
        sorted(
            (
                path
                for path in (
                    profile_directory / "build"
                ).glob(f"sdl3-sys-*/out/bin/{_SDL3_RUNTIME_NAME}")
                if path.is_file()
            ),
            key=lambda path: (path.as_posix().casefold(), path.as_posix()),
        )
    )
    if not candidates:
        raise WindowsNativeError(
            "Cargo produced no cached SDL3 runtime DLL under "
            f"{profile_directory / 'build'}"
        )

    source = candidates[0]
    if source.stat().st_size == 0:
        raise WindowsNativeError(f"cached SDL3 runtime DLL is empty: {source}")

    destination = profile_directory / _SDL3_RUNTIME_NAME
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    print(f"Restored SDL3 runtime: {source} -> {destination}")
    return destination


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


def find_windows_test_executables(
    deps_directory: PathInput,
    patterns: Sequence[str],
) -> tuple[Path, ...]:
    """Resolve every explicit PE sentinel pattern and return a stable union."""
    directory = Path(deps_directory)
    if not patterns:
        raise MissingTestBinaryError("at least one PE test-binary pattern is required")

    binaries: set[Path] = set()
    for pattern in patterns:
        if not pattern:
            raise MissingTestBinaryError("PE test-binary patterns must not be empty")
        matches = tuple(path for path in directory.glob(pattern) if path.is_file())
        if not matches:
            raise MissingTestBinaryError(
                f"no test binaries matched pattern {pattern!r} in {directory}"
            )
        binaries.update(matches)

    return tuple(
        sorted(
            binaries,
            key=lambda path: (path.name.casefold(), path.name, os.fspath(path)),
        )
    )


def parse_objdump_imports(output: str) -> tuple[str, ...]:
    """Parse PE import names while retaining their objdump order."""
    return tuple(match.group(1) for match in _DLL_IMPORT.finditer(output))


def parse_objdump_machine(output: str) -> str | None:
    """Parse llvm-objdump's COFF format identifier."""
    match = _PE_MACHINE.search(output)
    return match.group(1).casefold() if match else None


def _timeout_output(error: subprocess.TimeoutExpired, timeout: float) -> str:
    def text(value: str | bytes | None) -> str:
        if value is None:
            return ""
        if isinstance(value, bytes):
            return value.decode("utf-8", errors="replace")
        return value

    output = ""
    for partial in (text(error.stdout), text(error.stderr)):
        if not partial:
            continue
        output += partial
        if not partial.endswith("\n"):
            output += "\n"
    return f"{output}llvm-objdump timed out after {timeout:g} seconds\n"


def inspect_windows_pe(
    deps_directory: PathInput,
    objdump_executable: PathInput,
    *,
    binary_patterns: Sequence[str],
    runner: Runner = run,
    timeout: float = 60.0,
) -> PeInspection:
    """Inspect explicit PE sentinels and retain raw plus parsed evidence."""
    directory = Path(deps_directory)
    binaries = find_windows_test_executables(directory, binary_patterns)
    evidence: list[ObjdumpEvidence] = []
    for binary in binaries:
        command = (os.fspath(objdump_executable), "-p", os.fspath(binary))
        try:
            result = runner(
                command,
                capture_output=True,
                combine_output=True,
                accepted_returncodes=None,
                timeout=timeout,
            )
        except subprocess.TimeoutExpired as error:
            output = _timeout_output(error, timeout)
            evidence.append(
                ObjdumpEvidence(
                    binary=binary,
                    command=command,
                    output=output,
                    imports=parse_objdump_imports(output),
                    machine=parse_objdump_machine(output),
                    returncode=-1,
                )
            )
            inspection = PeInspection(tuple(evidence))
            raise InspectionCommandError(inspection, command, -1, output) from error
        except CommandError as error:
            output = error.output
            evidence.append(
                ObjdumpEvidence(
                    binary=binary,
                    command=command,
                    output=output,
                    imports=parse_objdump_imports(output),
                    machine=parse_objdump_machine(output),
                    returncode=error.returncode,
                )
            )
            inspection = PeInspection(tuple(evidence))
            raise InspectionCommandError(
                inspection,
                error.command,
                error.returncode,
                error.output,
            ) from error

        output = result.stdout or ""
        if result.stderr:
            output += result.stderr
        evidence.append(
            ObjdumpEvidence(
                binary=binary,
                command=command,
                output=output,
                imports=parse_objdump_imports(output),
                machine=parse_objdump_machine(output),
                returncode=result.returncode,
            )
        )
        if result.returncode != 0:
            inspection = PeInspection(tuple(evidence))
            raise InspectionCommandError(
                inspection,
                command,
                result.returncode,
                output,
            )
    return PeInspection(tuple(evidence))


def _unique_casefolded(values: Sequence[str]) -> tuple[str, ...]:
    seen = set()
    unique = []
    for value in values:
        folded = value.casefold()
        if not value or folded in seen:
            continue
        seen.add(folded)
        unique.append(value)
    return tuple(unique)


def verify_windows_pe(
    deps_directory: PathInput,
    objdump_executable: PathInput,
    *,
    binary_patterns: Sequence[str],
    required_imports: Sequence[str] = (),
    forbidden_imports: Sequence[str] = (),
    expected_machine: str | None = None,
    runner: Runner = run,
    timeout: float = 60.0,
) -> PeInspection:
    """Verify concrete PE machine and import policies for every sentinel."""
    inspection = inspect_windows_pe(
        deps_directory,
        objdump_executable,
        binary_patterns=binary_patterns,
        runner=runner,
        timeout=timeout,
    )

    if expected_machine is not None:
        expected = expected_machine.casefold()
        mismatches = tuple(
            item.machine or "<missing>"
            for item in inspection.evidence
            if item.machine is None or item.machine.casefold() != expected
        )
        if mismatches:
            raise MachineTypeError(inspection, expected_machine, mismatches)

    required = _unique_casefolded(required_imports)
    forbidden = _unique_casefolded(forbidden_imports)
    missing = tuple(
        required_name
        for required_name in required
        if any(
            required_name.casefold()
            not in {imported.casefold() for imported in item.imports}
            for item in inspection.evidence
        )
    )
    forbidden_keys = {name.casefold() for name in forbidden}
    forbidden_found = _unique_casefolded(
        tuple(
            imported
            for item in inspection.evidence
            for imported in item.imports
            if imported.casefold() in forbidden_keys
        )
    )
    if missing or forbidden_found:
        raise ImportPolicyError(
            inspection,
            missing_imports=missing,
            forbidden_imports=forbidden_found,
        )
    return inspection
