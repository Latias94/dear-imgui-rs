"""Create and aggregate deterministic release-gate evidence.

The module deliberately takes every release input as an argument. It does not
discover workflow jobs, read process environment variables, or access the
network. This keeps the release decision reproducible from an archived evidence
directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any


SCHEMA_VERSION = 1
GATE_RESULT_NAME = "gate-result.json"
_CANDIDATE_SHA = re.compile(r"[0-9a-f]{40}")
_CHECKSUM = re.compile(r"[0-9a-f]{64}")
_CONCLUSIONS = frozenset(
    {"success", "failure", "failed", "skipped", "cancelled", "timed_out"}
)
_REQUIRED_CELL_FIELDS = frozenset(
    {"version", "cell_id", "candidate_sha", "conclusion", "artifacts", "logs"}
)
_OPTIONAL_CELL_FIELDS = frozenset({"target", "crt"})
_FILE_FIELDS = frozenset({"path", "sha256"})


class EvidenceError(ValueError):
    """A release-evidence input violates the audited schema."""


@dataclass(frozen=True)
class ExpectedCell:
    """One cell required by the release gate."""

    cell_id: str
    target: str | None = None
    crt: str | None = None


DEFAULT_EXPECTED_CELL_INVENTORY = (
    ExpectedCell("linux-test-engine-runtime", "x86_64-unknown-linux-gnu"),
    ExpectedCell("linux-multi-viewport-smoke", "x86_64-unknown-linux-gnu"),
    ExpectedCell("linux-wasm", "wasm32-unknown-unknown"),
    ExpectedCell("windows-vcpkg", "x86_64-pc-windows-msvc", "md"),
    ExpectedCell("windows-platform-md", "x86_64-pc-windows-msvc", "md"),
    ExpectedCell("windows-platform-mt", "x86_64-pc-windows-msvc", "mt"),
    ExpectedCell("windows-gnu", "x86_64-pc-windows-gnu"),
    ExpectedCell("macos-build"),
    ExpectedCell("prebuilt-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
    ExpectedCell("prebuilt-x86_64-apple-darwin", "x86_64-apple-darwin"),
    ExpectedCell("prebuilt-aarch64-apple-darwin", "aarch64-apple-darwin"),
    ExpectedCell(
        "prebuilt-x86_64-pc-windows-msvc-md", "x86_64-pc-windows-msvc", "md"
    ),
    ExpectedCell(
        "prebuilt-x86_64-pc-windows-msvc-mt", "x86_64-pc-windows-msvc", "mt"
    ),
)
DEFAULT_EXPECTED_CELL_IDS = tuple(
    cell.cell_id for cell in DEFAULT_EXPECTED_CELL_INVENTORY
)


Runner = Callable[..., subprocess.CompletedProcess[str]]


def parse_candidate_sha(value: str) -> str:
    """Return a strictly formatted, full lowercase Git object ID."""
    if not isinstance(value, str) or _CANDIDATE_SHA.fullmatch(value) is None:
        raise EvidenceError(
            "candidate SHA must be exactly 40 lowercase hexadecimal characters"
        )
    return value


def resolve_candidate_sha(
    repo_root: Path,
    expected_candidate_sha: str,
    *,
    runner: Runner = subprocess.run,
) -> str:
    """Resolve the repository HEAD and require the expected candidate SHA."""
    expected = parse_candidate_sha(expected_candidate_sha)
    try:
        completed = runner(
            ["git", "rev-parse", "--verify", "HEAD"],
            cwd=Path(repo_root),
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except OSError as error:
        raise EvidenceError(f"could not execute git to resolve candidate HEAD: {error}") from error
    if completed.returncode != 0:
        diagnostic = (completed.stderr or "").strip()
        suffix = f": {diagnostic}" if diagnostic else ""
        raise EvidenceError(f"could not resolve candidate HEAD{suffix}")
    output = completed.stdout
    if not isinstance(output, str):
        raise EvidenceError("git returned non-text candidate HEAD output")
    if output.endswith("\r\n"):
        output = output[:-2]
    elif output.endswith("\n"):
        output = output[:-1]
    actual = parse_candidate_sha(output)
    if actual != expected:
        raise EvidenceError(
            f"candidate HEAD mismatch: expected {expected}, found {actual}"
        )
    return actual


def sha256_file(path: Path, *, chunk_size: int = 1024 * 1024) -> str:
    """Hash one regular file without loading its contents into memory."""
    if chunk_size <= 0:
        raise ValueError("chunk_size must be positive")
    path = Path(path)
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            before = os.fstat(source.fileno())
            if not path.is_file():
                raise EvidenceError(f"evidence payload is not a regular file: {path}")
            while chunk := source.read(chunk_size):
                digest.update(chunk)
            after = os.fstat(source.fileno())
    except EvidenceError:
        raise
    except OSError as error:
        raise EvidenceError(f"could not hash evidence payload {path}: {error}") from error
    identity_before = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    )
    if identity_before != identity_after:
        raise EvidenceError(f"evidence payload changed while hashing: {path}")
    return digest.hexdigest()


def _validate_text(value: Any, field: str, *, optional: bool = False) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or not value or value != value.strip():
        raise EvidenceError(f"{field} must be a non-empty string without outer whitespace")
    if any(ord(character) < 0x20 for character in value):
        raise EvidenceError(f"{field} must not contain control characters")
    return value


def _safe_relative_path(value: Any, field: str) -> PurePosixPath:
    if not isinstance(value, str) or not value:
        raise EvidenceError(f"{field} path must be a non-empty string")
    if "\\" in value:
        raise EvidenceError(f"{field} path must use forward slashes: {value!r}")
    windows_path = PureWindowsPath(value)
    if value.startswith("/") or windows_path.drive:
        raise EvidenceError(f"{field} path must be relative: {value!r}")
    raw_parts = value.split("/")
    if any(part in ("", ".", "..") for part in raw_parts):
        raise EvidenceError(f"{field} path contains an unsafe component: {value!r}")
    return PurePosixPath(*raw_parts)


def _path_within_root(path: Path, root: Path, field: str) -> Path:
    resolved_root = root.resolve()
    resolved_path = path.resolve()
    try:
        resolved_path.relative_to(resolved_root)
    except ValueError as error:
        raise EvidenceError(f"{field} path escapes evidence root: {path}") from error
    return resolved_path


def _payload_record(path: Path, root: Path, field: str) -> dict[str, str]:
    resolved = _path_within_root(Path(path), root, field)
    if not resolved.is_file():
        raise EvidenceError(f"{field} payload is not a regular file: {path}")
    relative = resolved.relative_to(root.resolve()).as_posix()
    _safe_relative_path(relative, field)
    return {"path": relative, "sha256": sha256_file(resolved)}


def _stable_json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def atomic_write_json(path: Path, value: Any) -> None:
    """Atomically replace a JSON file using deterministic UTF-8 and LF output."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb", prefix=f".{path.name}.", suffix=".tmp", dir=path.parent, delete=False
        ) as destination:
            temporary_path = Path(destination.name)
            destination.write(_stable_json_bytes(value))
            destination.flush()
            os.fsync(destination.fileno())
        os.replace(temporary_path, path)
        temporary_path = None
    finally:
        if temporary_path is not None:
            try:
                temporary_path.unlink()
            except FileNotFoundError:
                pass


def write_cell_evidence(
    output_path: Path,
    *,
    cell_id: str,
    candidate_sha: str,
    conclusion: str,
    artifacts: Sequence[Path] = (),
    logs: Sequence[Path] = (),
    target: str | None = None,
    crt: str | None = None,
    evidence_root: Path | None = None,
) -> dict[str, Any]:
    """Write one checksummed cell record and return its schema value."""
    output_path = Path(output_path)
    root = Path(evidence_root) if evidence_root is not None else output_path.parent
    root = root.resolve()
    _path_within_root(output_path, root, "cell evidence")
    cell_root = output_path.parent.resolve()
    validated_cell_id = _validate_text(cell_id, "cell_id")
    validated_conclusion = _validate_text(conclusion, "conclusion")
    if validated_conclusion not in _CONCLUSIONS:
        raise EvidenceError(f"unsupported conclusion: {validated_conclusion}")
    record: dict[str, Any] = {
        "version": SCHEMA_VERSION,
        "cell_id": validated_cell_id,
        "candidate_sha": parse_candidate_sha(candidate_sha),
        "conclusion": validated_conclusion,
        "artifacts": sorted(
            (_payload_record(path, cell_root, "artifact") for path in artifacts),
            key=lambda item: item["path"],
        ),
        "logs": sorted(
            (_payload_record(path, cell_root, "log") for path in logs),
            key=lambda item: item["path"],
        ),
    }
    validated_target = _validate_text(target, "target", optional=True)
    validated_crt = _validate_text(crt, "crt", optional=True)
    if validated_target is not None:
        record["target"] = validated_target
    if validated_crt is not None:
        record["crt"] = validated_crt
    atomic_write_json(output_path, record)
    return record


def _reject_duplicate_object_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def _load_json_object(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8", newline="") as source:
            value = json.load(source, object_pairs_hook=_reject_duplicate_object_keys)
    except EvidenceError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"could not read cell evidence: {error}") from error
    if not isinstance(value, dict):
        raise EvidenceError("cell evidence root must be a JSON object")
    return value


def _validate_payload_entries(
    entries: Any,
    *,
    field: str,
    evidence_path: Path,
) -> list[str]:
    errors: list[str] = []
    if not isinstance(entries, list):
        return [f"{field} must be a JSON array"]
    seen: set[str] = set()
    for index, entry in enumerate(entries):
        label = f"{field}[{index}]"
        if not isinstance(entry, dict):
            errors.append(f"{label} must be a JSON object")
            continue
        if set(entry) != _FILE_FIELDS:
            errors.append(f"{label} must contain exactly path and sha256")
            continue
        try:
            relative = _safe_relative_path(entry["path"], label)
        except EvidenceError as error:
            errors.append(str(error))
            continue
        relative_text = relative.as_posix()
        if relative_text in seen:
            errors.append(f"{field} repeats payload path {relative_text!r}")
            continue
        seen.add(relative_text)
        checksum = entry["sha256"]
        if not isinstance(checksum, str) or _CHECKSUM.fullmatch(checksum) is None:
            errors.append(f"{label} sha256 must be 64 lowercase hexadecimal characters")
            continue
        candidate = evidence_path.parent.joinpath(*relative.parts)
        try:
            resolved = _path_within_root(candidate, evidence_path.parent, label)
        except EvidenceError as error:
            errors.append(str(error))
            continue
        if not resolved.is_file():
            errors.append(f"{label} payload is missing or not a regular file: {relative_text}")
            continue
        try:
            actual = sha256_file(resolved)
        except EvidenceError as error:
            errors.append(str(error))
            continue
        if actual != checksum:
            errors.append(
                f"{label} checksum mismatch: expected {checksum}, found {actual}"
            )
    return errors


def _validate_cell_record(
    value: dict[str, Any], evidence_path: Path
) -> tuple[str | None, list[str]]:
    errors: list[str] = []
    fields = set(value)
    missing = sorted(_REQUIRED_CELL_FIELDS - fields)
    unexpected = sorted(fields - _REQUIRED_CELL_FIELDS - _OPTIONAL_CELL_FIELDS)
    if missing:
        errors.append("missing fields: " + ", ".join(missing))
    if unexpected:
        errors.append("unexpected fields: " + ", ".join(unexpected))
    version = value.get("version")
    if isinstance(version, bool) or version != SCHEMA_VERSION:
        errors.append(f"version must be {SCHEMA_VERSION}")
    cell_id: str | None = None
    try:
        cell_id = _validate_text(value.get("cell_id"), "cell_id")
    except EvidenceError as error:
        errors.append(str(error))
    try:
        parse_candidate_sha(value.get("candidate_sha"))
    except EvidenceError as error:
        errors.append(str(error))
    conclusion = value.get("conclusion")
    try:
        validated_conclusion = _validate_text(conclusion, "conclusion")
        if validated_conclusion not in _CONCLUSIONS:
            errors.append(f"unsupported conclusion: {validated_conclusion}")
    except EvidenceError as error:
        errors.append(str(error))
    for optional in ("target", "crt"):
        if optional in value:
            try:
                _validate_text(value[optional], optional)
            except EvidenceError as error:
                errors.append(str(error))
    for field in ("artifacts", "logs"):
        errors.extend(
            _validate_payload_entries(
                value.get(field),
                field=field,
                evidence_path=evidence_path,
            )
        )
    return cell_id, errors


def _display_path(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def _normalize_expected_cell(value: str | ExpectedCell) -> ExpectedCell:
    if isinstance(value, str):
        value = ExpectedCell(value)
    if not isinstance(value, ExpectedCell):
        raise EvidenceError("expected cell inventory entries must be strings or ExpectedCell")
    return ExpectedCell(
        cell_id=_validate_text(value.cell_id, "expected cell_id"),
        target=_validate_text(value.target, "expected target", optional=True),
        crt=_validate_text(value.crt, "expected crt", optional=True),
    )


def aggregate_release_evidence(
    evidence_paths: Sequence[Path],
    *,
    expected_cells: Sequence[str | ExpectedCell],
    expected_candidate_sha: str,
    evidence_root: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Aggregate an explicit cell inventory and atomically write the gate result."""
    expected_sha = parse_candidate_sha(expected_candidate_sha)
    evidence_root = Path(evidence_root).resolve()
    normalized_expected: list[ExpectedCell] = []
    inventory_errors: list[str] = []
    for index, value in enumerate(expected_cells):
        try:
            normalized_expected.append(_normalize_expected_cell(value))
        except EvidenceError as error:
            inventory_errors.append(f"expected_cells[{index}]: {error}")
    normalized_expected.sort(
        key=lambda cell: (cell.cell_id, cell.target or "", cell.crt or "")
    )
    expected_by_id: dict[str, ExpectedCell] = {}
    for expected in normalized_expected:
        if expected.cell_id in expected_by_id:
            inventory_errors.append(
                f"expected cell inventory repeats {expected.cell_id!r}"
            )
        else:
            expected_by_id[expected.cell_id] = expected
    if not normalized_expected:
        inventory_errors.append("expected cell inventory must not be empty")

    loaded: list[tuple[Path, dict[str, Any], list[str]]] = []
    malformed: list[tuple[Path, list[str]]] = []
    for supplied_path in sorted((Path(path) for path in evidence_paths), key=str):
        display = _display_path(supplied_path, evidence_root)
        try:
            resolved = _path_within_root(supplied_path, evidence_root, "cell evidence")
            if not resolved.is_file():
                raise EvidenceError(f"cell evidence is missing or not a regular file: {display}")
            value = _load_json_object(resolved)
            cell_id, errors = _validate_cell_record(value, resolved)
            if cell_id is None:
                malformed.append((supplied_path, errors))
            else:
                loaded.append((resolved, value, errors))
        except EvidenceError as error:
            malformed.append((supplied_path, [str(error)]))

    records_by_id: dict[str, list[tuple[Path, dict[str, Any], list[str]]]] = {}
    for item in loaded:
        records_by_id.setdefault(item[1]["cell_id"], []).append(item)

    checks: list[dict[str, Any]] = []
    if inventory_errors:
        checks.append(
            {
                "cell_id": "__inventory__",
                "conclusion": None,
                "evidence_paths": [],
                "errors": sorted(inventory_errors),
                "status": "failure",
            }
        )
    emitted_expected: set[str] = set()
    for expected in normalized_expected:
        if expected.cell_id in emitted_expected:
            continue
        emitted_expected.add(expected.cell_id)
        records = records_by_id.get(expected.cell_id, [])
        evidence_names = sorted(
            _display_path(path, evidence_root) for path, _value, _errors in records
        )
        errors: list[str] = []
        conclusion: str | None = None
        if not records:
            errors.append("required cell evidence is missing")
        elif len(records) > 1:
            errors.append(f"duplicate cell evidence: found {len(records)} records")
            for _path, _value, record_errors in records:
                errors.extend(record_errors)
        else:
            _path, value, record_errors = records[0]
            errors.extend(record_errors)
            conclusion_value = value.get("conclusion")
            conclusion = conclusion_value if isinstance(conclusion_value, str) else None
            candidate = value.get("candidate_sha")
            if candidate != expected_sha:
                errors.append(
                    f"candidate SHA mismatch: expected {expected_sha}, found {candidate!r}"
                )
            if conclusion != "success":
                errors.append(f"cell conclusion is not success: {conclusion!r}")
            for field in ("target", "crt"):
                expected_value = getattr(expected, field)
                if expected_value is not None and value.get(field) != expected_value:
                    errors.append(
                        f"{field} mismatch: expected {expected_value!r}, "
                        f"found {value.get(field)!r}"
                    )
        checks.append(
            {
                "cell_id": expected.cell_id,
                "conclusion": conclusion,
                "evidence_paths": evidence_names,
                "errors": sorted(set(errors)),
                "status": "success" if not errors else "failure",
            }
        )

    for cell_id in sorted(set(records_by_id) - set(expected_by_id)):
        records = records_by_id[cell_id]
        errors = ["cell is not present in the expected inventory"]
        for _path, _value, record_errors in records:
            errors.extend(record_errors)
        if len(records) > 1:
            errors.append(f"duplicate cell evidence: found {len(records)} records")
        conclusions = {
            value.get("conclusion")
            for _path, value, _errors in records
            if isinstance(value.get("conclusion"), str)
        }
        checks.append(
            {
                "cell_id": cell_id,
                "conclusion": next(iter(conclusions)) if len(conclusions) == 1 else None,
                "evidence_paths": sorted(
                    _display_path(path, evidence_root)
                    for path, _value, _errors in records
                ),
                "errors": sorted(set(errors)),
                "status": "failure",
            }
        )

    for path, errors in sorted(malformed, key=lambda item: str(item[0])):
        checks.append(
            {
                "cell_id": None,
                "conclusion": None,
                "evidence_paths": [_display_path(path, evidence_root)],
                "errors": sorted(set(errors)),
                "status": "failure",
            }
        )

    successful = sum(check["status"] == "success" for check in checks)
    failed = len(checks) - successful
    result = {
        "version": SCHEMA_VERSION,
        "candidate_sha": expected_sha,
        "decision": "Go" if failed == 0 else "No-Go",
        "checks": checks,
        "summary": {
            "expected_cells": len(expected_by_id),
            "successful_checks": successful,
            "failed_checks": failed,
        },
    }
    atomic_write_json(output_path, result)
    return result


def _expected_cell_argument(value: str) -> ExpectedCell:
    parts = value.split(":")
    if not 1 <= len(parts) <= 3 or any(not part for part in parts):
        raise argparse.ArgumentTypeError("use CELL_ID[:TARGET[:CRT]]")
    return ExpectedCell(
        parts[0],
        parts[1] if len(parts) >= 2 else None,
        parts[2] if len(parts) == 3 else None,
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    record = subparsers.add_parser("record", help="write one cell evidence record")
    record.add_argument("--repo-root", required=True, type=Path)
    record.add_argument("--candidate-sha", required=True)
    record.add_argument("--output", required=True, type=Path)
    record.add_argument("--evidence-root", required=True, type=Path)
    record.add_argument("--cell-id", required=True)
    record.add_argument("--conclusion", required=True, choices=sorted(_CONCLUSIONS))
    record.add_argument("--target")
    record.add_argument("--crt")
    record.add_argument("--artifact", action="append", default=[], type=Path)
    record.add_argument("--log", action="append", default=[], type=Path)

    aggregate = subparsers.add_parser("aggregate", help="write the release gate result")
    aggregate.add_argument("--repo-root", required=True, type=Path)
    aggregate.add_argument("--candidate-sha", required=True)
    aggregate.add_argument("--evidence-root", required=True, type=Path)
    aggregate.add_argument("--output", required=True, type=Path)
    aggregate.add_argument(
        "--expected-cell",
        required=True,
        action="append",
        type=_expected_cell_argument,
        help="required CELL_ID[:TARGET[:CRT]]; repeat for every release cell",
    )
    aggregate.add_argument("evidence", nargs="+", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the explicit record or aggregate command."""
    parser = _build_parser()
    arguments = parser.parse_args(argv)
    try:
        candidate_sha = resolve_candidate_sha(
            arguments.repo_root, arguments.candidate_sha
        )
        if arguments.command == "record":
            write_cell_evidence(
                arguments.output,
                cell_id=arguments.cell_id,
                candidate_sha=candidate_sha,
                conclusion=arguments.conclusion,
                artifacts=arguments.artifact,
                logs=arguments.log,
                target=arguments.target,
                crt=arguments.crt,
                evidence_root=arguments.evidence_root,
            )
            return 0
        result = aggregate_release_evidence(
            arguments.evidence,
            expected_cells=arguments.expected_cell,
            expected_candidate_sha=candidate_sha,
            evidence_root=arguments.evidence_root,
            output_path=arguments.output,
        )
        return 0 if result["decision"] == "Go" else 1
    except EvidenceError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
