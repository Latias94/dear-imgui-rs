#!/usr/bin/env python3
"""Capture, materialize, and finalize authoritative release-gate cells."""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import re
import shutil
import sys
import tempfile
import tomllib
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any


CI_DIR = Path(__file__).resolve().parent
TOOLS_DIR = CI_DIR.parent
WORKSPACE_ROOT = TOOLS_DIR.parent
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

import _prebuilt  # noqa: E402
import release_evidence  # noqa: E402
from _process import ProcessStartError, run_bounded  # noqa: E402
from _verification import VerificationError  # noqa: E402
from _windows_native import VcpkgTriplet, inspect_vcpkg_status  # noqa: E402


SCHEMA_VERSION = 1
TIMEOUT_EXIT_CODE = 124
START_FAILURE_EXIT_CODE = 127
_EXECUTION_FIELDS = frozenset(
    {
        "schema_version",
        "command",
        "returncode",
        "timed_out",
        "start_failure",
        "evidence_errors",
    }
)
_RUNTIME_RESULT_FIELDS = frozenset(
    {
        "schema_version",
        "status",
        "gate",
        "success",
        "category",
        "attempt",
        "summary",
        "retry",
        "evidence",
        "details",
    }
)
_RUNTIME_CATEGORIES = frozenset(
    {
        "Passed",
        "TestTimedOut",
        "HarnessTimeout",
        "InfrastructureUnavailable",
        "ProductFailure",
    }
)
_RUNTIME_CELLS = {
    "linux-test-engine-runtime": "test-engine-runtime",
    "linux-multi-viewport-smoke": "multi-viewport-smoke",
    "linux-sdl3-glow-multi-viewport-smoke": "sdl3-glow-multi-viewport-smoke",
}
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class ReleaseCellError(ValueError):
    """One release-cell input violates the authoritative contract."""


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ReleaseCellError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def _read_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8", newline="") as source:
            value = json.load(source, object_pairs_hook=_reject_duplicate_keys)
    except ReleaseCellError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ReleaseCellError(f"could not read {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise ReleaseCellError(f"{label} must contain a JSON object: {path}")
    return value


def _safe_relative(value: str, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value or "\\" in value:
        raise ReleaseCellError(f"{label} must be a non-empty POSIX relative path")
    windows = PureWindowsPath(value)
    parts = value.split("/")
    if value.startswith("/") or windows.drive or any(
        part in ("", ".", "..") for part in parts
    ):
        raise ReleaseCellError(f"unsafe {label}: {value!r}")
    return PurePosixPath(*parts)


def _resolve_under(root: Path, value: Path, label: str) -> Path:
    root = root.resolve()
    candidate = value if value.is_absolute() else root / value
    resolved = candidate.resolve()
    try:
        relative = resolved.relative_to(root)
    except ValueError as error:
        raise ReleaseCellError(f"{label} escapes cell root: {value}") from error
    if not relative.parts:
        raise ReleaseCellError(f"{label} must name a file below the cell root")
    return resolved


def _atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            prefix=f".{path.name}.",
            suffix=".tmp",
            dir=path.parent,
            delete=False,
        ) as destination:
            temporary = Path(destination.name)
            destination.write(payload)
            destination.flush()
            os.fsync(destination.fileno())
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def _normalize_utf8_lf(path: Path) -> None:
    try:
        text = path.read_bytes().decode("utf-8")
    except (OSError, UnicodeError) as error:
        raise ReleaseCellError(f"could not normalize UTF-8 log {path}: {error}") from error
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    _atomic_write_bytes(path, normalized.encode("utf-8"))


def _copy_regular_file(source: Path, destination: Path) -> None:
    source_path = Path(source)
    if source_path.is_symlink() or not source_path.is_file():
        raise ReleaseCellError(f"release evidence is not a regular file: {source_path}")
    source = source_path.resolve()
    if source == destination.resolve():
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with (
            source.open("rb") as input_file,
            tempfile.NamedTemporaryFile(
                mode="wb",
                prefix=f".{destination.name}.",
                suffix=".tmp",
                dir=destination.parent,
                delete=False,
            ) as output_file,
        ):
            temporary = Path(output_file.name)
            shutil.copyfileobj(input_file, output_file, length=1024 * 1024)
            output_file.flush()
            os.fsync(output_file.fileno())
        os.replace(temporary, destination)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def _authoritative_cell(cell_id: str) -> release_evidence.ExpectedCell:
    matches = [
        cell
        for cell in release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY
        if cell.cell_id == cell_id
    ]
    if len(matches) != 1:
        raise ReleaseCellError(f"unknown or duplicate authoritative cell: {cell_id!r}")
    return matches[0]


def _resolve_identity(
    cell: release_evidence.ExpectedCell,
    *,
    target: str | None,
    crt: str | None,
    target_fallback: str | None = None,
) -> tuple[str | None, str | None]:
    if cell.target is not None:
        if target is not None and target != cell.target:
            raise ReleaseCellError(
                f"target mismatch for {cell.cell_id}: expected {cell.target!r}, "
                f"found {target!r}"
            )
        resolved_target = cell.target
    else:
        resolved_target = target or target_fallback
        if cell.cell_id == "macos-build" and (
            resolved_target is None or not resolved_target.endswith("-apple-darwin")
        ):
            raise ReleaseCellError("macos-build requires an explicit Apple Darwin target")

    if cell.crt is not None:
        if crt is not None and crt != cell.crt:
            raise ReleaseCellError(
                f"CRT mismatch for {cell.cell_id}: expected {cell.crt!r}, found {crt!r}"
            )
        resolved_crt = cell.crt
    else:
        if crt is not None:
            raise ReleaseCellError(f"{cell.cell_id} does not have a CRT dimension")
        resolved_crt = None
    return resolved_target, resolved_crt


def _execution_payload(
    command: Sequence[str],
    *,
    returncode: int | None,
    timed_out: bool,
    start_failure: bool,
    evidence_errors: Sequence[str] = (),
) -> dict[str, object]:
    return {
        "schema_version": SCHEMA_VERSION,
        "command": list(command),
        "returncode": returncode,
        "timed_out": timed_out,
        "start_failure": start_failure,
        "evidence_errors": list(evidence_errors),
    }


def _propagated_returncode(returncode: int) -> int:
    return returncode if 0 <= returncode <= 255 else 1


def capture_command(
    *,
    cell_root: Path,
    execution_path: Path,
    stdout_log: Path,
    stderr_log: Path,
    command: Sequence[str | Path],
    timeout: float,
    cwd: Path | None = None,
    termination_grace: float = 5.0,
    bounded_runner: Callable[..., Any] = run_bounded,
) -> int:
    """Run one bounded command and retain deterministic execution evidence."""
    cell_root = Path(cell_root).resolve()
    execution = _resolve_under(cell_root, Path(execution_path), "execution JSON")
    stdout = _resolve_under(cell_root, Path(stdout_log), "stdout log")
    stderr = _resolve_under(cell_root, Path(stderr_log), "stderr log")
    if len({execution, stdout, stderr}) != 3:
        raise ReleaseCellError("execution, stdout, and stderr paths must be distinct")
    if execution.relative_to(cell_root).parts[0] != "executions":
        raise ReleaseCellError("execution JSON must be below executions/")
    for path, label in ((stdout, "stdout"), (stderr, "stderr")):
        if path.relative_to(cell_root).parts[0] != "logs":
            raise ReleaseCellError(f"{label} log must be below logs/")
    rendered = tuple(os.fspath(argument) for argument in command)
    if not rendered:
        raise ReleaseCellError("a command is required after --")

    try:
        result = bounded_runner(
            rendered,
            timeout=timeout,
            stdout_log=stdout,
            stderr_log=stderr,
            cwd=Path(cwd).resolve() if cwd is not None else None,
            termination_grace=termination_grace,
        )
    except ProcessStartError:
        _normalize_utf8_lf(stdout)
        _normalize_utf8_lf(stderr)
        release_evidence.atomic_write_json(
            execution,
            _execution_payload(
                rendered,
                returncode=None,
                timed_out=False,
                start_failure=True,
            ),
        )
        return START_FAILURE_EXIT_CODE

    _normalize_utf8_lf(stdout)
    _normalize_utf8_lf(stderr)
    evidence_errors = (*result.stream_errors, *result.termination.errors)
    release_evidence.atomic_write_json(
        execution,
        _execution_payload(
            rendered,
            returncode=result.returncode,
            timed_out=result.timed_out,
            start_failure=False,
            evidence_errors=evidence_errors,
        ),
    )
    if result.timed_out:
        return TIMEOUT_EXIT_CODE
    if evidence_errors and result.returncode == 0:
        return 1
    return _propagated_returncode(result.returncode)


def _metadata_base(
    *, cell_id: str, candidate_sha: str, target: str | None, crt: str | None
) -> dict[str, object]:
    value: dict[str, object] = {
        "schema_version": SCHEMA_VERSION,
        "cell_id": cell_id,
        "candidate_sha": candidate_sha,
    }
    if target is not None:
        value["target"] = target
    if crt is not None:
        value["crt"] = crt
    return value


def _workspace_manifests(repo_root: Path) -> tuple[Path, ...]:
    root_manifest = repo_root / "Cargo.toml"
    try:
        with root_manifest.open("rb") as source:
            root_value = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ReleaseCellError(f"could not read workspace manifest: {error}") from error
    workspace = root_value.get("workspace")
    members = workspace.get("members") if isinstance(workspace, dict) else None
    if not isinstance(members, list) or any(not isinstance(item, str) for item in members):
        raise ReleaseCellError("workspace.members must be an array of paths")
    manifests = {root_manifest.resolve()}
    for member in members:
        relative = _safe_relative(member, "workspace member")
        member_path = repo_root.joinpath(*relative.parts)
        if any(character in member for character in "*?["):
            matches = sorted(repo_root.glob(member), key=lambda path: path.as_posix())
        else:
            matches = [member_path]
        if not matches:
            raise ReleaseCellError(f"workspace member pattern has no matches: {member!r}")
        for match in matches:
            manifest = (match / "Cargo.toml").resolve()
            try:
                manifest.relative_to(repo_root)
                with manifest.open("rb") as source:
                    parsed = tomllib.load(source)
            except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
                raise ReleaseCellError(f"invalid workspace manifest {manifest}: {error}") from error
            if not isinstance(parsed, dict):
                raise ReleaseCellError(f"manifest must contain a TOML table: {manifest}")
            manifests.add(manifest)
    return tuple(sorted(manifests, key=lambda path: path.relative_to(repo_root).as_posix()))


def _hash_records(paths: Sequence[Path], repo_root: Path) -> list[dict[str, str]]:
    records = []
    for path in paths:
        if path.is_symlink() or not path.is_file():
            raise ReleaseCellError(f"metadata input is not a regular file: {path}")
        records.append(
            {
                "path": path.relative_to(repo_root).as_posix(),
                "sha256": release_evidence.sha256_file(path),
            }
        )
    return records


def _binding_files(repo_root: Path, target: str) -> tuple[Path, ...]:
    if target == "wasm32-unknown-unknown":
        filename = "wasm_bindings_pregenerated.rs"
    elif "windows" in target:
        filename = "bindings_pregenerated_windows.rs"
    else:
        filename = "bindings_pregenerated.rs"
    core = repo_root / "dear-imgui-sys/src" / filename
    if not core.is_file():
        raise ReleaseCellError(f"core binding input is missing: {core}")
    extension_filename = (
        "wasm_bindings_pregenerated.rs"
        if target == "wasm32-unknown-unknown"
        else "bindings_pregenerated.rs"
    )
    extensions = tuple(
        sorted(
            repo_root.glob(f"extensions/*-sys/src/{extension_filename}"),
            key=lambda path: path.relative_to(repo_root).as_posix(),
        )
    )
    paths = (core, *extensions)
    for path in paths:
        try:
            path.read_bytes().decode("utf-8")
        except (OSError, UnicodeError) as error:
            raise ReleaseCellError(f"binding input is not UTF-8: {path}: {error}") from error
    return paths


def _vcpkg_payload(
    root: Path,
    *,
    base: Mapping[str, object],
    target: str,
    crt: str,
) -> dict[str, object]:
    status = inspect_vcpkg_status(root)
    if not status.marker.is_file() or not status.has_status_data:
        raise ReleaseCellError(f"vcpkg status data is incomplete under {root}")
    status_record: dict[str, object] | None = None
    if status.status_file.is_file():
        status_record = {
            "path": "installed/vcpkg/status",
            "bytes": status.status_bytes,
            "sha256": release_evidence.sha256_file(status.status_file),
        }
    updates = [
        {
            "path": f"installed/vcpkg/updates/{path.name}",
            "bytes": path.stat().st_size,
            "sha256": release_evidence.sha256_file(path),
        }
        for path in status.update_files
    ]
    value = dict(base)
    value.update(
        {
            "triplet": VcpkgTriplet.from_target(target, crt).name,
            "status": status_record,
            "updates": updates,
        }
    )
    return value


def _validated_mingw_bytes(path: Path) -> bytes:
    try:
        text = path.read_bytes().decode("utf-8")
    except (OSError, UnicodeError) as error:
        raise ReleaseCellError(f"could not read MinGW import evidence: {error}") from error
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    if not normalized or "\0" in normalized:
        raise ReleaseCellError("MinGW import evidence must be non-empty UTF-8 text")
    if "libstdc++-6.dll" in normalized.casefold():
        raise ReleaseCellError("MinGW import evidence contains forbidden libstdc++-6.dll")
    if not normalized.endswith("\n"):
        normalized += "\n"
    return normalized.encode("utf-8")


def _prebuilt_outputs(
    *,
    repo_root: Path,
    cell_root: Path,
    package_dir: Path,
    target: str,
    crt: str,
    candidate_sha: str,
    base: Mapping[str, object],
) -> tuple[Path, ...]:
    package_dir = package_dir.resolve()
    if not package_dir.is_dir():
        raise ReleaseCellError(f"prebuilt package directory is missing: {package_dir}")
    try:
        core = _prebuilt.select_core_prebuilt_archives(
            package_dir,
            target,
            crt,
            profile_scope="all",
            candidate_sha=candidate_sha,
        )
        extensions = _prebuilt.select_extension_prebuilt_archives(
            package_dir,
            target,
            crt,
            candidate_sha,
            repo_root,
            core,
            profile_scope="all",
        )
    except VerificationError as error:
        raise ReleaseCellError(str(error)) from error
    selected = set(core.values()) | set(extensions.values())
    available = set(package_dir.glob("*.tar.gz"))
    if selected != available:
        missing = sorted(path.name for path in selected - available)
        unexpected = sorted(path.name for path in available - selected)
        raise ReleaseCellError(
            f"prebuilt package inventory mismatch: missing={missing!r}, "
            f"unexpected={unexpected!r}"
        )

    archives: list[dict[str, object]] = []
    bindings: list[dict[str, object]] = []
    copied: list[Path] = []
    for source in sorted(selected, key=lambda path: path.name):
        try:
            fields = _prebuilt._read_prebuilt_manifest(source)
        except VerificationError as error:
            raise ReleaseCellError(str(error)) from error
        if fields.get("candidate_sha") != candidate_sha:
            raise ReleaseCellError(f"prebuilt candidate mismatch in {source.name}")
        destination = cell_root / "packages" / source.name
        _copy_regular_file(source, destination)
        copied.append(destination)
        digest = release_evidence.sha256_file(destination)
        archives.append(
            {"archive": source.name, "sha256": digest, "manifest": dict(fields)}
        )
        binding_fields = {
            key: fields[key]
            for key in (
                "binding_spec_hash",
                "cimgui_revision",
                "imgui_revision",
                "extension_binding_identity",
                "core_artifact_identity",
            )
            if key in fields
        }
        if not binding_fields:
            raise ReleaseCellError(f"prebuilt manifest lacks binding identity: {source.name}")
        bindings.append(
            {"archive": source.name, "archive_sha256": digest, **binding_fields}
        )

    metadata = cell_root / "metadata"
    manifests_path = metadata / "prebuilt-manifests.json"
    binding_path = metadata / "binding-hashes.json"
    release_evidence.atomic_write_json(
        manifests_path, {**base, "archives": archives}
    )
    release_evidence.atomic_write_json(
        binding_path, {**base, "archives": bindings}
    )
    return (*copied, manifests_path, binding_path)


def _expected_metadata_paths(cell: release_evidence.ExpectedCell) -> set[str]:
    return {
        requirement.pattern
        for requirement in cell.requirements
        if requirement.collection == "artifacts"
        and requirement.pattern.startswith("metadata/")
    }


def materialize_metadata(
    *,
    repo_root: Path,
    candidate_sha: str,
    cell_id: str,
    cell_root: Path,
    target: str | None = None,
    crt: str | None = None,
    vcpkg_root: Path | None = None,
    mingw_imports: Path | None = None,
    package_dir: Path | None = None,
) -> tuple[Path, ...]:
    """Materialize the fixed metadata contract for one authoritative cell."""
    repo_root = Path(repo_root).resolve()
    candidate_sha = release_evidence.resolve_candidate_sha(repo_root, candidate_sha)
    cell = _authoritative_cell(cell_id)
    if cell_id in _RUNTIME_CELLS:
        raise ReleaseCellError("runtime cells are materialized by finalize-runtime")
    resolved_target, resolved_crt = _resolve_identity(cell, target=target, crt=crt)
    cell_root = Path(cell_root).resolve()

    option_requirements = {
        "windows-vcpkg": (vcpkg_root, "--vcpkg-root"),
        "windows-gnu": (mingw_imports, "--mingw-imports"),
    }
    if cell_id.startswith("prebuilt-"):
        option_requirements[cell_id] = (package_dir, "--package-dir")
    if cell_id in option_requirements and option_requirements[cell_id][0] is None:
        raise ReleaseCellError(
            f"{option_requirements[cell_id][1]} is required for {cell_id}"
        )
    if vcpkg_root is not None and cell_id != "windows-vcpkg":
        raise ReleaseCellError("--vcpkg-root is valid only for windows-vcpkg")
    if mingw_imports is not None and cell_id != "windows-gnu":
        raise ReleaseCellError("--mingw-imports is valid only for windows-gnu")
    if package_dir is not None and not cell_id.startswith("prebuilt-"):
        raise ReleaseCellError("--package-dir is valid only for prebuilt cells")

    base = _metadata_base(
        cell_id=cell_id,
        candidate_sha=candidate_sha,
        target=resolved_target,
        crt=resolved_crt,
    )
    if cell_id.startswith("prebuilt-"):
        assert package_dir is not None and resolved_target is not None
        outputs = _prebuilt_outputs(
            repo_root=repo_root,
            cell_root=cell_root,
            package_dir=package_dir,
            target=resolved_target,
            crt=resolved_crt or "",
            candidate_sha=candidate_sha,
            base=base,
        )
    else:
        metadata = cell_root / "metadata"
        outputs_list: list[Path] = []
        expected = _expected_metadata_paths(cell)
        if "metadata/target.json" in expected:
            target_path = metadata / "target.json"
            release_evidence.atomic_write_json(target_path, base)
            outputs_list.append(target_path)
        if "metadata/crt.json" in expected:
            crt_path = metadata / "crt.json"
            release_evidence.atomic_write_json(crt_path, base)
            outputs_list.append(crt_path)
        if "metadata/platform-io-abi.json" in expected:
            abi_path = metadata / "platform-io-abi.json"
            release_evidence.atomic_write_json(
                abi_path,
                {**base, "contract": "cpp-platform-io-aggregate-callback-abi"},
            )
            outputs_list.append(abi_path)
        if "metadata/binding-hashes.json" in expected:
            assert resolved_target is not None
            binding_path = metadata / "binding-hashes.json"
            release_evidence.atomic_write_json(
                binding_path,
                {
                    **base,
                    "files": _hash_records(
                        _binding_files(repo_root, resolved_target), repo_root
                    ),
                },
            )
            outputs_list.append(binding_path)
        if "metadata/manifests.json" in expected:
            manifest_path = metadata / "manifests.json"
            release_evidence.atomic_write_json(
                manifest_path,
                {
                    **base,
                    "files": _hash_records(_workspace_manifests(repo_root), repo_root),
                },
            )
            outputs_list.append(manifest_path)
        if "metadata/vcpkg.json" in expected:
            assert vcpkg_root is not None
            assert resolved_target is not None and resolved_crt is not None
            vcpkg_path = metadata / "vcpkg.json"
            release_evidence.atomic_write_json(
                vcpkg_path,
                _vcpkg_payload(
                    vcpkg_root,
                    base=base,
                    target=resolved_target,
                    crt=resolved_crt,
                ),
            )
            outputs_list.append(vcpkg_path)
        if "metadata/mingw-imports.txt" in expected:
            assert mingw_imports is not None
            imports_path = metadata / "mingw-imports.txt"
            _atomic_write_bytes(imports_path, _validated_mingw_bytes(mingw_imports))
            outputs_list.append(imports_path)
        outputs = tuple(outputs_list)

    actual_metadata = {
        path.relative_to(cell_root).as_posix()
        for path in outputs
        if path.is_relative_to(cell_root / "metadata")
    }
    expected_metadata = _expected_metadata_paths(cell)
    if actual_metadata != expected_metadata:
        raise ReleaseCellError(
            f"metadata contract mismatch for {cell_id}: expected "
            f"{sorted(expected_metadata)!r}, found {sorted(actual_metadata)!r}"
        )
    return tuple(sorted(outputs, key=lambda path: path.relative_to(cell_root).as_posix()))


def _matches(path: str, pattern: str) -> bool:
    path_parts = PurePosixPath(path).parts
    pattern_parts = PurePosixPath(pattern).parts
    return len(path_parts) == len(pattern_parts) and all(
        fnmatch.fnmatchcase(part, expected)
        for part, expected in zip(path_parts, pattern_parts, strict=True)
    )


def _validate_required_payloads(
    cell: release_evidence.ExpectedCell,
    artifacts: Sequence[Path],
    logs: Sequence[Path],
    cell_root: Path,
) -> None:
    relative = {
        "artifacts": {path.relative_to(cell_root).as_posix() for path in artifacts},
        "logs": {path.relative_to(cell_root).as_posix() for path in logs},
    }
    errors = []
    for requirement in cell.requirements:
        if any(
            _matches(path, requirement.pattern)
            for path in relative[requirement.collection]
        ):
            continue
        other = "logs" if requirement.collection == "artifacts" else "artifacts"
        if any(_matches(path, requirement.pattern) for path in relative[other]):
            errors.append(
                f"{requirement.pattern!r} is classified as {other}, not "
                f"{requirement.collection}"
            )
        else:
            errors.append(f"missing {requirement.collection} {requirement.pattern!r}")
    if errors:
        raise ReleaseCellError("; ".join(errors))


def _validate_metadata_identity(
    path: Path,
    *,
    cell_id: str,
    candidate_sha: str,
    target: str | None,
    crt: str | None,
) -> dict[str, Any]:
    value = _read_json_object(path, "metadata JSON")
    if value.get("schema_version") != SCHEMA_VERSION:
        raise ReleaseCellError(f"metadata schema version mismatch: {path}")
    if value.get("cell_id") != cell_id:
        raise ReleaseCellError(f"metadata cell mismatch: {path}")
    if value.get("candidate_sha") != candidate_sha:
        raise ReleaseCellError(f"metadata candidate SHA mismatch: {path}")
    if value.get("target") != target:
        raise ReleaseCellError(f"metadata target mismatch: {path}")
    if ("crt" in value or crt is not None) and value.get("crt") != crt:
        raise ReleaseCellError(f"metadata CRT mismatch: {path}")
    return value


def _discover_normal_payloads(
    cell_root: Path,
    *,
    output_path: Path,
    cell_id: str,
    candidate_sha: str,
    target: str | None,
    crt: str | None,
) -> tuple[list[Path], list[Path]]:
    artifacts: list[Path] = []
    logs: list[Path] = []
    for path in sorted(
        cell_root.rglob("*"),
        key=lambda item: item.relative_to(cell_root).as_posix(),
    ):
        if not path.is_file() or path.resolve() == output_path.resolve():
            continue
        if path.is_symlink():
            raise ReleaseCellError(f"cell evidence must not be a symlink: {path}")
        relative = path.relative_to(cell_root)
        top = relative.parts[0]
        if top == "logs":
            logs.append(path)
        elif top in {"metadata", "packages", "executions"}:
            artifacts.append(path)
            if top == "metadata" and path.suffix == ".json":
                _validate_metadata_identity(
                    path,
                    cell_id=cell_id,
                    candidate_sha=candidate_sha,
                    target=target,
                    crt=crt,
                )
        else:
            raise ReleaseCellError(f"unclassified cell evidence path: {relative.as_posix()}")
    return artifacts, logs


def _validate_execution(path: Path) -> dict[str, Any]:
    value = _read_json_object(path, "capture execution JSON")
    if set(value) != _EXECUTION_FIELDS or value.get("schema_version") != SCHEMA_VERSION:
        raise ReleaseCellError(f"capture execution JSON has an invalid schema: {path}")
    command = value["command"]
    if (
        not isinstance(command, list)
        or not command
        or any(not isinstance(item, str) for item in command)
        or not command[0]
    ):
        raise ReleaseCellError(f"capture execution command is invalid: {path}")
    timed_out = value["timed_out"]
    start_failure = value["start_failure"]
    returncode = value["returncode"]
    errors = value["evidence_errors"]
    if not isinstance(timed_out, bool) or not isinstance(start_failure, bool):
        raise ReleaseCellError(f"capture execution booleans are invalid: {path}")
    if not isinstance(errors, list) or any(not isinstance(item, str) for item in errors):
        raise ReleaseCellError(f"capture execution evidence errors are invalid: {path}")
    if start_failure:
        if returncode is not None or timed_out:
            raise ReleaseCellError(f"start failure execution is inconsistent: {path}")
    elif not isinstance(returncode, int) or isinstance(returncode, bool):
        raise ReleaseCellError(f"capture execution returncode is invalid: {path}")
    return value


def _validate_prebuilt_package_inventory(
    cell_root: Path, artifacts: Sequence[Path]
) -> None:
    manifests = _read_json_object(
        cell_root / "metadata/prebuilt-manifests.json",
        "prebuilt manifest metadata",
    ).get("archives")
    if not isinstance(manifests, list) or not manifests:
        raise ReleaseCellError("prebuilt manifest metadata must list archives")
    expected: dict[str, str] = {}
    for index, entry in enumerate(manifests):
        if not isinstance(entry, dict) or set(entry) != {
            "archive",
            "sha256",
            "manifest",
        }:
            raise ReleaseCellError(
                f"prebuilt manifest metadata archive {index} has an invalid schema"
            )
        name = entry["archive"]
        digest = entry["sha256"]
        manifest = entry["manifest"]
        if (
            not isinstance(name, str)
            or PurePosixPath(name).name != name
            or not name.endswith(".tar.gz")
            or not isinstance(digest, str)
            or _SHA256.fullmatch(digest) is None
            or not isinstance(manifest, dict)
        ):
            raise ReleaseCellError(
                f"prebuilt manifest metadata archive {index} is invalid"
            )
        if name in expected:
            raise ReleaseCellError(f"prebuilt manifest metadata repeats {name!r}")
        expected[name] = digest
    packages = {
        path.name: path
        for path in artifacts
        if path.is_relative_to(cell_root / "packages")
    }
    if set(packages) != set(expected):
        raise ReleaseCellError(
            "packages/ does not match metadata/prebuilt-manifests.json"
        )
    for name, path in packages.items():
        if release_evidence.sha256_file(path) != expected[name]:
            raise ReleaseCellError(f"prebuilt package checksum mismatch: {name}")

    binding_value = _read_json_object(
        cell_root / "metadata/binding-hashes.json", "prebuilt binding metadata"
    )
    bindings = binding_value.get("archives")
    if not isinstance(bindings, list):
        raise ReleaseCellError(
            "prebuilt binding metadata does not cover the exact package inventory"
        )
    binding_names: list[str] = []
    for entry in bindings:
        if not isinstance(entry, dict):
            raise ReleaseCellError("prebuilt binding metadata archive is invalid")
        name = entry.get("archive")
        digest = entry.get("archive_sha256")
        if (
            not isinstance(name, str)
            or name not in expected
            or digest != expected[name]
        ):
            raise ReleaseCellError("prebuilt binding metadata archive is invalid")
        binding_names.append(name)
    if len(binding_names) != len(set(binding_names)) or set(binding_names) != set(
        expected
    ):
        raise ReleaseCellError(
            "prebuilt binding metadata does not cover the exact package inventory"
        )


def _output_cell_path(cell_root: Path, output_path: Path | None) -> Path:
    output = cell_root / "cell.json" if output_path is None else Path(output_path)
    output = _resolve_under(cell_root, output, "cell record")
    if output.parent != cell_root or output.name != "cell.json":
        raise ReleaseCellError("the authoritative cell record must be cell-root/cell.json")
    return output


def _target_from_metadata(cell_root: Path) -> str | None:
    path = cell_root / "metadata/target.json"
    if not path.is_file():
        return None
    value = _read_json_object(path, "target metadata")
    target = value.get("target")
    return target if isinstance(target, str) else None


def finalize_cell(
    *,
    repo_root: Path,
    candidate_sha: str,
    cell_id: str,
    cell_root: Path,
    execution_paths: Sequence[Path],
    target: str | None = None,
    crt: str | None = None,
    output_path: Path | None = None,
) -> dict[str, Any]:
    """Finalize a normal or prebuilt cell from capture execution JSON only."""
    repo_root = Path(repo_root).resolve()
    candidate_sha = release_evidence.resolve_candidate_sha(repo_root, candidate_sha)
    cell = _authoritative_cell(cell_id)
    if cell_id in _RUNTIME_CELLS:
        raise ReleaseCellError("runtime cells must use finalize-runtime")
    cell_root = Path(cell_root).resolve()
    if not execution_paths:
        raise ReleaseCellError("at least one capture execution JSON is required")
    output = _output_cell_path(cell_root, output_path)
    resolved_executions = [
        _resolve_under(cell_root, Path(path), "capture execution JSON")
        for path in execution_paths
    ]
    if len(set(resolved_executions)) != len(resolved_executions):
        raise ReleaseCellError("capture execution JSON paths must not repeat")
    executions = []
    for path in resolved_executions:
        if path.relative_to(cell_root).parts[0] != "executions" or not path.is_file():
            raise ReleaseCellError(f"capture execution JSON is missing: {path}")
        executions.append(_validate_execution(path))

    target_fallback = _target_from_metadata(cell_root)
    resolved_target, resolved_crt = _resolve_identity(
        cell, target=target, crt=crt, target_fallback=target_fallback
    )
    artifacts, logs = _discover_normal_payloads(
        cell_root,
        output_path=output,
        cell_id=cell_id,
        candidate_sha=candidate_sha,
        target=resolved_target,
        crt=resolved_crt,
    )
    discovered_executions = {
        path
        for path in artifacts
        if path.is_relative_to(cell_root / "executions")
    }
    if set(resolved_executions) != discovered_executions:
        raise ReleaseCellError(
            "--execution must enumerate every executions/ JSON exactly once"
        )
    actual_metadata = {
        path.relative_to(cell_root).as_posix()
        for path in artifacts
        if path.is_relative_to(cell_root / "metadata")
    }
    if actual_metadata != _expected_metadata_paths(cell):
        raise ReleaseCellError(
            f"metadata paths do not match {cell_id}'s authoritative contract"
        )
    if cell_id.startswith("prebuilt-"):
        _validate_prebuilt_package_inventory(cell_root, artifacts)
    _validate_required_payloads(cell, artifacts, logs, cell_root)

    conclusion = "success"
    if any(value["timed_out"] for value in executions):
        conclusion = "timed_out"
    elif any(
        value["start_failure"]
        or value["returncode"] != 0
        or bool(value["evidence_errors"])
        for value in executions
    ):
        conclusion = "failure"
    return release_evidence.write_cell_evidence(
        output,
        cell_id=cell_id,
        candidate_sha=candidate_sha,
        conclusion=conclusion,
        artifacts=artifacts,
        logs=logs,
        target=resolved_target,
        crt=resolved_crt,
        evidence_root=cell_root,
    )


def _runtime_source_file(root: Path, relative: str) -> Path:
    safe = _safe_relative(relative, "runtime evidence path")
    if safe.name == "cell.json":
        raise ReleaseCellError("runtime attempt evidence must not contain cell.json")
    path = root.joinpath(*safe.parts)
    resolved = path.resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError as error:
        raise ReleaseCellError(f"runtime evidence escapes attempt root: {relative}") from error
    if path.is_symlink() or not resolved.is_file():
        raise ReleaseCellError(f"runtime evidence is missing: {relative}")
    return resolved


def _validate_runtime_attempt(
    root: Path, *, expected_gate: str, expected_attempt: int
) -> tuple[dict[str, Any], tuple[tuple[str, Path], ...]]:
    root = root.resolve()
    result_path = root / "gate-result.json"
    value = _read_json_object(result_path, "runtime gate result")
    if set(value) != _RUNTIME_RESULT_FIELDS:
        raise ReleaseCellError("runtime gate result has an invalid schema")
    if (
        type(value["schema_version"]) is not int
        or value["schema_version"] != SCHEMA_VERSION
        or value["status"] != "Complete"
    ):
        raise ReleaseCellError("runtime gate result is not a complete schema-v1 result")
    if value["gate"] != expected_gate:
        raise ReleaseCellError(
            f"runtime gate mismatch: expected {expected_gate!r}, found {value['gate']!r}"
        )
    if type(value["attempt"]) is not int or value["attempt"] != expected_attempt:
        raise ReleaseCellError(
            f"runtime attempt mismatch: expected {expected_attempt}, "
            f"found {value['attempt']!r}"
        )
    success = value["success"]
    category = value["category"]
    if (
        not isinstance(success, bool)
        or not isinstance(category, str)
        or category not in _RUNTIME_CATEGORIES
    ):
        raise ReleaseCellError("runtime success/category is invalid")
    if success != (category == "Passed"):
        raise ReleaseCellError("runtime success/category is inconsistent")
    if not isinstance(value["summary"], str) or not value["summary"]:
        raise ReleaseCellError("runtime summary must be non-empty")
    if not isinstance(value["details"], dict):
        raise ReleaseCellError("runtime details must be an object")
    expected_retry = (
        not success
        and category == "InfrastructureUnavailable"
        and expected_attempt == 1
    )
    retry = value["retry"]
    if (
        not isinstance(retry, dict)
        or set(retry) != {"eligible", "max_fresh_runner_attempts"}
        or retry["eligible"] is not expected_retry
        or type(retry["max_fresh_runner_attempts"]) is not int
        or retry["max_fresh_runner_attempts"] != 2
    ):
        raise ReleaseCellError("runtime retry metadata is inconsistent")
    evidence = value["evidence"]
    if not isinstance(evidence, list) or not evidence or any(
        not isinstance(item, str) for item in evidence
    ):
        raise ReleaseCellError("runtime evidence must be a non-empty path array")
    if len(evidence) != len(set(evidence)):
        raise ReleaseCellError("runtime evidence paths must not repeat")
    files = tuple((relative, _runtime_source_file(root, relative)) for relative in evidence)
    actual = {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file() and path.name != "gate-result.json"
    }
    if actual != set(evidence):
        raise ReleaseCellError("runtime gate result does not enumerate exact attempt evidence")
    invocation = _read_json_object(root / "gate-invocation.json", "gate invocation")
    if set(invocation) != {
        "schema_version",
        "status",
        "gate",
        "attempt",
        "process_id",
    } or type(invocation.get("schema_version")) is not int or invocation.get(
        "schema_version"
    ) != SCHEMA_VERSION:
        raise ReleaseCellError("runtime gate invocation has an invalid schema")
    if (
        invocation.get("status") != "Complete"
        or invocation.get("gate") != expected_gate
        or type(invocation.get("attempt")) is not int
        or invocation.get("attempt") != expected_attempt
        or not isinstance(invocation.get("process_id"), int)
        or isinstance(invocation.get("process_id"), bool)
        or invocation["process_id"] <= 0
    ):
        raise ReleaseCellError("runtime gate invocation does not match its result")
    return value, (("gate-result.json", result_path), *files)


def _copy_runtime_evidence(
    *,
    cell_root: Path,
    attempts: Sequence[tuple[int, tuple[tuple[str, Path], ...]]],
    selected: tuple[tuple[str, Path], ...],
) -> None:
    runtime_root = cell_root / "runtime"
    if runtime_root.is_symlink() or (
        hasattr(runtime_root, "is_junction") and runtime_root.is_junction()
    ):
        raise ReleaseCellError("runtime destination must not be a link or junction")
    resolved_runtime = runtime_root.resolve()
    try:
        resolved_runtime.relative_to(cell_root)
    except ValueError as error:
        raise ReleaseCellError("runtime destination escapes the cell root") from error
    destinations: dict[Path, Path] = {}
    for attempt, files in attempts:
        for relative, source in files:
            destination = runtime_root / f"attempt{attempt}" / Path(relative)
            resolved = destination.resolve()
            try:
                resolved.relative_to(resolved_runtime)
            except ValueError as error:
                raise ReleaseCellError("runtime attempt destination escapes runtime/") from error
            destinations[resolved] = source
    for relative, source in selected:
        destination = runtime_root / Path(relative)
        resolved = destination.resolve()
        try:
            resolved.relative_to(resolved_runtime)
        except ValueError as error:
            raise ReleaseCellError("stable runtime destination escapes runtime/") from error
        destinations[resolved] = source
    if runtime_root.exists():
        existing = {
            path.resolve()
            for path in runtime_root.rglob("*")
            if path.is_file()
        }
        unexpected = existing - set(destinations)
        if unexpected:
            rendered = sorted(path.relative_to(cell_root).as_posix() for path in unexpected)
            raise ReleaseCellError(f"runtime destination contains stale files: {rendered!r}")
    for destination, source in sorted(
        destinations.items(), key=lambda item: item[0].as_posix()
    ):
        _copy_regular_file(source, destination)


def _discover_runtime_payloads(
    cell_root: Path, *, output_path: Path
) -> tuple[list[Path], list[Path]]:
    artifacts: list[Path] = []
    logs: list[Path] = []
    for path in sorted(
        (cell_root / "runtime").rglob("*"),
        key=lambda item: item.relative_to(cell_root).as_posix(),
    ):
        if not path.is_file() or path.resolve() == output_path.resolve():
            continue
        if path.is_symlink():
            raise ReleaseCellError(f"runtime evidence must not be a symlink: {path}")
        if path.name.endswith(".log"):
            logs.append(path)
        else:
            artifacts.append(path)
    return artifacts, logs


def finalize_runtime_cell(
    *,
    repo_root: Path,
    candidate_sha: str,
    cell_id: str,
    cell_root: Path,
    attempt1_dir: Path,
    attempt2_dir: Path | None = None,
    output_path: Path | None = None,
) -> dict[str, Any]:
    """Finalize one U9 runtime cell from its machine-readable attempt chain."""
    repo_root = Path(repo_root).resolve()
    candidate_sha = release_evidence.resolve_candidate_sha(repo_root, candidate_sha)
    cell = _authoritative_cell(cell_id)
    try:
        gate = _RUNTIME_CELLS[cell_id]
    except KeyError as error:
        raise ReleaseCellError(f"{cell_id} is not a runtime release cell") from error
    cell_root = Path(cell_root).resolve()
    output = _output_cell_path(cell_root, output_path)
    first, first_files = _validate_runtime_attempt(
        Path(attempt1_dir), expected_gate=gate, expected_attempt=1
    )
    attempts = [(1, first_files)]
    selected = first
    selected_files = first_files
    if first["retry"]["eligible"] is True and attempt2_dir is None:
        raise ReleaseCellError("attempt 1 is retry-eligible and requires attempt 2")
    if attempt2_dir is not None:
        if first["retry"]["eligible"] is not True:
            raise ReleaseCellError("attempt 1 is not retry-eligible")
        second, second_files = _validate_runtime_attempt(
            Path(attempt2_dir), expected_gate=gate, expected_attempt=2
        )
        attempts.append((2, second_files))
        selected = second
        selected_files = second_files
    _copy_runtime_evidence(
        cell_root=cell_root, attempts=attempts, selected=selected_files
    )
    artifacts, logs = _discover_runtime_payloads(cell_root, output_path=output)
    if selected["success"]:
        _validate_required_payloads(cell, artifacts, logs, cell_root)
    if selected["success"]:
        conclusion = "success"
    elif selected["category"] in {"TestTimedOut", "HarnessTimeout"}:
        conclusion = "timed_out"
    else:
        conclusion = "failure"
    return release_evidence.write_cell_evidence(
        output,
        cell_id=cell_id,
        candidate_sha=candidate_sha,
        conclusion=conclusion,
        artifacts=artifacts,
        logs=logs,
        target=cell.target,
        crt=cell.crt,
        evidence_root=cell_root,
    )


def aggregate_cells(
    *,
    repo_root: Path,
    candidate_sha: str,
    evidence_root: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Discover downloaded cell records and aggregate the fixed inventory."""
    repo_root = Path(repo_root).resolve()
    candidate_sha = release_evidence.resolve_candidate_sha(repo_root, candidate_sha)
    evidence_root = Path(evidence_root).resolve()
    if not evidence_root.is_dir():
        raise ReleaseCellError(f"evidence root is missing: {evidence_root}")
    output = _resolve_under(evidence_root, Path(output_path), "aggregate output")
    discovered = sorted(
        (
            path
            for path in evidence_root.rglob("cell.json")
            if path.is_file() and path.resolve() != output.resolve()
        ),
        key=lambda path: path.relative_to(evidence_root).as_posix(),
    )
    return release_evidence.aggregate_release_evidence(
        discovered,
        expected_cells=release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY,
        expected_candidate_sha=candidate_sha,
        evidence_root=evidence_root,
        output_path=output,
    )


def _positive_float(value: str) -> float:
    parsed = float(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def _nonnegative_float(value: str) -> float:
    parsed = float(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("value must be non-negative")
    return parsed


def _add_identity_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repo-root", type=Path, default=WORKSPACE_ROOT)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--cell-id", required=True)
    parser.add_argument("--cell-root", required=True, type=Path)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command_name", required=True)

    capture = commands.add_parser("capture", help="run and retain one bounded command")
    capture.add_argument("--cell-root", required=True, type=Path)
    capture.add_argument("--execution", "--execution-json", required=True, type=Path)
    capture.add_argument("--stdout-log", "--stdout", required=True, type=Path)
    capture.add_argument("--stderr-log", "--stderr", required=True, type=Path)
    capture.add_argument("--timeout", required=True, type=_positive_float)
    capture.add_argument(
        "--termination-grace", type=_nonnegative_float, default=5.0
    )
    capture.add_argument("--cwd", type=Path)
    capture.add_argument("child_command", nargs=argparse.REMAINDER)

    metadata = commands.add_parser(
        "metadata", help="materialize one cell's authoritative metadata"
    )
    _add_identity_arguments(metadata)
    metadata.add_argument("--target")
    metadata.add_argument("--crt", choices=("md", "mt"))
    metadata.add_argument("--vcpkg-root", type=Path)
    metadata.add_argument("--mingw-imports", type=Path)
    metadata.add_argument("--package-dir", type=Path)

    finalize = commands.add_parser(
        "finalize", help="write one normal or prebuilt cell record"
    )
    _add_identity_arguments(finalize)
    finalize.add_argument("--execution", action="append", required=True, type=Path)
    finalize.add_argument("--target")
    finalize.add_argument("--crt", choices=("md", "mt"))
    finalize.add_argument("--output", type=Path)

    runtime = commands.add_parser(
        "finalize-runtime", help="select and retain one U9 runtime attempt chain"
    )
    _add_identity_arguments(runtime)
    runtime.add_argument("--attempt1", "--attempt-1", required=True, type=Path)
    runtime.add_argument("--attempt2", "--attempt-2", type=Path)
    runtime.add_argument("--output", type=Path)

    aggregate = commands.add_parser(
        "aggregate", help="discover cell.json records and aggregate the fixed inventory"
    )
    aggregate.add_argument("--repo-root", type=Path, default=WORKSPACE_ROOT)
    aggregate.add_argument("--candidate-sha", required=True)
    aggregate.add_argument("--evidence-root", required=True, type=Path)
    aggregate.add_argument("--output", required=True, type=Path)
    return parser


def _command_arguments(arguments: Sequence[str]) -> tuple[str, ...]:
    command = list(arguments)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise ReleaseCellError("a command is required after --")
    return tuple(command)


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    arguments = parser.parse_args(argv)
    try:
        if arguments.command_name == "capture":
            return capture_command(
                cell_root=arguments.cell_root,
                execution_path=arguments.execution,
                stdout_log=arguments.stdout_log,
                stderr_log=arguments.stderr_log,
                command=_command_arguments(arguments.child_command),
                timeout=arguments.timeout,
                cwd=arguments.cwd,
                termination_grace=arguments.termination_grace,
            )
        if arguments.command_name == "metadata":
            materialize_metadata(
                repo_root=arguments.repo_root,
                candidate_sha=arguments.candidate_sha,
                cell_id=arguments.cell_id,
                cell_root=arguments.cell_root,
                target=arguments.target,
                crt=arguments.crt,
                vcpkg_root=arguments.vcpkg_root,
                mingw_imports=arguments.mingw_imports,
                package_dir=arguments.package_dir,
            )
            return 0
        if arguments.command_name == "finalize":
            finalize_cell(
                repo_root=arguments.repo_root,
                candidate_sha=arguments.candidate_sha,
                cell_id=arguments.cell_id,
                cell_root=arguments.cell_root,
                execution_paths=arguments.execution,
                target=arguments.target,
                crt=arguments.crt,
                output_path=arguments.output,
            )
            return 0
        if arguments.command_name == "finalize-runtime":
            finalize_runtime_cell(
                repo_root=arguments.repo_root,
                candidate_sha=arguments.candidate_sha,
                cell_id=arguments.cell_id,
                cell_root=arguments.cell_root,
                attempt1_dir=arguments.attempt1,
                attempt2_dir=arguments.attempt2,
                output_path=arguments.output,
            )
            return 0
        result = aggregate_cells(
            repo_root=arguments.repo_root,
            candidate_sha=arguments.candidate_sha,
            evidence_root=arguments.evidence_root,
            output_path=arguments.output,
        )
        return 0 if result["decision"] == "Go" else 1
    except (
        ReleaseCellError,
        release_evidence.EvidenceError,
        VerificationError,
        OSError,
    ) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
