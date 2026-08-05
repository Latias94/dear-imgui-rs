import importlib
import json
import os
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

RUNTIME = importlib.import_module("_runtime_gate")
COMMON = importlib.import_module("_runtime_gate_common")
TEST_ENGINE = importlib.import_module("_runtime_gate_test_engine")
VIEWPORT = importlib.import_module("_runtime_gate_viewport")
PROCESS = importlib.import_module("_process")
CANDIDATE_SHA = "a" * 40


def bounded_result(
    *,
    stdout_log: Path,
    stderr_log: Path,
    returncode: int = 0,
    timed_out: bool = False,
):
    stdout_log.parent.mkdir(parents=True, exist_ok=True)
    stdout_log.write_text("stdout\n", encoding="utf-8")
    stderr_log.write_text("stderr\n", encoding="utf-8")
    return PROCESS.BoundedProcessResult(
        args=("fixture",),
        returncode=returncode,
        timed_out=timed_out,
        duration=0.25,
        stdout_log=stdout_log,
        stderr_log=stderr_log,
        termination=PROCESS.TerminationDiagnostics(
            strategy="fixture",
            attempted=timed_out,
            graceful=True if timed_out else None,
            force_kill=False,
            fallback_reason=None,
            notes=(),
            errors=(),
        ),
        stream_errors=(),
    )


def dear_app_smoke_payload(**overrides):
    payload = {
        "schema_version": 1,
        "mode": "DearAppGraphical",
        "outcome": "Passed",
        "engine_started": True,
        "test_registered": True,
        "test_queued": True,
        "admitted_frames": 4,
        "frame_budget": 256,
        "test_engine_calls": 4,
        "terminal_observed": True,
        "tested": 1,
        "success": 1,
        "in_queue": 0,
        "exit_requested": True,
        "budget_exhausted": False,
        "application_shutdown": True,
        "engine_shutdown": True,
        "runtime_teardown_complete": True,
        "error": None,
    }
    payload.update(overrides)
    return payload


def test_engine_payload(expectation, **overrides):
    if expectation.outcome == "Passed":
        terminal_tests = [
            {"category": "fixture", "name": expectation.name, "status": "Success"}
        ]
    elif expectation.outcome == "Failed":
        terminal_tests = [
            {"category": "fixture", "name": expectation.name, "status": "Error"}
        ]
    elif expectation.outcome in {"TimedOut", "Aborted"}:
        terminal_tests = [
            {
                "category": "runtime",
                "name": "long-running",
                "status": "NotRun",
            }
        ]
    else:
        terminal_tests = []

    tested = sum(
        test["status"] in {"Success", "Error", "Suspended"}
        for test in terminal_tests
    )
    succeeded = sum(test["status"] == "Success" for test in terminal_tests)
    in_queue = sum(
        test["status"] in {"Queued", "Running"} for test in terminal_tests
    )
    cleanup_frames = 1 if expectation.outcome in {"TimedOut", "Aborted"} else 0
    if expectation.outcome == "TimedOut":
        frames = 2 + cleanup_frames
    elif expectation.outcome == "Aborted":
        frames = 1 + cleanup_frames
    else:
        frames = 1 if terminal_tests else 0
    payload = {
        "schema_version": 2,
        "outcome": expectation.outcome,
        "infrastructure": expectation.infrastructure,
        "tested": tested,
        "success": succeeded,
        "in_queue": in_queue,
        "frames": frames,
        "cleanup_frames": cleanup_frames,
        "cleanup_complete": not expectation.infrastructure,
        "engine_shutdown_complete": not expectation.infrastructure,
        "terminal_tests": terminal_tests,
        "error": (
            "injected infrastructure error" if expectation.infrastructure else None
        ),
    }
    payload.update(overrides)
    return payload


def ash_vulkan_smoke_payload(**overrides):
    payload = {
        "schema_version": 2,
        "adapter": {
            "name": "llvmpipe (LLVM 20)",
            "backend": "Vulkan",
            "device_type": "Cpu",
            "driver": "Mesa",
            "driver_info": "Lavapipe",
        },
        "dynamic_rendering_enabled": True,
        "validation_layer_enabled": True,
        "secondary_viewport_created": True,
        "secondary_viewport_resized": True,
        "merge_observed": True,
        "secondary_render_submitted_viewport_ids": [17],
        "secondary_present_submitted_viewport_ids": [17],
        "callback_only_frame_executed": True,
        "raw_callback_typed_state_observed": True,
        "nearest_sampler_descriptor_set_observed": True,
        "linear_sampler_descriptor_set_observed": True,
        "sampler_descriptor_sets_distinct": True,
        "reset_render_state_recovered": True,
        "render_state_cleared_after_callback": True,
        "managed_texture_updated": True,
        "managed_texture_removed": True,
        "texture_retirement_null_fence_rejected": True,
        "texture_retirement_fence_completion_count": 2,
        "texture_retirement_queue_drained": True,
        "main_present_completed": True,
        "renderer_shutdown_complete": True,
        "viewport_runtime_shutdown_complete": True,
        "platform_shutdown_complete": True,
        "gpu_idle_before_teardown": True,
        "vulkan_resources_dropped": True,
        "validation_warning_count": 0,
        "validation_error_count": 0,
    }
    payload.update(overrides)
    return payload


class RuntimeGateTests(unittest.TestCase):
    def test_test_engine_gate_executes_and_classifies_every_contract(self):
        expectations = {
            expectation.name: expectation
            for expectation in TEST_ENGINE.TEST_ENGINE_SCENARIOS
        }
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            binary = root / "imgui_test_engine_basic"
            binary.touch()
            build = bounded_result(
                stdout_log=evidence / "build.stdout.log",
                stderr_log=evidence / "build.stderr.log",
            )

            def run_scenario(command, **kwargs):
                arguments = [os.fspath(argument) for argument in command]
                name = arguments[arguments.index("--scenario") + 1]
                result_path = Path(arguments[arguments.index("--json-output") + 1])
                expectation = expectations[name]
                result_path.write_text(
                    json.dumps(test_engine_payload(expectation)),
                    encoding="utf-8",
                )
                return bounded_result(
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                    returncode=expectation.returncode,
                )

            with (
                patch.object(TEST_ENGINE, "_run_example_build", return_value=build),
                patch.object(TEST_ENGINE, "_example_binary", return_value=binary),
                patch.object(TEST_ENGINE, "run_bounded", side_effect=run_scenario),
                patch.object(
                    TEST_ENGINE,
                    "_run_dear_app_graphical_smoke",
                    return_value={"result": dear_app_smoke_payload()},
                ) as graphical_smoke,
            ):
                result = RUNTIME.run_test_engine_runtime(
                    workspace_root=root,
                    evidence_dir=evidence,
                    candidate_sha=CANDIDATE_SHA,
                )

            self.assertTrue(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PASSED)
            categories = {
                scenario["scenario"]: scenario["category"]
                for scenario in result.details["scenarios"]
            }
            self.assertEqual(set(categories), set(expectations))
            self.assertEqual(categories["timeout"], "TestTimedOut")
            self.assertEqual(categories["ffi-failure"], "InfrastructureUnavailable")
            graphical_smoke.assert_called_once()
            self.assertEqual(
                result.details["dear_app_smoke"]["result"]["outcome"],
                "Passed",
            )
            aggregate = json.loads(
                (evidence / "gate-result.json").read_text(encoding="utf-8")
            )
            self.assertEqual(aggregate["category"], "Passed")
            self.assertIn("pass.stdout.log", aggregate["evidence"])
            invocation = json.loads(
                (evidence / "gate-invocation.json").read_text(encoding="utf-8")
            )
            self.assertEqual(invocation["status"], "Complete")
            self.assertEqual(invocation["candidate_sha"], CANDIDATE_SHA)

    def test_timeout_and_abort_require_exact_terminal_cleanup_evidence(self):
        expectations = {
            expectation.name: expectation
            for expectation in TEST_ENGINE.TEST_ENGINE_SCENARIOS
        }
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            for scenario in ("timeout", "abort"):
                expectation = expectations[scenario]
                result = bounded_result(
                    stdout_log=evidence / f"{scenario}.stdout.log",
                    stderr_log=evidence / f"{scenario}.stderr.log",
                    returncode=expectation.returncode,
                )
                valid_payload = test_engine_payload(expectation)
                self.assertEqual(
                    TEST_ENGINE._validate_test_engine_payload(
                        expectation, result, valid_payload
                    ),
                    [],
                )

                invalid_payloads = (
                    {**valid_payload, "terminal_tests": []},
                    {
                        **valid_payload,
                        "terminal_tests": [
                            {
                                "category": "runtime",
                                "name": "different-test",
                                "status": "NotRun",
                            }
                        ],
                    },
                    {
                        **valid_payload,
                        "terminal_tests": [
                            {
                                "category": "runtime",
                                "name": "long-running",
                                "status": "Running",
                            }
                        ],
                    },
                    {**valid_payload, "in_queue": 1},
                    {**valid_payload, "frames": valid_payload["frames"] + 1},
                    {**valid_payload, "cleanup_complete": False},
                    {**valid_payload, "engine_shutdown_complete": False},
                    {**valid_payload, "schema_version": 1},
                )
                for payload in invalid_payloads:
                    with self.subTest(scenario=scenario, payload=payload):
                        self.assertTrue(
                            TEST_ENGINE._validate_test_engine_payload(
                                expectation, result, payload
                            )
                        )

    def test_dear_app_smoke_schema_requires_wiring_terminal_and_teardown_proof(self):
        valid = dear_app_smoke_payload()
        self.assertEqual(TEST_ENGINE._validate_dear_app_smoke_payload(valid), [])

        invalid = dear_app_smoke_payload(
            test_engine_calls=3,
            terminal_observed=False,
            tested=0,
            success=0,
            runtime_teardown_complete=False,
            error="incomplete runtime",
        )
        errors = TEST_ENGINE._validate_dear_app_smoke_payload(invalid)
        self.assertIn("terminal_observed expected True, got False", errors)
        self.assertIn("runtime_teardown_complete expected True, got False", errors)
        self.assertIn(
            "Application::test_engine must be called once per admitted frame", errors
        )
        self.assertIn(
            "graphical smoke requires exactly one successful terminal test", errors
        )
        self.assertIn("a passed graphical smoke must not contain an error", errors)

    def test_child_deadline_is_harness_timeout_not_test_timeout(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            binary = root / "imgui_test_engine_basic"
            binary.touch()
            build = bounded_result(
                stdout_log=evidence / "build.stdout.log",
                stderr_log=evidence / "build.stderr.log",
            )
            timed_out = bounded_result(
                stdout_log=evidence / "pass.stdout.log",
                stderr_log=evidence / "pass.stderr.log",
                returncode=-9,
                timed_out=True,
            )
            with (
                patch.object(TEST_ENGINE, "_run_example_build", return_value=build),
                patch.object(TEST_ENGINE, "_example_binary", return_value=binary),
                patch.object(TEST_ENGINE, "run_bounded", return_value=timed_out),
            ):
                result = RUNTIME.run_test_engine_runtime(
                    workspace_root=root,
                    evidence_dir=evidence,
                    candidate_sha=CANDIDATE_SHA,
                    child_timeout=0.5,
                )

            self.assertFalse(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.HARNESS_TIMEOUT)

    def test_false_pass_and_retry_policy_cannot_hide_product_failures(self):
        self.assertEqual(
            TEST_ENGINE._contract_failure_category(RUNTIME.GateCategory.PASSED),
            RUNTIME.GateCategory.PRODUCT_FAILURE,
        )
        self.assertEqual(
            TEST_ENGINE._contract_failure_category(
                RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE
            ),
            RUNTIME.GateCategory.PRODUCT_FAILURE,
        )
        first_infrastructure_failure = RUNTIME.GateResult(
            "fixture",
            False,
            RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "missing display",
            attempt=1,
        ).to_json()
        retried_infrastructure_failure = RUNTIME.GateResult(
            "fixture",
            False,
            RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "missing display",
            attempt=2,
        ).to_json()
        product_failure = RUNTIME.GateResult(
            "fixture",
            False,
            RUNTIME.GateCategory.PRODUCT_FAILURE,
            "renderer failed",
            attempt=1,
        ).to_json()

        self.assertTrue(first_infrastructure_failure["retry"]["eligible"])
        self.assertFalse(retried_infrastructure_failure["retry"]["eligible"])
        self.assertFalse(product_failure["retry"]["eligible"])

    def test_missing_viewport_infrastructure_is_a_retained_no_go(self):
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "evidence"
            unavailable = RUNTIME.RuntimeContractError(
                RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                "required runtime program is unavailable: Xvfb",
            )
            with patch.object(
                VIEWPORT, "_require_linux_runtime_tools", side_effect=unavailable
            ):
                result = RUNTIME.run_multi_viewport_smoke(
                    workspace_root=REPO_ROOT,
                    evidence_dir=evidence,
                    candidate_sha=CANDIDATE_SHA,
                )

            self.assertFalse(result.success)
            self.assertEqual(
                result.category, RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE
            )
            aggregate = json.loads(
                (evidence / "gate-result.json").read_text(encoding="utf-8")
            )
            self.assertEqual(aggregate["category"], "InfrastructureUnavailable")

    def test_viewport_success_requires_lavapipe_and_full_lifecycle(self):
        valid = {
            "schema_version": 3,
            "adapter": {
                "name": "llvmpipe (LLVM 20)",
                "backend": "Vulkan",
                "device_type": "Cpu",
                "driver": "llvmpipe",
                "driver_info": "Mesa 25",
            },
            "secondary_viewport_while_held_observed": True,
            "merge_observed": True,
            "secondary_render_submitted_before_main_acquire_viewport_ids": [42, 43],
            "secondary_present_submitted_before_main_acquire_viewport_ids": [43],
            "main_present_bracketed_by_test_engine": True,
        }

        self.assertEqual(VIEWPORT._validate_viewport_payload(valid), [])
        valid["merge_observed"] = False
        self.assertRegex(
            "\n".join(VIEWPORT._validate_viewport_payload(valid)),
            "merge_observed",
        )
        valid["merge_observed"] = True
        valid["secondary_present_submitted_before_main_acquire_viewport_ids"] = [99]
        self.assertRegex(
            "\n".join(VIEWPORT._validate_viewport_payload(valid)),
            "must share a viewport ID",
        )

    def test_upstream_viewport_suite_requires_every_registered_test_to_pass(self):
        registered_tests = ["viewport_basic", "viewport_focus"]
        valid = {
            "schema_version": 1,
            "suite": "upstream-viewports",
            "category": "viewport",
            "platform_backend": "Winit",
            "renderer_backend": "WGPU",
            "real_platform_backend": True,
            "runtime_teardown_complete": True,
            "registered_count": len(registered_tests),
            "registered_tests": registered_tests,
            "tested": len(registered_tests),
            "success": len(registered_tests),
            "in_queue": 0,
            "adapter": {
                "name": "llvmpipe (LLVM 20)",
                "backend": "Vulkan",
                "device_type": "Cpu",
                "driver": "llvmpipe",
                "driver_info": "Mesa 25",
            },
        }
        self.assertEqual(
            VIEWPORT._validate_upstream_viewport_suite_payload(valid), []
        )

        invalid_bool = dict(valid)
        invalid_bool["in_queue"] = False
        self.assertIn(
            "in_queue must be a nonnegative integer",
            VIEWPORT._validate_upstream_viewport_suite_payload(invalid_bool),
        )

        invalid_manifest = dict(valid)
        invalid_manifest["registered_tests"] = ["viewport_basic", "viewport_basic"]
        errors = VIEWPORT._validate_upstream_viewport_suite_payload(invalid_manifest)
        self.assertIn(
            "registered_tests must contain unique, nonempty test names",
            errors,
        )

        incomplete = dict(valid)
        incomplete["success"] = len(registered_tests) - 1
        errors = VIEWPORT._validate_upstream_viewport_suite_payload(incomplete)
        self.assertIn(
            "upstream viewport suite requires every dynamically registered test "
            "to finish successfully",
            errors,
        )

    def test_ash_vulkan_success_requires_validation_callbacks_and_teardown(self):
        valid = ash_vulkan_smoke_payload()

        self.assertEqual(VIEWPORT._validate_ash_vulkan_viewport_payload(valid), [])
        valid["validation_error_count"] = 1
        self.assertRegex(
            "\n".join(VIEWPORT._validate_ash_vulkan_viewport_payload(valid)),
            "validation_error_count expected 0",
        )
        valid["validation_error_count"] = 0
        valid["validation_warning_count"] = 1
        self.assertRegex(
            "\n".join(VIEWPORT._validate_ash_vulkan_viewport_payload(valid)),
            "validation_warning_count expected 0",
        )
        valid["validation_warning_count"] = 0
        valid["texture_retirement_fence_completion_count"] = 1
        self.assertRegex(
            "\n".join(VIEWPORT._validate_ash_vulkan_viewport_payload(valid)),
            "texture_retirement_fence_completion_count must be at least 2",
        )
        valid["texture_retirement_fence_completion_count"] = 2
        valid["secondary_present_submitted_viewport_ids"] = [99]
        self.assertRegex(
            "\n".join(VIEWPORT._validate_ash_vulkan_viewport_payload(valid)),
            "must share a viewport ID",
        )

    def test_sdl3_glow_success_requires_llvmpipe_rendering_and_full_lifecycle(self):
        valid = {
            "schema_version": 5,
            "renderer": {
                "backend": "OpenGL",
                "vendor": "Mesa",
                "name": "llvmpipe (LLVM 20)",
                "version": "4.5 Mesa 25",
            },
            "merge_observed": True,
            "secondary_context_ready_before_main_present_viewport_ids": [7, 8],
            "secondary_draw_issued_before_main_present_viewport_ids": [7, 9],
            "secondary_swap_succeeded_before_main_present_viewport_ids": [7, 10],
            "main_present_bracketed_by_test_engine": True,
            "external_texture_filters_preserved": True,
            "sampler_pixels_prove_isolation": True,
            "raw_callback_typed_state_observed": True,
            "reset_render_state_recovered": True,
            "render_state_cleared_after_callback": True,
            "application_gl_state_restored": True,
            "sampler_strategy": "sampler_objects",
        }

        self.assertEqual(VIEWPORT._validate_sdl3_glow_viewport_payload(valid), [])
        valid["secondary_swap_succeeded_before_main_present_viewport_ids"] = [99]
        self.assertRegex(
            "\n".join(VIEWPORT._validate_sdl3_glow_viewport_payload(valid)),
            "must share a viewport ID",
        )

    def test_new_invocation_invalidates_owned_stale_success_evidence(self):
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "evidence"
            evidence.mkdir()
            (evidence / "gate-result.json").write_text(
                '{"success":true}', encoding="utf-8"
            )
            (evidence / "pass.json").write_text(
                '{"outcome":"Passed"}', encoding="utf-8"
            )

            COMMON._prepare_evidence(
                evidence_dir=evidence,
                gate="test-engine-runtime",
                attempt=2,
                candidate_sha=CANDIDATE_SHA,
                owned_files=("pass.json",),
            )

            self.assertFalse((evidence / "gate-result.json").exists())
            self.assertFalse((evidence / "pass.json").exists())
            invocation = json.loads(
                (evidence / "gate-invocation.json").read_text(encoding="utf-8")
            )
            self.assertEqual(invocation["status"], "Running")
            self.assertEqual(invocation["attempt"], 2)
            self.assertEqual(invocation["candidate_sha"], CANDIDATE_SHA)

    def test_third_fresh_runner_attempt_is_rejected_without_building(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            with patch.object(TEST_ENGINE, "_run_example_build") as builder:
                result = RUNTIME.run_test_engine_runtime(
                    workspace_root=root,
                    evidence_dir=evidence,
                    candidate_sha=CANDIDATE_SHA,
                    attempt=3,
                )

            builder.assert_not_called()
            self.assertFalse(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PRODUCT_FAILURE)
            self.assertFalse(result.retry_eligible)


if __name__ == "__main__":
    unittest.main()
