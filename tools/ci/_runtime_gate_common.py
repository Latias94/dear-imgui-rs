"""Shared evidence and process contracts for native runtime gates."""

from __future__ import annotations

import json
import os
from collections.abc import Mapping, Sequence
from dataclasses import asdict, dataclass, field, replace
from enum import Enum
from pathlib import Path

from _process import BoundedProcessResult, run_bounded
from _verification import parse_candidate_sha, write_json


class GateCategory(str, Enum):
    """Stable classifications reported by native runtime gates."""

    PASSED = "Passed"
    TEST_TIMED_OUT = "TestTimedOut"
    HARNESS_TIMEOUT = "HarnessTimeout"
    INFRASTRUCTURE_UNAVAILABLE = "InfrastructureUnavailable"
    PRODUCT_FAILURE = "ProductFailure"


@dataclass(frozen=True)
class GateResult:
    """Stable, machine-readable result for one native runtime gate."""

    gate: str
    success: bool
    category: GateCategory
    summary: str
    attempt: int = 1
    details: Mapping[str, object] = field(default_factory=dict)
    evidence: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.attempt <= 0:
            raise ValueError("attempt must be greater than zero")

    @property
    def retry_eligible(self) -> bool:
        return (
            not self.success
            and self.category is GateCategory.INFRASTRUCTURE_UNAVAILABLE
            and self.attempt == 1
        )

    def to_json(self) -> dict[str, object]:
        return {
            "schema_version": 1,
            "status": "Complete",
            "gate": self.gate,
            "success": self.success,
            "category": self.category.value,
            "attempt": self.attempt,
            "summary": self.summary,
            "retry": {"eligible": self.retry_eligible},
            "evidence": list(self.evidence),
            "details": dict(self.details),
        }


class RuntimeContractError(RuntimeError):
    """A classified runtime gate failure."""

    def __init__(self, category: GateCategory, message: str) -> None:
        self.category = category
        super().__init__(message)


_FAILURE_PRIORITY = {
    GateCategory.HARNESS_TIMEOUT: 4,
    GateCategory.INFRASTRUCTURE_UNAVAILABLE: 3,
    GateCategory.TEST_TIMED_OUT: 2,
    GateCategory.PRODUCT_FAILURE: 1,
    GateCategory.PASSED: 0,
}


def _evidence_files(evidence_dir: Path) -> tuple[str, ...]:
    return tuple(
        path.relative_to(evidence_dir).as_posix()
        for path in sorted(evidence_dir.rglob("*"))
        if path.is_file() and path.name != "gate-result.json"
    )


def _finalize(result: GateResult, evidence_dir: Path, candidate_sha: str) -> GateResult:
    candidate_sha = parse_candidate_sha(candidate_sha)
    write_json(
        evidence_dir / "gate-invocation.json",
        {
            "schema_version": 1,
            "status": "Complete",
            "gate": result.gate,
            "attempt": result.attempt,
            "candidate_sha": candidate_sha,
        },
    )
    result = replace(result, evidence=_evidence_files(evidence_dir))
    result_json = result.to_json()
    result_json["candidate_sha"] = candidate_sha
    write_json(evidence_dir / "gate-result.json", result_json)
    return result


def _prepare_evidence(
    *,
    evidence_dir: Path,
    gate: str,
    attempt: int,
    candidate_sha: str,
    owned_files: Sequence[str],
) -> None:
    candidate_sha = parse_candidate_sha(candidate_sha)
    evidence_dir.mkdir(parents=True, exist_ok=True)
    for name in ("gate-result.json", "gate-invocation.json", *owned_files):
        (evidence_dir / name).unlink(missing_ok=True)
    write_json(
        evidence_dir / "gate-invocation.json",
        {
            "schema_version": 1,
            "status": "Running",
            "gate": gate,
            "attempt": attempt,
            "candidate_sha": candidate_sha,
        },
    )


def record_runtime_preparation_failure(
    *,
    gate: str,
    attempt: int,
    candidate_sha: str,
    evidence_dir: Path,
    summary: str,
) -> GateResult:
    """Record a gate skipped because its runner preparation failed."""
    _prepare_evidence(
        evidence_dir=evidence_dir,
        gate=gate,
        attempt=attempt,
        candidate_sha=candidate_sha,
        owned_files=(),
    )
    return _finalize(
        GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            summary,
            attempt,
            details={"phase": "preparation"},
        ),
        evidence_dir,
        candidate_sha,
    )


def _reject_excess_attempt(
    *,
    gate: str,
    attempt: int,
    candidate_sha: str,
    evidence_dir: Path,
) -> GateResult | None:
    if attempt <= 2:
        return None
    return _finalize(
        GateResult(
            gate,
            False,
            GateCategory.PRODUCT_FAILURE,
            "runtime gates permit only one fresh-runner retry",
            attempt,
        ),
        evidence_dir,
        candidate_sha,
    )


def _process_json(
    result: BoundedProcessResult,
    evidence_dir: Path,
) -> dict[str, object]:
    def relative(path: Path) -> str:
        try:
            return path.relative_to(evidence_dir).as_posix()
        except ValueError:
            return str(path)

    return {
        "command": list(result.args),
        "returncode": result.returncode,
        "timed_out": result.timed_out,
        "duration_seconds": round(result.duration, 3),
        "stdout_log": relative(result.stdout_log),
        "stderr_log": relative(result.stderr_log),
        "stream_errors": list(result.stream_errors),
        "termination": asdict(result.termination),
    }


def _background_json(process: object, evidence_dir: Path) -> dict[str, object]:
    stdout_log = Path(getattr(process, "stdout_log"))
    stderr_log = Path(getattr(process, "stderr_log"))
    termination = getattr(process, "termination")
    return {
        "command": list(getattr(process, "args")),
        "returncode": getattr(process, "returncode"),
        "stdout_log": stdout_log.relative_to(evidence_dir).as_posix(),
        "stderr_log": stderr_log.relative_to(evidence_dir).as_posix(),
        "stream_errors": list(getattr(process, "stream_errors")),
        "termination": asdict(termination) if termination is not None else None,
    }


def _target_directory(workspace_root: Path) -> Path:
    configured = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    if not configured.is_absolute():
        configured = workspace_root / configured
    return configured


def _example_binary(workspace_root: Path, name: str) -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    return _target_directory(workspace_root) / "debug" / f"{name}{suffix}"


def _sdl3_runtime_library_directories(workspace_root: Path) -> tuple[Path, ...]:
    build_root = _target_directory(workspace_root) / "debug" / "build"
    directories = {
        library.parent.resolve()
        for library in build_root.glob("sdl3-sys-*/out/**/libSDL3.so.0")
        if library.is_file()
    }
    if not directories:
        raise RuntimeContractError(
            GateCategory.PRODUCT_FAILURE,
            "cargo succeeded without producing the bundled SDL3 runtime library",
        )
    return tuple(sorted(directories, key=lambda path: str(path)))


def _run_example_build(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    binary: str,
    features: str,
    timeout: float,
    child_environment: Mapping[str, str],
) -> BoundedProcessResult:
    return run_bounded(
        (
            "cargo",
            "build",
            "-p",
            "dear-imgui-examples",
            "--bin",
            binary,
            "--features",
            features,
        ),
        cwd=workspace_root,
        env=child_environment,
        timeout=timeout,
        stdout_log=evidence_dir / "build.stdout.log",
        stderr_log=evidence_dir / "build.stderr.log",
    )


def _check_stage(
    result: BoundedProcessResult,
    *,
    label: str,
    nonzero_category: GateCategory,
) -> None:
    if result.timed_out:
        raise RuntimeContractError(
            GateCategory.HARNESS_TIMEOUT,
            f"{label} exceeded its harness timeout",
        )
    if result.stream_errors or result.termination.errors:
        messages = (*result.stream_errors, *result.termination.errors)
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"{label} could not retain complete process evidence: {'; '.join(messages)}",
        )
    if result.returncode != 0:
        raise RuntimeContractError(
            nonzero_category,
            f"{label} exited with status {result.returncode}",
        )


def _read_object(path: Path) -> dict[str, object]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise RuntimeContractError(
            GateCategory.PRODUCT_FAILURE,
            f"child did not write {path.name}",
        ) from error
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise RuntimeContractError(
            GateCategory.PRODUCT_FAILURE,
            f"child wrote an invalid {path.name}: {error}",
        ) from error
    if not isinstance(payload, dict):
        raise RuntimeContractError(
            GateCategory.PRODUCT_FAILURE,
            f"{path.name} must contain a JSON object",
        )
    return payload


def _highest_failure(
    failures: Sequence[tuple[GateCategory, str]],
) -> tuple[GateCategory, str]:
    return max(failures, key=lambda failure: _FAILURE_PRIORITY[failure[0]])
