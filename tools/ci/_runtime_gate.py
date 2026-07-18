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
from collections.abc import Mapping, Sequence
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


class GateCategory(str, Enum):
    """Stable classifications consumed by release aggregation."""

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


def write_json_atomic(path: Path, payload: Mapping[str, object]) -> None:
    """Write JSON without exposing a partially written release result."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            prefix=f".{path.name}.",
            suffix=".tmp",
            dir=path.parent,
            delete=False,
        ) as temporary:
            temporary_name = temporary.name
            json.dump(payload, temporary, indent=2, sort_keys=True)
            temporary.write("\n")
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, path)
        temporary_name = None
    finally:
        if temporary_name is not None:
            Path(temporary_name).unlink(missing_ok=True)


def _evidence_files(evidence_dir: Path) -> tuple[str, ...]:
    return tuple(
        path.relative_to(evidence_dir).as_posix()
        for path in sorted(evidence_dir.rglob("*"))
        if path.is_file() and path.name != "gate-result.json"
    )


def _finalize(result: GateResult, evidence_dir: Path) -> GateResult:
    write_json_atomic(
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
    write_json_atomic(evidence_dir / "gate-result.json", result.to_json())
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
    write_json_atomic(
        evidence_dir / "gate-invocation.json",
        {
            "schema_version": 1,
            "status": "Running",
            "gate": gate,
            "attempt": attempt,
            "process_id": os.getpid(),
        },
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
    _prepare_evidence(
        evidence_dir=evidence_dir,
        gate=gate,
        attempt=attempt,
        owned_files=("build.stdout.log", "build.stderr.log", *scenario_files),
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

        if failures:
            category, summary = _highest_failure(failures)
            result = GateResult(gate, False, category, summary, attempt, details)
        else:
            result = GateResult(
                gate,
                True,
                GateCategory.PASSED,
                "all Test Engine runtime outcome contracts matched",
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


def _require_linux_runtime_tools() -> dict[str, Path]:
    if not sys.platform.startswith("linux"):
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "multi-viewport-smoke requires Linux, Xvfb, and Mesa Lavapipe",
        )
    tools: dict[str, Path] = {}
    for name in ("Xvfb", "openbox", "xdpyinfo", "xprop", "vulkaninfo", "dpkg-query"):
        executable = shutil.which(name)
        if executable is None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"required runtime program is unavailable: {name}",
            )
        tools[name] = Path(executable)
    return tools


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
            stdout_log=evidence_dir / "window-manager.stdout.log",
            stderr_log=evidence_dir / "window-manager.stderr.log",
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


def _validate_viewport_payload(payload: Mapping[str, object]) -> list[str]:
    errors: list[str] = []
    schema_version = payload.get("schema_version")
    if type(schema_version) is not int or schema_version != 1:
        errors.append(f"schema_version expected 1, got {schema_version!r}")
    if payload.get("outcome") != "Passed":
        errors.append(f"outcome expected 'Passed', got {payload.get('outcome')!r}")
    for field_name in (
        "secondary_viewport_observed",
        "merge_observed",
        "teardown_complete",
    ):
        if payload.get(field_name) is not True:
            errors.append(f"{field_name} expected True, got {payload.get(field_name)!r}")
    adapter = payload.get("adapter")
    if not isinstance(adapter, dict):
        errors.append("adapter must be a JSON object")
        return errors
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
    return errors


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


def run_multi_viewport_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run a real Winit/WGPU secondary-window lifecycle under Lavapipe."""
    gate = "multi-viewport-smoke"
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
            "viewport.stdout.log",
            "viewport.stderr.log",
            "viewport-result.json",
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
    try:
        tools = _require_linux_runtime_tools()
        lavapipe_icd = _find_lavapipe_icd()
        display = os.environ.get("DEAR_IMGUI_XVFB_DISPLAY", ":99")
        diagnostics = {
            "display": display,
            "screen": "2560x1440x24",
            "architecture": platform.machine(),
            "runner_image": os.environ.get("ImageOS"),
            "runner_image_version": os.environ.get("ImageVersion"),
            "lavapipe_icd": str(lavapipe_icd),
            "tools": {name: str(path) for name, path in sorted(tools.items())},
        }
        write_json_atomic(evidence_dir / "runtime-environment.json", diagnostics)
        details["environment"] = diagnostics

        package_versions = run_bounded(
            (
                tools["dpkg-query"],
                "--show",
                "--showformat=${Package}=${Version}\\n",
                "xvfb",
                "openbox",
                "mesa-vulkan-drivers",
                "vulkan-tools",
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
                "WINIT_UNIX_BACKEND": "x11",
                "WGPU_BACKEND": "vulkan",
                "VK_DRIVER_FILES": lavapipe_icd,
                "VK_ICD_FILENAMES": lavapipe_icd,
                "LIBGL_ALWAYS_SOFTWARE": "1",
                "GALLIUM_DRIVER": "llvmpipe",
                "DEAR_IMGUI_VIEWPORT_SMOKE": "1",
                "DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN": "1",
                "DEAR_IMGUI_VIEWPORT_SMOKE_JSON": evidence_dir
                / "viewport-result.json",
                "IMGUI_SYS_FORCE_BUILD": "1",
            }
        )
        xdg_runtime = evidence_dir / "xdg-runtime"
        xdg_runtime.mkdir(exist_ok=True)
        xdg_runtime.chmod(0o700)
        child_environment["XDG_RUNTIME_DIR"] = str(xdg_runtime)

        build = _run_example_build(
            workspace_root=workspace_root,
            evidence_dir=evidence_dir,
            binary="multi_viewport_wgpu",
            features="multi-viewport,test-engine",
            timeout=build_timeout,
            child_environment=child_environment,
        )
        details["build"] = _process_json(build, evidence_dir)
        _check_stage(
            build,
            label="WGPU multi-viewport example build",
            nonzero_category=GateCategory.PRODUCT_FAILURE,
        )
        binary = _example_binary(workspace_root, "multi_viewport_wgpu")
        if not binary.is_file():
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"cargo succeeded without producing {binary}",
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
                        adapter_probe = run_bounded(
                            (tools["vulkaninfo"], "--summary"),
                            cwd=workspace_root,
                            env=child_environment,
                            timeout=30.0,
                            stdout_log=evidence_dir / "adapter.stdout.log",
                            stderr_log=evidence_dir / "adapter.stderr.log",
                        )
                        details["adapter_probe"] = _process_json(
                            adapter_probe, evidence_dir
                        )
                        _check_stage(
                            adapter_probe,
                            label="Lavapipe adapter probe",
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
                                "vulkaninfo did not expose a Lavapipe/llvmpipe adapter",
                            )

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
                            label="WGPU multi-viewport child",
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

        payload = _read_object(evidence_dir / "viewport-result.json")
        errors = _validate_viewport_payload(payload)
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
            "secondary Winit/WGPU viewport create, render, merge, and teardown passed",
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
    return _finalize(result, evidence_dir)
