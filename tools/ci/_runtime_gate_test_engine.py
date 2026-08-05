"""Dear ImGui Test Engine runtime contracts."""

from __future__ import annotations

import os
import platform
import sys
import tempfile
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

from _process import ProcessStartError, environment, managed_background, run_bounded
from _runtime_gate_common import (
    GateCategory,
    GateResult,
    RuntimeContractError,
    _background_json,
    _check_stage,
    _example_binary,
    _finalize,
    _highest_failure,
    _prepare_evidence,
    _process_json,
    _read_object,
    _reject_excess_attempt,
    _run_example_build,
)
from _runtime_gate_display import (
    _check_background,
    _find_lavapipe_icd,
    _require_linux_runtime_tools,
    _wait_for_window_manager,
    _wait_for_xvfb,
)
from _verification import write_json


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
    ScenarioExpectation(
        "native-defaults",
        0,
        "Passed",
        False,
        GateCategory.PASSED,
    ),
    ScenarioExpectation(
        "upstream-docking",
        0,
        "Passed",
        False,
        GateCategory.PASSED,
    ),
)


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
        or payload.get("in_queue") != 0
    ):
        errors.append(
            "Passed requires a nonzero tested count, every test successful, "
            "and an empty queue"
        )
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


def _contract_failure_category(observed: GateCategory) -> GateCategory:
    if observed is GateCategory.TEST_TIMED_OUT:
        return observed
    return GateCategory.PRODUCT_FAILURE


def run_test_engine_runtime(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    candidate_sha: str,
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
        candidate_sha=candidate_sha,
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
        candidate_sha=candidate_sha,
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
    return _finalize(result, evidence_dir, candidate_sha)


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
        write_json(
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
