"""Native Test Engine and multi-viewport runtime release gates."""

from __future__ import annotations

import json
import os
import platform
import re
import shutil
import sys
import tempfile
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import asdict, dataclass, field, replace
from enum import Enum
from pathlib import Path

from _process import (
    BoundedProcessResult,
    ProcessStartError,
    environment,
    managed_background,
    run_bounded,
)
from release_evidence import atomic_write_json


class GateCategory(str, Enum):
    """Stable classifications consumed by release aggregation."""

    PASSED = "Passed"
    TEST_TIMED_OUT = "TestTimedOut"
    HARNESS_TIMEOUT = "HarnessTimeout"
    INFRASTRUCTURE_UNAVAILABLE = "InfrastructureUnavailable"
    PRODUCT_FAILURE = "ProductFailure"


class ViewportSmokeProfile(str, Enum):
    """Runtime routing profile for one viewport smoke contract."""

    WGPU_VULKAN = "WgpuVulkan"
    SDL3_GLOW = "Sdl3Glow"
    ASH_VULKAN = "AshVulkan"


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
            "retry": {
                "eligible": self.retry_eligible,
                "max_fresh_runner_attempts": 2,
            },
            "evidence": list(self.evidence),
            "details": dict(self.details),
        }


class RuntimeContractError(RuntimeError):
    """A classified runtime gate failure."""

    def __init__(self, category: GateCategory, message: str) -> None:
        self.category = category
        super().__init__(message)


@dataclass(frozen=True)
class ScenarioExpectation:
    name: str
    returncode: int
    outcome: str
    infrastructure: bool
    category: GateCategory


@dataclass(frozen=True)
class ViewportSmokeSpec:
    """Backend-specific contract layered over the shared real-window harness."""

    profile: ViewportSmokeProfile
    gate: str
    binary: str
    features: str
    package_names: tuple[str, ...]
    probe_tool: str
    probe_arguments: tuple[str, ...]
    probe_log_stem: str
    probe_label: str
    probe_identities: tuple[str, ...]
    probe_identity_error: str
    probe_required_fragments: tuple[str, ...]
    build_label: str
    child_label: str
    success_summary: str
    payload_validator: Callable[[Mapping[str, object]], list[str]]


TEST_ENGINE_SCENARIOS = (
    ScenarioExpectation("pass", 0, "Passed", False, GateCategory.PASSED),
    ScenarioExpectation("failure", 2, "Failed", False, GateCategory.PRODUCT_FAILURE),
    ScenarioExpectation("no-match", 2, "NoMatch", False, GateCategory.PRODUCT_FAILURE),
    ScenarioExpectation("timeout", 2, "TimedOut", False, GateCategory.TEST_TIMED_OUT),
    ScenarioExpectation("abort", 2, "Aborted", False, GateCategory.PRODUCT_FAILURE),
    ScenarioExpectation(
        "ffi-failure",
        3,
        "InfrastructureError",
        True,
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
    ),
    ScenarioExpectation(
        "callback-error",
        3,
        "InfrastructureError",
        True,
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
    ),
)


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


def _finalize(result: GateResult, evidence_dir: Path) -> GateResult:
    atomic_write_json(
        evidence_dir / "gate-invocation.json",
        {
            "schema_version": 1,
            "status": "Complete",
            "gate": result.gate,
            "attempt": result.attempt,
            "process_id": os.getpid(),
        },
    )
    result = replace(result, evidence=_evidence_files(evidence_dir))
    atomic_write_json(evidence_dir / "gate-result.json", result.to_json())
    return result


def _prepare_evidence(
    *,
    evidence_dir: Path,
    gate: str,
    attempt: int,
    owned_files: Sequence[str],
) -> None:
    evidence_dir.mkdir(parents=True, exist_ok=True)
    for name in ("gate-result.json", "gate-invocation.json", *owned_files):
        (evidence_dir / name).unlink(missing_ok=True)
    atomic_write_json(
        evidence_dir / "gate-invocation.json",
        {
            "schema_version": 1,
            "status": "Running",
            "gate": gate,
            "attempt": attempt,
            "process_id": os.getpid(),
        },
    )


def record_runtime_preparation_failure(
    *,
    gate: str,
    attempt: int,
    evidence_dir: Path,
    summary: str,
) -> GateResult:
    """Record a gate skipped because its runner preparation failed."""
    _prepare_evidence(
        evidence_dir=evidence_dir,
        gate=gate,
        attempt=attempt,
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
    )


def _reject_excess_attempt(
    *,
    gate: str,
    attempt: int,
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
    )


def _termination_json(result: BoundedProcessResult) -> dict[str, object]:
    return asdict(result.termination)


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
        "termination": _termination_json(result),
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


def _observed_test_category(payload: Mapping[str, object]) -> GateCategory:
    if payload.get("outcome") == "TimedOut":
        return GateCategory.TEST_TIMED_OUT
    if payload.get("infrastructure") is True:
        return GateCategory.INFRASTRUCTURE_UNAVAILABLE
    if payload.get("outcome") == "Passed":
        return GateCategory.PASSED
    return GateCategory.PRODUCT_FAILURE


def _validate_test_engine_payload(
    expectation: ScenarioExpectation,
    result: BoundedProcessResult,
    payload: Mapping[str, object],
) -> list[str]:
    errors: list[str] = []
    schema_version = payload.get("schema_version")
    if type(schema_version) is not int or schema_version != 1:
        errors.append(f"schema_version expected 1, got {schema_version!r}")
    outcome = payload.get("outcome")
    if outcome != expectation.outcome:
        errors.append(f"outcome expected {expectation.outcome!r}, got {outcome!r}")
    infrastructure = payload.get("infrastructure")
    if infrastructure is not expectation.infrastructure:
        errors.append(
            f"infrastructure expected {expectation.infrastructure!r}, "
            f"got {infrastructure!r}"
        )
    if result.returncode != expectation.returncode:
        errors.append(
            f"exit status expected {expectation.returncode}, got {result.returncode}"
        )

    observed_category = _observed_test_category(payload)
    if observed_category is not expectation.category:
        errors.append(
            f"category expected {expectation.category.value!r}, "
            f"got {observed_category.value!r}"
        )

    tested = payload.get("tested")
    succeeded = payload.get("success")
    count_fields = ("tested", "success", "in_queue", "frames", "cleanup_frames")
    for field_name in count_fields:
        value = payload.get(field_name)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"{field_name} must be a nonnegative integer")
    if expectation.outcome == "Passed" and (
        not isinstance(tested, int)
        or isinstance(tested, bool)
        or tested <= 0
        or not isinstance(succeeded, int)
        or isinstance(succeeded, bool)
        or succeeded != tested
    ):
        errors.append("Passed requires a nonzero tested count with every test successful")
    elif expectation.outcome == "Failed" and (
        not isinstance(tested, int)
        or isinstance(tested, bool)
        or tested <= 0
        or not isinstance(succeeded, int)
        or isinstance(succeeded, bool)
        or succeeded >= tested
    ):
        errors.append("Failed requires at least one executed, unsuccessful test")
    elif expectation.outcome == "NoMatch" and (tested != 0 or succeeded != 0):
        errors.append("NoMatch requires zero tested and zero successful tests")
    error = payload.get("error")
    if expectation.infrastructure and (not isinstance(error, str) or not error):
        errors.append("an infrastructure result requires a nonempty error diagnostic")
    elif not expectation.infrastructure and error is not None:
        errors.append("a terminal test outcome must not contain an infrastructure error")
    return errors


def _validate_dear_app_smoke_payload(payload: Mapping[str, object]) -> list[str]:
    """Validate one complete dear-app/Test Engine presentation lifecycle."""
    errors: list[str] = []
    schema_version = payload.get("schema_version")
    if type(schema_version) is not int or schema_version != 1:
        errors.append(f"schema_version expected 1, got {schema_version!r}")
    if payload.get("mode") != "DearAppGraphical":
        errors.append(
            f"mode expected 'DearAppGraphical', got {payload.get('mode')!r}"
        )
    if payload.get("outcome") != "Passed":
        errors.append(f"outcome expected 'Passed', got {payload.get('outcome')!r}")

    for field_name in (
        "engine_started",
        "test_registered",
        "test_queued",
        "terminal_observed",
        "exit_requested",
        "application_shutdown",
        "engine_shutdown",
        "runtime_teardown_complete",
    ):
        if payload.get(field_name) is not True:
            errors.append(f"{field_name} expected True, got {payload.get(field_name)!r}")
    if payload.get("budget_exhausted") is not False:
        errors.append(
            f"budget_exhausted expected False, got {payload.get('budget_exhausted')!r}"
        )

    integer_fields = (
        "admitted_frames",
        "frame_budget",
        "test_engine_calls",
        "tested",
        "success",
        "in_queue",
    )
    for field_name in integer_fields:
        value = payload.get(field_name)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"{field_name} must be a nonnegative integer")

    admitted_frames = payload.get("admitted_frames")
    frame_budget = payload.get("frame_budget")
    test_engine_calls = payload.get("test_engine_calls")
    if (
        isinstance(admitted_frames, int)
        and not isinstance(admitted_frames, bool)
        and isinstance(frame_budget, int)
        and not isinstance(frame_budget, bool)
        and not (0 < admitted_frames <= frame_budget)
    ):
        errors.append("admitted_frames must be within the nonzero frame budget")
    if (
        isinstance(admitted_frames, int)
        and not isinstance(admitted_frames, bool)
        and isinstance(test_engine_calls, int)
        and not isinstance(test_engine_calls, bool)
        and test_engine_calls != admitted_frames
    ):
        errors.append("Application::test_engine must be called once per admitted frame")
    if (
        payload.get("tested") != 1
        or payload.get("success") != 1
        or payload.get("in_queue") != 0
    ):
        errors.append("graphical smoke requires exactly one successful terminal test")
    if payload.get("error") is not None:
        errors.append("a passed graphical smoke must not contain an error")
    return errors


def _highest_failure(
    failures: Sequence[tuple[GateCategory, str]],
) -> tuple[GateCategory, str]:
    return max(failures, key=lambda failure: _FAILURE_PRIORITY[failure[0]])


def _contract_failure_category(observed: GateCategory) -> GateCategory:
    if observed is GateCategory.TEST_TIMED_OUT:
        return observed
    return GateCategory.PRODUCT_FAILURE


def run_test_engine_runtime(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    child_timeout: float = 120.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Build and execute every stable Test Engine runner outcome."""
    gate = "test-engine-runtime"
    if not evidence_dir.is_absolute():
        evidence_dir = workspace_root / evidence_dir
    scenario_files = tuple(
        name
        for scenario in TEST_ENGINE_SCENARIOS
        for name in (
            f"{scenario.name}.json",
            f"{scenario.name}.stdout.log",
            f"{scenario.name}.stderr.log",
        )
    )
    graphical_files = (
        "dear-app-runtime-environment.json",
        "dear-app-package-versions.stdout.log",
        "dear-app-package-versions.stderr.log",
        "dear-app-adapter.stdout.log",
        "dear-app-adapter.stderr.log",
        "dear-app-xvfb.stdout.log",
        "dear-app-xvfb.stderr.log",
        "dear-app-display.stdout.log",
        "dear-app-display.stderr.log",
        "dear-app-openbox.stdout.log",
        "dear-app-openbox.stderr.log",
        "dear-app-window-manager.stdout.log",
        "dear-app-window-manager.stderr.log",
        "dear-app.stdout.log",
        "dear-app.stderr.log",
        "dear-app-result.json",
    )
    _prepare_evidence(
        evidence_dir=evidence_dir,
        gate=gate,
        attempt=attempt,
        owned_files=(
            "build.stdout.log",
            "build.stderr.log",
            *scenario_files,
            *graphical_files,
        ),
    )
    if rejected := _reject_excess_attempt(
        gate=gate,
        attempt=attempt,
        evidence_dir=evidence_dir,
    ):
        return rejected
    details: dict[str, object] = {"scenarios": []}
    child_environment = environment({"IMGUI_SYS_FORCE_BUILD": "1"})
    try:
        build = _run_example_build(
            workspace_root=workspace_root,
            evidence_dir=evidence_dir,
            binary="imgui_test_engine_basic",
            features="test-engine",
            timeout=build_timeout,
            child_environment=child_environment,
        )
        details["build"] = _process_json(build, evidence_dir)
        _check_stage(
            build,
            label="Test Engine example build",
            nonzero_category=GateCategory.PRODUCT_FAILURE,
        )
        binary = _example_binary(workspace_root, "imgui_test_engine_basic")
        if not binary.is_file():
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"cargo succeeded without producing {binary}",
            )

        failures: list[tuple[GateCategory, str]] = []
        scenarios = details["scenarios"]
        assert isinstance(scenarios, list)
        for expectation in TEST_ENGINE_SCENARIOS:
            child_json = evidence_dir / f"{expectation.name}.json"
            child_json.unlink(missing_ok=True)
            try:
                result = run_bounded(
                    (
                        binary,
                        "--scenario",
                        expectation.name,
                        "--json-output",
                        child_json,
                    ),
                    cwd=workspace_root,
                    env=child_environment,
                    timeout=child_timeout,
                    stdout_log=evidence_dir / f"{expectation.name}.stdout.log",
                    stderr_log=evidence_dir / f"{expectation.name}.stderr.log",
                )
            except ProcessStartError as error:
                message = f"could not start Test Engine scenario {expectation.name}: {error}"
                failures.append((GateCategory.INFRASTRUCTURE_UNAVAILABLE, message))
                scenarios.append(
                    {
                        "scenario": expectation.name,
                        "contract_match": False,
                        "category": GateCategory.INFRASTRUCTURE_UNAVAILABLE.value,
                        "errors": [message],
                    }
                )
                break

            record = _process_json(result, evidence_dir)
            record.update(
                {
                    "scenario": expectation.name,
                    "expected_returncode": expectation.returncode,
                    "expected_outcome": expectation.outcome,
                    "expected_infrastructure": expectation.infrastructure,
                    "expected_category": expectation.category.value,
                }
            )
            if result.timed_out:
                message = f"Test Engine scenario {expectation.name} exceeded {child_timeout:g}s"
                record.update(
                    {
                        "contract_match": False,
                        "category": GateCategory.HARNESS_TIMEOUT.value,
                        "errors": [message],
                    }
                )
                scenarios.append(record)
                failures.append((GateCategory.HARNESS_TIMEOUT, message))
                break
            if result.stream_errors or result.termination.errors:
                messages = [*result.stream_errors, *result.termination.errors]
                record.update(
                    {
                        "contract_match": False,
                        "category": GateCategory.INFRASTRUCTURE_UNAVAILABLE.value,
                        "errors": messages,
                    }
                )
                scenarios.append(record)
                failures.append(
                    (
                        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                        f"scenario {expectation.name} lost process evidence",
                    )
                )
                break

            try:
                payload = _read_object(child_json)
            except RuntimeContractError as error:
                record.update(
                    {
                        "contract_match": False,
                        "category": error.category.value,
                        "errors": [str(error)],
                    }
                )
                scenarios.append(record)
                failures.append((error.category, str(error)))
                continue

            errors = _validate_test_engine_payload(expectation, result, payload)
            category = _observed_test_category(payload)
            record.update(
                {
                    "contract_match": not errors,
                    "category": category.value,
                    "result": payload,
                    "errors": errors,
                }
            )
            scenarios.append(record)
            if errors:
                message = f"scenario {expectation.name}: {'; '.join(errors)}"
                failure_category = _contract_failure_category(category)
                failures.append((failure_category, message))

        if not failures:
            details["dear_app_smoke"] = _run_dear_app_graphical_smoke(
                workspace_root=workspace_root,
                evidence_dir=evidence_dir,
                binary=binary,
                child_timeout=child_timeout,
            )

        if failures:
            category, summary = _highest_failure(failures)
            result = GateResult(gate, False, category, summary, attempt, details)
        else:
            result = GateResult(
                gate,
                True,
                GateCategory.PASSED,
                "all headless outcomes and the dear-app graphical lifecycle matched",
                attempt,
                details,
            )
    except RuntimeContractError as error:
        result = GateResult(gate, False, error.category, str(error), attempt, details)
    except ProcessStartError as error:
        result = GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            str(error),
            attempt,
            details,
        )
    except OSError as error:
        result = GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"runtime evidence operation failed: {error}",
            attempt,
            details,
        )
    return _finalize(result, evidence_dir)


def _find_lavapipe_icd() -> Path:
    roots = (
        Path("/usr/share/vulkan/icd.d"),
        Path("/usr/local/share/vulkan/icd.d"),
    )
    candidates = sorted(
        candidate
        for root in roots
        if root.is_dir()
        for pattern in ("lvp_icd*.json", "*lavapipe*.json")
        for candidate in root.glob(pattern)
    )
    if not candidates:
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "Mesa Lavapipe ICD was not found",
        )
    architecture = platform.machine().lower()
    aliases = {
        "amd64": ("x86_64", "amd64"),
        "x86_64": ("x86_64", "amd64"),
        "arm64": ("aarch64", "arm64"),
        "aarch64": ("aarch64", "arm64"),
    }.get(architecture, (architecture,))
    for alias in aliases:
        for candidate in candidates:
            if alias in candidate.name.lower():
                return candidate
    if len(candidates) == 1:
        return candidates[0]
    raise RuntimeContractError(
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        f"no Lavapipe ICD matches host architecture {architecture!r}",
    )


def _require_linux_tools(
    tool_names: Sequence[str],
    *,
    platform_error: str,
) -> dict[str, Path]:
    if not sys.platform.startswith("linux"):
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            platform_error,
        )
    tools: dict[str, Path] = {}
    for name in tool_names:
        executable = shutil.which(name)
        if executable is None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"required runtime program is unavailable: {name}",
            )
        tools[name] = Path(executable)
    return tools


def _require_linux_runtime_tools() -> dict[str, Path]:
    return _require_linux_tools(
        ("Xvfb", "openbox", "xdpyinfo", "xprop", "vulkaninfo", "dpkg-query"),
        platform_error="multi-viewport-smoke requires Linux, Xvfb, and Mesa Lavapipe",
    )


def _require_linux_sdl3_glow_tools() -> dict[str, Path]:
    return _require_linux_tools(
        ("Xvfb", "openbox", "xdpyinfo", "xprop", "glxinfo", "dpkg-query"),
        platform_error=(
            "sdl3-glow-multi-viewport-smoke requires Linux, Xvfb, and Mesa llvmpipe"
        ),
    )


def _wait_for_xvfb(process: object, display: str, timeout: float = 10.0) -> None:
    match = re.fullmatch(r":([0-9]+)", display)
    if match is None:
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"invalid Xvfb display: {display!r}",
        )
    socket = Path("/tmp/.X11-unix") / f"X{match.group(1)}"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        returncode = getattr(process, "poll")()
        if returncode is not None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"Xvfb exited during startup with status {returncode}",
            )
        if socket.exists():
            return
        time.sleep(0.05)
    raise RuntimeContractError(
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        f"Xvfb did not publish {socket} within {timeout:g}s",
    )


def _wait_for_window_manager(
    *,
    process: object,
    executable: Path,
    workspace_root: Path,
    evidence_dir: Path,
    child_environment: Mapping[str, str],
    timeout: float = 10.0,
    log_stem: str = "window-manager",
) -> BoundedProcessResult:
    deadline = time.monotonic() + timeout
    last_result: BoundedProcessResult | None = None
    while time.monotonic() < deadline:
        returncode = getattr(process, "poll")()
        if returncode is not None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"openbox exited during startup with status {returncode}",
            )
        last_result = run_bounded(
            (executable, "-root", "_NET_SUPPORTING_WM_CHECK"),
            cwd=workspace_root,
            env=child_environment,
            timeout=3.0,
            stdout_log=evidence_dir / f"{log_stem}.stdout.log",
            stderr_log=evidence_dir / f"{log_stem}.stderr.log",
        )
        if last_result.stream_errors or last_result.termination.errors:
            _check_stage(
                last_result,
                label="Openbox readiness probe",
                nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            )
        output = "\n".join(
            path.read_text(encoding="utf-8", errors="replace")
            for path in last_result.log_paths
        )
        if (
            not last_result.timed_out
            and last_result.returncode == 0
            and "_NET_SUPPORTING_WM_CHECK(WINDOW)" in output
        ):
            if getattr(process, "poll")() is not None:
                raise RuntimeContractError(
                    GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                    "openbox exited after publishing its readiness property",
                )
            return last_result
        time.sleep(0.1)
    if last_result is not None and last_result.timed_out:
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "Openbox readiness probe timed out",
        )
    raise RuntimeContractError(
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        f"Openbox did not claim the window manager selection within {timeout:g}s",
    )


def _validate_viewport_lifecycle(
    payload: Mapping[str, object],
    lifecycle_fields: Sequence[str],
    *,
    schema_version: int = 3,
) -> list[str]:
    errors: list[str] = []
    actual_schema_version = payload.get("schema_version")
    if type(actual_schema_version) is not int or actual_schema_version != schema_version:
        errors.append(
            f"schema_version expected {schema_version}, got {actual_schema_version!r}"
        )
    for field_name in lifecycle_fields:
        if payload.get(field_name) is not True:
            errors.append(f"{field_name} expected True, got {payload.get(field_name)!r}")
    return errors


def _viewport_id_set(
    payload: Mapping[str, object], field_name: str, errors: list[str]
) -> set[int]:
    value = payload.get(field_name)
    if not isinstance(value, list) or not value:
        errors.append(f"{field_name} must be a nonempty u32 array")
        return set()
    if any(
        type(viewport_id) is not int or not 0 <= viewport_id <= 0xFFFF_FFFF
        for viewport_id in value
    ):
        errors.append(f"{field_name} must contain only u32 values")
        return set()
    viewport_ids = set(value)
    if len(viewport_ids) != len(value):
        errors.append(f"{field_name} must not contain duplicate viewport IDs")
    return viewport_ids


def _validate_software_vulkan_adapter(
    payload: Mapping[str, object], errors: list[str]
) -> None:
    adapter = payload.get("adapter")
    if not isinstance(adapter, dict):
        errors.append("adapter must be a JSON object")
        return
    if adapter.get("backend") != "Vulkan":
        errors.append(f"adapter backend must be Vulkan, got {adapter.get('backend')!r}")
    if adapter.get("device_type") != "Cpu":
        errors.append(
            f"adapter device_type must be Cpu, got {adapter.get('device_type')!r}"
        )
    identity = " ".join(
        str(adapter.get(field_name, "")).lower()
        for field_name in ("name", "driver", "driver_info")
    )
    if "lavapipe" not in identity and "llvmpipe" not in identity:
        errors.append("adapter identity does not report Lavapipe/llvmpipe")


def _validate_viewport_payload(payload: Mapping[str, object]) -> list[str]:
    errors = _validate_viewport_lifecycle(
        payload,
        (
            "secondary_viewport_while_held_observed",
            "merge_observed",
            "main_present_bracketed_by_test_engine",
        ),
    )
    rendered = _viewport_id_set(
        payload,
        "secondary_render_submitted_before_main_acquire_viewport_ids",
        errors,
    )
    presented = _viewport_id_set(
        payload,
        "secondary_present_submitted_before_main_acquire_viewport_ids",
        errors,
    )
    if rendered and presented and rendered.isdisjoint(presented):
        errors.append(
            "secondary render and present submissions before main acquisition "
            "must share a viewport ID"
        )
    _validate_software_vulkan_adapter(payload, errors)
    return errors


def _validate_sdl3_glow_viewport_payload(
    payload: Mapping[str, object],
) -> list[str]:
    errors = _validate_viewport_lifecycle(
        payload,
        (
            "merge_observed",
            "main_present_bracketed_by_test_engine",
            "external_texture_filters_preserved",
            "sampler_pixels_prove_isolation",
            "raw_callback_typed_state_observed",
            "reset_render_state_recovered",
            "render_state_cleared_after_callback",
            "application_gl_state_restored",
        ),
        schema_version=5,
    )
    sampler_strategy = payload.get("sampler_strategy")
    if sampler_strategy not in ("sampler_objects", "texture_parameters"):
        errors.append(
            "sampler_strategy must be sampler_objects or texture_parameters, "
            f"got {sampler_strategy!r}"
        )
    context_ids = _viewport_id_set(
        payload,
        "secondary_context_ready_before_main_present_viewport_ids",
        errors,
    )
    rendered_ids = _viewport_id_set(
        payload,
        "secondary_draw_issued_before_main_present_viewport_ids",
        errors,
    )
    swapped_ids = _viewport_id_set(
        payload,
        "secondary_swap_succeeded_before_main_present_viewport_ids",
        errors,
    )
    if context_ids and rendered_ids and swapped_ids and not (
        context_ids & rendered_ids & swapped_ids
    ):
        errors.append(
            "secondary context-ready, draw-issued, and swap-succeeded stages before "
            "main present must share a viewport ID"
        )
    renderer = payload.get("renderer")
    if not isinstance(renderer, dict):
        errors.append("renderer must be a JSON object")
        return errors
    if renderer.get("backend") != "OpenGL":
        errors.append(
            f"renderer backend must be OpenGL, got {renderer.get('backend')!r}"
        )
    for field_name in ("vendor", "name", "version"):
        if not isinstance(renderer.get(field_name), str) or not renderer[field_name]:
            errors.append(f"renderer {field_name} must be a non-empty string")
    identity = " ".join(
        str(renderer.get(field_name, "")).lower()
        for field_name in ("vendor", "name", "version")
    )
    if "lavapipe" not in identity and "llvmpipe" not in identity:
        errors.append("renderer identity does not report Mesa llvmpipe")
    return errors


def _validate_ash_vulkan_viewport_payload(
    payload: Mapping[str, object],
) -> list[str]:
    errors = _validate_viewport_lifecycle(
        payload,
        (
            "dynamic_rendering_enabled",
            "validation_layer_enabled",
            "secondary_viewport_created",
            "secondary_viewport_resized",
            "merge_observed",
            "callback_only_frame_executed",
            "raw_callback_typed_state_observed",
            "nearest_sampler_descriptor_set_observed",
            "linear_sampler_descriptor_set_observed",
            "sampler_descriptor_sets_distinct",
            "reset_render_state_recovered",
            "render_state_cleared_after_callback",
            "managed_texture_updated",
            "managed_texture_removed",
            "texture_retirement_null_fence_rejected",
            "texture_retirement_queue_drained",
            "main_present_completed",
            "renderer_shutdown_complete",
            "viewport_runtime_shutdown_complete",
            "platform_shutdown_complete",
            "gpu_idle_before_teardown",
            "vulkan_resources_dropped",
        ),
        schema_version=2,
    )
    rendered_ids = _viewport_id_set(
        payload,
        "secondary_render_submitted_viewport_ids",
        errors,
    )
    presented_ids = _viewport_id_set(
        payload,
        "secondary_present_submitted_viewport_ids",
        errors,
    )
    if rendered_ids and presented_ids and rendered_ids.isdisjoint(presented_ids):
        errors.append(
            "secondary render and present submissions must share a viewport ID"
        )
    validation_error_count = payload.get("validation_error_count")
    if type(validation_error_count) is not int or validation_error_count != 0:
        errors.append(
            "validation_error_count expected 0, "
            f"got {validation_error_count!r}"
        )
    validation_warning_count = payload.get("validation_warning_count")
    if type(validation_warning_count) is not int or validation_warning_count != 0:
        errors.append(
            "validation_warning_count expected 0, "
            f"got {validation_warning_count!r}"
        )
    retirement_count = payload.get("texture_retirement_fence_completion_count")
    if type(retirement_count) is not int or retirement_count < 2:
        errors.append(
            "texture_retirement_fence_completion_count must be at least 2, "
            f"got {retirement_count!r}"
        )
    _validate_software_vulkan_adapter(payload, errors)
    return errors


_WGPU_VIEWPORT_SMOKE = ViewportSmokeSpec(
    profile=ViewportSmokeProfile.WGPU_VULKAN,
    gate="multi-viewport-smoke",
    binary="multi_viewport_wgpu",
    features="multi-viewport,test-engine",
    package_names=(
        "xvfb",
        "openbox",
        "mesa-vulkan-drivers",
        "vulkan-tools",
        "libxkbcommon-x11-0",
    ),
    probe_tool="vulkaninfo",
    probe_arguments=("--summary",),
    probe_log_stem="adapter",
    probe_label="Lavapipe adapter probe",
    probe_identities=("lavapipe", "llvmpipe"),
    probe_identity_error="vulkaninfo did not expose a Lavapipe/llvmpipe adapter",
    probe_required_fragments=(),
    build_label="WGPU multi-viewport example build",
    child_label="WGPU multi-viewport child",
    success_summary="secondary Winit/WGPU viewport create, render, merge, and teardown passed",
    payload_validator=_validate_viewport_payload,
)

_SDL3_GLOW_VIEWPORT_SMOKE = ViewportSmokeSpec(
    profile=ViewportSmokeProfile.SDL3_GLOW,
    gate="sdl3-glow-multi-viewport-smoke",
    binary="sdl3_glow_multi_viewport",
    features="sdl3-glow-multi-viewport,test-engine",
    package_names=(
        "xvfb",
        "openbox",
        "mesa-utils",
        "libgl1-mesa-dri",
        "libxkbcommon-x11-0",
    ),
    probe_tool="glxinfo",
    probe_arguments=("-B",),
    probe_log_stem="renderer",
    probe_label="Mesa llvmpipe OpenGL probe",
    probe_identities=("llvmpipe", "lavapipe"),
    probe_identity_error="glxinfo did not expose a Mesa llvmpipe renderer",
    probe_required_fragments=(),
    build_label="SDL3/Glow multi-viewport example build",
    child_label="SDL3/Glow multi-viewport child",
    success_summary="secondary SDL3/Glow viewport create, render, merge, and teardown passed",
    payload_validator=_validate_sdl3_glow_viewport_payload,
)

_ASH_VULKAN_VIEWPORT_SMOKE = ViewportSmokeSpec(
    profile=ViewportSmokeProfile.ASH_VULKAN,
    gate="ash-vulkan-validation-smoke",
    binary="multi_viewport_ash",
    features="ash-winit-multi-viewport,ash-dynamic-rendering",
    package_names=(
        "xvfb",
        "openbox",
        "mesa-vulkan-drivers",
        "vulkan-tools",
        "vulkan-validationlayers",
        "libxkbcommon-x11-0",
    ),
    probe_tool="vulkaninfo",
    probe_arguments=("--summary",),
    probe_log_stem="adapter",
    probe_label="Lavapipe and Vulkan validation-layer probe",
    probe_identities=("lavapipe", "llvmpipe"),
    probe_identity_error="vulkaninfo did not expose a Lavapipe/llvmpipe adapter",
    probe_required_fragments=("vk_layer_khronos_validation",),
    build_label="Ash dynamic-rendering multi-viewport example build",
    child_label="Ash Vulkan validation multi-viewport child",
    success_summary=(
        "Ash dynamic-rendering secondary viewport create, resize, callbacks, "
        "present, merge, validation, and teardown passed"
    ),
    payload_validator=_validate_ash_vulkan_viewport_payload,
)


def _check_background(process: object, label: str) -> None:
    stream_errors = tuple(getattr(process, "stream_errors"))
    termination = getattr(process, "termination")
    termination_errors = () if termination is None else termination.errors
    if stream_errors or termination_errors:
        messages = (*stream_errors, *termination_errors)
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"{label} cleanup or logging failed: {'; '.join(messages)}",
        )


def _run_dear_app_graphical_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    binary: Path,
    child_timeout: float,
) -> dict[str, object]:
    """Run dear-app and Test Engine through one real software-Vulkan surface."""
    details: dict[str, object] = {}
    tools = _require_linux_runtime_tools()
    lavapipe_icd = _find_lavapipe_icd()
    display = os.environ.get("DEAR_IMGUI_XVFB_DISPLAY", ":99")
    runtime_temp_root = "/tmp" if sys.platform.startswith("linux") else None
    xdg_runtime_owner = tempfile.TemporaryDirectory(
        prefix="dear-imgui-xdg-", dir=runtime_temp_root
    )
    xdg_runtime = Path(xdg_runtime_owner.name)
    xdg_runtime.chmod(0o700)
    xvfb = None
    openbox = None
    try:
        diagnostics = {
            "display": display,
            "screen": "2560x1440x24",
            "architecture": platform.machine(),
            "runner_image": os.environ.get("ImageOS"),
            "runner_image_version": os.environ.get("ImageVersion"),
            "xdg_runtime_dir": str(xdg_runtime),
            "lavapipe_icd": str(lavapipe_icd),
            "tools": {name: str(path) for name, path in sorted(tools.items())},
        }
        atomic_write_json(
            evidence_dir / "dear-app-runtime-environment.json", diagnostics
        )
        details["environment"] = diagnostics

        child_environment = environment(
            {
                "DISPLAY": display,
                "WINIT_UNIX_BACKEND": "x11",
                "WGPU_BACKEND": "vulkan",
                "VK_DRIVER_FILES": lavapipe_icd,
                "VK_ICD_FILENAMES": lavapipe_icd,
                "LIBGL_ALWAYS_SOFTWARE": "1",
                "GALLIUM_DRIVER": "llvmpipe",
                "DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN": "1",
                "IMGUI_SYS_FORCE_BUILD": "1",
            }
        )
        child_environment["XDG_RUNTIME_DIR"] = str(xdg_runtime)

        package_versions = run_bounded(
            (
                tools["dpkg-query"],
                "--show",
                "--showformat=${Package}=${Version}\\n",
                "xvfb",
                "openbox",
                "mesa-vulkan-drivers",
                "vulkan-tools",
                "libxkbcommon-x11-0",
            ),
            cwd=workspace_root,
            timeout=15.0,
            stdout_log=evidence_dir / "dear-app-package-versions.stdout.log",
            stderr_log=evidence_dir / "dear-app-package-versions.stderr.log",
        )
        details["package_versions"] = _process_json(package_versions, evidence_dir)
        _check_stage(
            package_versions,
            label="dear-app native runtime package version probe",
            nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        )

        xvfb = managed_background(
            (
                tools["Xvfb"],
                display,
                "-screen",
                "0",
                "2560x1440x24",
                "-nolisten",
                "tcp",
                "-ac",
            ),
            cwd=workspace_root,
            env=child_environment,
            stdout_log=evidence_dir / "dear-app-xvfb.stdout.log",
            stderr_log=evidence_dir / "dear-app-xvfb.stderr.log",
        )
        try:
            with xvfb:
                _wait_for_xvfb(xvfb, display)
                display_probe = run_bounded(
                    (tools["xdpyinfo"], "-display", display),
                    cwd=workspace_root,
                    env=child_environment,
                    timeout=15.0,
                    stdout_log=evidence_dir / "dear-app-display.stdout.log",
                    stderr_log=evidence_dir / "dear-app-display.stderr.log",
                )
                details["display_probe"] = _process_json(
                    display_probe, evidence_dir
                )
                _check_stage(
                    display_probe,
                    label="dear-app Xvfb display probe",
                    nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                )

                openbox = managed_background(
                    (tools["openbox"],),
                    cwd=workspace_root,
                    env=child_environment,
                    stdout_log=evidence_dir / "dear-app-openbox.stdout.log",
                    stderr_log=evidence_dir / "dear-app-openbox.stderr.log",
                )
                try:
                    with openbox:
                        window_manager_probe = _wait_for_window_manager(
                            process=openbox,
                            executable=tools["xprop"],
                            workspace_root=workspace_root,
                            evidence_dir=evidence_dir,
                            child_environment=child_environment,
                            log_stem="dear-app-window-manager",
                        )
                        details["window_manager_probe"] = _process_json(
                            window_manager_probe, evidence_dir
                        )
                        adapter_probe = run_bounded(
                            (tools["vulkaninfo"], "--summary"),
                            cwd=workspace_root,
                            env=child_environment,
                            timeout=30.0,
                            stdout_log=evidence_dir / "dear-app-adapter.stdout.log",
                            stderr_log=evidence_dir / "dear-app-adapter.stderr.log",
                        )
                        details["adapter_probe"] = _process_json(
                            adapter_probe, evidence_dir
                        )
                        _check_stage(
                            adapter_probe,
                            label="dear-app Lavapipe adapter probe",
                            nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                        )
                        adapter_output = "\n".join(
                            path.read_text(encoding="utf-8", errors="replace").lower()
                            for path in adapter_probe.log_paths
                        )
                        if (
                            "lavapipe" not in adapter_output
                            and "llvmpipe" not in adapter_output
                        ):
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "vulkaninfo did not expose a Lavapipe/llvmpipe adapter for dear-app",
                            )
                        result_path = evidence_dir / "dear-app-result.json"
                        result_path.unlink(missing_ok=True)
                        child = run_bounded(
                            (
                                binary,
                                "--dear-app-smoke",
                                "--max-frames",
                                "256",
                                "--json-output",
                                result_path,
                            ),
                            cwd=workspace_root,
                            env=child_environment,
                            timeout=child_timeout,
                            stdout_log=evidence_dir / "dear-app.stdout.log",
                            stderr_log=evidence_dir / "dear-app.stderr.log",
                        )
                        details["child"] = _process_json(child, evidence_dir)
                        _check_stage(
                            child,
                            label="dear-app graphical Test Engine child",
                            nonzero_category=GateCategory.PRODUCT_FAILURE,
                        )
                        if xvfb.poll() is not None:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "Xvfb exited while the dear-app child ran with status "
                                f"{xvfb.returncode}",
                            )
                        if openbox.poll() is not None:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "openbox exited while the dear-app child ran with status "
                                f"{openbox.returncode}",
                            )
                finally:
                    if openbox is not None:
                        details["openbox"] = _background_json(openbox, evidence_dir)
                        _check_background(openbox, "dear-app openbox")
        finally:
            if xvfb is not None:
                details["xvfb"] = _background_json(xvfb, evidence_dir)
                _check_background(xvfb, "dear-app Xvfb")

        payload = _read_object(evidence_dir / "dear-app-result.json")
        errors = _validate_dear_app_smoke_payload(payload)
        details["result"] = payload
        if errors:
            raise RuntimeContractError(
                GateCategory.PRODUCT_FAILURE,
                "; ".join(errors),
            )
        return details
    finally:
        xdg_runtime_owner.cleanup()


def _run_viewport_smoke(
    *,
    spec: ViewportSmokeSpec,
    workspace_root: Path,
    evidence_dir: Path,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run one real secondary-window lifecycle under a software renderer."""
    gate = spec.gate
    if not evidence_dir.is_absolute():
        evidence_dir = workspace_root / evidence_dir
    _prepare_evidence(
        evidence_dir=evidence_dir,
        gate=gate,
        attempt=attempt,
        owned_files=(
            "runtime-environment.json",
            "build.stdout.log",
            "build.stderr.log",
            "package-versions.stdout.log",
            "package-versions.stderr.log",
            "xvfb.stdout.log",
            "xvfb.stderr.log",
            "display.stdout.log",
            "display.stderr.log",
            "openbox.stdout.log",
            "openbox.stderr.log",
            "window-manager.stdout.log",
            "window-manager.stderr.log",
            "adapter.stdout.log",
            "adapter.stderr.log",
            "renderer.stdout.log",
            "renderer.stderr.log",
            "viewport.stdout.log",
            "viewport.stderr.log",
            "viewport-result.json",
            "viewport-texture-parameters.stdout.log",
            "viewport-texture-parameters.stderr.log",
            "viewport-texture-parameters-result.json",
            "viewport-sampler-objects.stdout.log",
            "viewport-sampler-objects.stderr.log",
            "viewport-sampler-objects-result.json",
        ),
    )
    if rejected := _reject_excess_attempt(
        gate=gate,
        attempt=attempt,
        evidence_dir=evidence_dir,
    ):
        return rejected
    details: dict[str, object] = {}
    xvfb = None
    openbox = None
    xdg_runtime_owner = None
    try:
        if spec.profile in (
            ViewportSmokeProfile.WGPU_VULKAN,
            ViewportSmokeProfile.ASH_VULKAN,
        ):
            tools = _require_linux_runtime_tools()
            lavapipe_icd = _find_lavapipe_icd()
            route_diagnostics = {"lavapipe_icd": str(lavapipe_icd)}
            route_environment: dict[str, str | Path] = {
                "WINIT_UNIX_BACKEND": "x11",
                "VK_DRIVER_FILES": lavapipe_icd,
                "VK_ICD_FILENAMES": lavapipe_icd,
                "DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN": "1",
            }
            if spec.profile is ViewportSmokeProfile.WGPU_VULKAN:
                route_environment.update(
                    {
                        "WGPU_BACKEND": "vulkan",
                        "DEAR_IMGUI_VIEWPORT_DRAG_SMOKE": "1",
                    }
                )
            else:
                route_environment["DEAR_IMGUI_REQUIRE_VULKAN_VALIDATION"] = "1"
        elif spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            tools = _require_linux_sdl3_glow_tools()
            route_diagnostics = {"required_opengl_renderer": "Mesa llvmpipe"}
            route_environment = {
                "SDL_VIDEODRIVER": "x11",
                "DEAR_IMGUI_REQUIRE_SOFTWARE_OPENGL": "1",
            }
        else:  # pragma: no cover - specs are module-owned constants.
            raise RuntimeContractError(
                GateCategory.PRODUCT_FAILURE,
                f"unknown viewport smoke profile: {spec.profile.value}",
            )
        display = os.environ.get("DEAR_IMGUI_XVFB_DISPLAY", ":99")
        # Keep Wayland's AF_UNIX socket path below Linux's 108-byte limit.
        runtime_temp_root = "/tmp" if sys.platform.startswith("linux") else None
        xdg_runtime_owner = tempfile.TemporaryDirectory(
            prefix="dear-imgui-xdg-", dir=runtime_temp_root
        )
        xdg_runtime = Path(xdg_runtime_owner.name)
        xdg_runtime.chmod(0o700)
        diagnostics = {
            "display": display,
            "screen": "2560x1440x24",
            "architecture": platform.machine(),
            "runner_image": os.environ.get("ImageOS"),
            "runner_image_version": os.environ.get("ImageVersion"),
            "xdg_runtime_dir": str(xdg_runtime),
            "tools": {name: str(path) for name, path in sorted(tools.items())},
            **route_diagnostics,
        }
        atomic_write_json(evidence_dir / "runtime-environment.json", diagnostics)
        details["environment"] = diagnostics

        package_versions = run_bounded(
            (
                tools["dpkg-query"],
                "--show",
                "--showformat=${Package}=${Version}\\n",
                *spec.package_names,
            ),
            cwd=workspace_root,
            timeout=15.0,
            stdout_log=evidence_dir / "package-versions.stdout.log",
            stderr_log=evidence_dir / "package-versions.stderr.log",
        )
        details["package_versions"] = _process_json(package_versions, evidence_dir)
        _check_stage(
            package_versions,
            label="native runtime package version probe",
            nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        )

        child_environment = environment(
            {
                "DISPLAY": display,
                "LIBGL_ALWAYS_SOFTWARE": "1",
                "GALLIUM_DRIVER": "llvmpipe",
                "DEAR_IMGUI_VIEWPORT_SMOKE": "1",
                "DEAR_IMGUI_VIEWPORT_SMOKE_JSON": evidence_dir
                / "viewport-result.json",
                "IMGUI_SYS_FORCE_BUILD": "1",
                **route_environment,
            }
        )
        child_environment["XDG_RUNTIME_DIR"] = str(xdg_runtime)

        build = _run_example_build(
            workspace_root=workspace_root,
            evidence_dir=evidence_dir,
            binary=spec.binary,
            features=spec.features,
            timeout=build_timeout,
            child_environment=child_environment,
        )
        details["build"] = _process_json(build, evidence_dir)
        _check_stage(
            build,
            label=spec.build_label,
            nonzero_category=GateCategory.PRODUCT_FAILURE,
        )
        binary = _example_binary(workspace_root, spec.binary)
        if not binary.is_file():
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"cargo succeeded without producing {binary}",
            )
        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            sdl3_library_dirs = _sdl3_runtime_library_directories(workspace_root)
            inherited_library_path = child_environment.get("LD_LIBRARY_PATH", "")
            child_environment["LD_LIBRARY_PATH"] = os.pathsep.join(
                (
                    *(str(path) for path in sdl3_library_dirs),
                    *((inherited_library_path,) if inherited_library_path else ()),
                )
            )
            diagnostics["sdl3_library_dirs"] = [
                str(path) for path in sdl3_library_dirs
            ]
            atomic_write_json(evidence_dir / "runtime-environment.json", diagnostics)

        xvfb = managed_background(
            (
                tools["Xvfb"],
                display,
                "-screen",
                "0",
                "2560x1440x24",
                "-nolisten",
                "tcp",
                "-ac",
            ),
            cwd=workspace_root,
            env=child_environment,
            stdout_log=evidence_dir / "xvfb.stdout.log",
            stderr_log=evidence_dir / "xvfb.stderr.log",
        )
        try:
            with xvfb:
                _wait_for_xvfb(xvfb, display)
                display_info = run_bounded(
                    (tools["xdpyinfo"], "-display", display),
                    cwd=workspace_root,
                    env=child_environment,
                    timeout=15.0,
                    stdout_log=evidence_dir / "display.stdout.log",
                    stderr_log=evidence_dir / "display.stderr.log",
                )
                details["display_probe"] = _process_json(display_info, evidence_dir)
                _check_stage(
                    display_info,
                    label="Xvfb display probe",
                    nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                )

                openbox = managed_background(
                    (tools["openbox"],),
                    cwd=workspace_root,
                    env=child_environment,
                    stdout_log=evidence_dir / "openbox.stdout.log",
                    stderr_log=evidence_dir / "openbox.stderr.log",
                )
                try:
                    with openbox:
                        window_manager_probe = _wait_for_window_manager(
                            process=openbox,
                            executable=tools["xprop"],
                            workspace_root=workspace_root,
                            evidence_dir=evidence_dir,
                            child_environment=child_environment,
                        )
                        details["window_manager_probe"] = _process_json(
                            window_manager_probe, evidence_dir
                        )
                        renderer_probe = run_bounded(
                            (tools[spec.probe_tool], *spec.probe_arguments),
                            cwd=workspace_root,
                            env=child_environment,
                            timeout=30.0,
                            stdout_log=evidence_dir
                            / f"{spec.probe_log_stem}.stdout.log",
                            stderr_log=evidence_dir
                            / f"{spec.probe_log_stem}.stderr.log",
                        )
                        details["renderer_probe"] = _process_json(
                            renderer_probe, evidence_dir
                        )
                        _check_stage(
                            renderer_probe,
                            label=spec.probe_label,
                            nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                        )
                        renderer_output = "\n".join(
                            path.read_text(encoding="utf-8", errors="replace").lower()
                            for path in renderer_probe.log_paths
                        )
                        if not any(
                            identity in renderer_output
                            for identity in spec.probe_identities
                        ):
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                spec.probe_identity_error,
                            )
                        missing_probe_fragments = tuple(
                            fragment
                            for fragment in spec.probe_required_fragments
                            if fragment.lower() not in renderer_output
                        )
                        if missing_probe_fragments:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "runtime probe is missing required capabilities: "
                                + ", ".join(missing_probe_fragments),
                            )

                        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
                            profiles = (
                                (
                                    "texture-parameters",
                                    "3.2",
                                    "-GL_ARB_sampler_objects",
                                    "texture_parameters",
                                ),
                                ("sampler-objects", "3.3", None, "sampler_objects"),
                            )
                            profile_details: dict[str, object] = {}
                            for slug, version_override, extension_override, _ in profiles:
                                profile_environment = dict(child_environment)
                                profile_environment["MESA_GL_VERSION_OVERRIDE"] = version_override
                                if extension_override is None:
                                    profile_environment.pop("MESA_EXTENSION_OVERRIDE", None)
                                else:
                                    profile_environment["MESA_EXTENSION_OVERRIDE"] = (
                                        extension_override
                                    )
                                profile_result = (
                                    evidence_dir / f"viewport-{slug}-result.json"
                                )
                                profile_result.unlink(missing_ok=True)
                                profile_environment[
                                    "DEAR_IMGUI_VIEWPORT_SMOKE_JSON"
                                ] = str(profile_result)
                                child = run_bounded(
                                    (binary,),
                                    cwd=workspace_root,
                                    env=profile_environment,
                                    timeout=child_timeout,
                                    stdout_log=evidence_dir / f"viewport-{slug}.stdout.log",
                                    stderr_log=evidence_dir / f"viewport-{slug}.stderr.log",
                                )
                                profile_details[slug] = _process_json(child, evidence_dir)
                                _check_stage(
                                    child,
                                    label=f"{spec.child_label} ({slug})",
                                    nonzero_category=GateCategory.PRODUCT_FAILURE,
                                )
                            details["viewport_profiles"] = profile_details
                        else:
                            viewport_result = evidence_dir / "viewport-result.json"
                            viewport_result.unlink(missing_ok=True)
                            child = run_bounded(
                                (binary,),
                                cwd=workspace_root,
                                env=child_environment,
                                timeout=child_timeout,
                                stdout_log=evidence_dir / "viewport.stdout.log",
                                stderr_log=evidence_dir / "viewport.stderr.log",
                            )
                            details["viewport"] = _process_json(child, evidence_dir)
                            _check_stage(
                                child,
                                label=spec.child_label,
                                nonzero_category=GateCategory.PRODUCT_FAILURE,
                            )
                        if xvfb.poll() is not None:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                f"Xvfb exited while the child ran with status {xvfb.returncode}",
                            )
                        if openbox.poll() is not None:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "openbox exited while the child ran with status "
                                f"{openbox.returncode}",
                            )
                finally:
                    details["openbox"] = _background_json(openbox, evidence_dir)
                    _check_background(openbox, "openbox")
        finally:
            details["xvfb"] = _background_json(xvfb, evidence_dir)
            _check_background(xvfb, "Xvfb")

        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            results: dict[str, object] = {}
            for slug, expected_strategy in (
                ("texture-parameters", "texture_parameters"),
                ("sampler-objects", "sampler_objects"),
            ):
                payload = _read_object(evidence_dir / f"viewport-{slug}-result.json")
                errors = spec.payload_validator(payload)
                if payload.get("sampler_strategy") != expected_strategy:
                    errors.append(
                        f"sampler_strategy expected {expected_strategy!r}, "
                        f"got {payload.get('sampler_strategy')!r}"
                    )
                results[slug] = payload
                if errors:
                    raise RuntimeContractError(
                        GateCategory.PRODUCT_FAILURE,
                        f"{slug}: " + "; ".join(errors),
                    )
            details["results"] = results
        else:
            payload = _read_object(evidence_dir / "viewport-result.json")
            errors = spec.payload_validator(payload)
            details["result"] = payload
            if errors:
                raise RuntimeContractError(
                    GateCategory.PRODUCT_FAILURE,
                    "; ".join(errors),
                )
        result = GateResult(
            gate,
            True,
            GateCategory.PASSED,
            spec.success_summary,
            attempt,
            details,
        )
    except RuntimeContractError as error:
        result = GateResult(gate, False, error.category, str(error), attempt, details)
    except ProcessStartError as error:
        result = GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            str(error),
            attempt,
            details,
        )
    except OSError as error:
        result = GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"runtime environment operation failed: {error}",
            attempt,
            details,
        )
    finally:
        if xdg_runtime_owner is not None:
            xdg_runtime_owner.cleanup()
    return _finalize(result, evidence_dir)


def run_multi_viewport_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run a real Winit/WGPU secondary-window lifecycle under Lavapipe."""
    return _run_viewport_smoke(
        spec=_WGPU_VIEWPORT_SMOKE,
        workspace_root=workspace_root,
        evidence_dir=evidence_dir,
        child_timeout=child_timeout,
        build_timeout=build_timeout,
        attempt=attempt,
    )


def run_sdl3_glow_viewport_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run a real SDL3/Glow secondary-window lifecycle under Mesa llvmpipe."""
    return _run_viewport_smoke(
        spec=_SDL3_GLOW_VIEWPORT_SMOKE,
        workspace_root=workspace_root,
        evidence_dir=evidence_dir,
        child_timeout=child_timeout,
        build_timeout=build_timeout,
        attempt=attempt,
    )


def run_ash_vulkan_validation_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run Ash dynamic-rendering multi-viewport under Lavapipe validation."""
    return _run_viewport_smoke(
        spec=_ASH_VULKAN_VIEWPORT_SMOKE,
        workspace_root=workspace_root,
        evidence_dir=evidence_dir,
        child_timeout=child_timeout,
        build_timeout=build_timeout,
        attempt=attempt,
    )
