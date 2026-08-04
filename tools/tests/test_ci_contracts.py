import importlib
import io
import json
import os
import subprocess
import sys
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

CONTRACTS = importlib.import_module("run_contract")
RUNTIME = importlib.import_module("_runtime_gate")
PROCESS = importlib.import_module("_process")


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


class ExpectedFailureTests(unittest.TestCase):
    def test_accepts_nonzero_exit_with_every_required_diagnostic(self):
        result = subprocess.CompletedProcess(
            args=[], returncode=101, stdout="first contract\nsecond contract\n"
        )
        with patch.object(CONTRACTS, "run", return_value=result) as runner:
            CONTRACTS.expect_failure(
                "fixture", ("first contract", "second contract"), ("cargo", "check")
            )

        self.assertEqual(runner.call_args.kwargs["accepted_returncodes"], None)
        self.assertTrue(runner.call_args.kwargs["combine_output"])

    def test_rejects_an_unexpected_success(self):
        result = subprocess.CompletedProcess(args=[], returncode=0, stdout="")
        with (
            patch.object(CONTRACTS, "run", return_value=result),
            self.assertRaisesRegex(
                CONTRACTS.VerificationError, "fixture unexpectedly succeeded"
            ),
        ):
            CONTRACTS.expect_failure("fixture", ("contract",), ("cargo", "check"))

    def test_rejects_failure_without_the_required_diagnostic(self):
        result = subprocess.CompletedProcess(
            args=[], returncode=101, stdout="some other compiler error"
        )
        with (
            patch.object(CONTRACTS, "run", return_value=result),
            self.assertRaisesRegex(
                CONTRACTS.VerificationError, "missing diagnostic: expected contract"
            ),
        ):
            CONTRACTS.expect_failure(
                "fixture", ("expected contract",), ("cargo", "check")
            )


class ContractRunnerTests(unittest.TestCase):
    def test_parser_accepts_a_diagnostic_that_starts_with_dashes(self):
        args = CONTRACTS._build_parser().parse_args(
            (
                "expect-failure",
                "--label",
                "fixture",
                "--contains=--features bindgen",
                "--",
                "cargo",
                "check",
            )
        )

        self.assertEqual(args.required_messages, ["--features bindgen"])
        self.assertEqual(args.command, ["--", "cargo", "check"])

    def test_prebuilt_test_engine_uses_an_isolated_dual_library_fixture(self):
        observed = {}

        def inspect_run(command, **kwargs):
            fixture = Path(kwargs["env"]["IMGUI_SYS_LIB_DIR"])
            observed["command"] = tuple(command)
            observed["fixture"] = fixture
            self.assertTrue((fixture / "libdear_imgui.a").is_file())
            self.assertTrue((fixture / "dear_imgui.lib").is_file())
            return subprocess.CompletedProcess(args=command, returncode=0)

        with patch.object(CONTRACTS, "run", side_effect=inspect_run):
            CONTRACTS.check_unified_prebuilt_test_engine()

        self.assertIn("prebuilt,test-engine", observed["command"])
        self.assertFalse(observed["fixture"].exists())

    def test_clippy_expands_only_allowance_pairs_after_deny_warnings(self):
        with (
            patch.dict(
                os.environ,
                {"CLIPPY_HISTORICAL_LINTS": "-A dead_code -A clippy::needless_borrow"},
            ),
            patch.object(CONTRACTS, "run") as runner,
        ):
            CONTRACTS.run_clippy(("--", "-p", "dear-imgui-rs", "--lib"))

        self.assertEqual(
            tuple(runner.call_args.args[0]),
            (
                "cargo",
                "clippy",
                "-p",
                "dear-imgui-rs",
                "--lib",
                "--",
                "-D",
                "warnings",
                "-A",
                "dead_code",
                "-A",
                "clippy::needless_borrow",
            ),
        )

    def test_clippy_rejects_non_allowance_flags(self):
        with (
            patch.dict(os.environ, {"CLIPPY_HISTORICAL_LINTS": "-W dead_code"}),
            self.assertRaisesRegex(
                CONTRACTS.VerificationError, "may contain only -A allowances"
            ),
        ):
            CONTRACTS.run_clippy(("--", "--workspace"))

    def test_default_bindgen_check_covers_every_sys_crate_and_dependency(self):
        absent = subprocess.CompletedProcess(
            args=[], returncode=101, stdout="did not match any packages"
        )
        with patch.object(CONTRACTS, "run", return_value=absent) as runner:
            CONTRACTS.check_no_default_bindgen()

        self.assertEqual(runner.call_count, len(CONTRACTS.SYS_CRATES) * 2)

    def test_release_notes_extract_the_validated_workspace_version(self):
        with TemporaryDirectory() as temporary:
            output = Path(temporary) / "release-notes.md"
            with patch.object(CONTRACTS, "run") as runner:
                CONTRACTS.prepare_release_notes("v0.16.0-alpha.1", output)

            self.assertEqual(runner.call_count, 2)
            validate_command = tuple(runner.call_args_list[0].args[0])
            self.assertEqual(validate_command[-2:], ("--version", "0.16.0-alpha.1"))
            extract_command = tuple(runner.call_args_list[1].args[0])
            self.assertEqual(extract_command[-2:], ("--output", output))

    def test_release_notes_reject_shell_metacharacters_in_tag(self):
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(
                CONTRACTS.VerificationError, "invalid release tag"
            ):
                CONTRACTS.prepare_release_notes(
                    "v0.16.0-alpha.1;echo",
                    Path(temporary) / "notes",
                )

    def test_release_notes_require_the_workspace_release_version(self):
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(
                CONTRACTS.VerificationError, "workspace release tag"
            ):
                CONTRACTS.prepare_release_notes(
                    "v0.15.0",
                    Path(temporary) / "notes",
                )

    def test_release_identity_accepts_an_uncreated_tag_on_exact_main_candidate(self):
        candidate = "a" * 40
        head = subprocess.CompletedProcess([], 0, stdout=f"{candidate}\n")
        missing_tag = subprocess.CompletedProcess([], 1, stdout="")
        with patch.object(CONTRACTS, "run", side_effect=[head, missing_tag]) as runner:
            version = CONTRACTS.validate_release_identity(
                tag="v0.16.0-alpha.1",
                candidate_sha=candidate,
                expected_ref="refs/heads/main",
                actual_ref="refs/heads/main",
            )

        self.assertEqual(version, "0.16.0-alpha.1")
        self.assertEqual(runner.call_count, 2)

    def test_release_identity_rejects_an_existing_tag_on_another_commit(self):
        candidate = "a" * 40
        results = [
            subprocess.CompletedProcess([], 0, stdout=f"{candidate}\n"),
            subprocess.CompletedProcess([], 0, stdout=""),
            subprocess.CompletedProcess([], 0, stdout=f"{'b' * 40}\n"),
        ]
        with (
            patch.object(CONTRACTS, "run", side_effect=results),
            self.assertRaisesRegex(CONTRACTS.VerificationError, "points to"),
        ):
            CONTRACTS.validate_release_identity(
                tag="v0.16.0-alpha.1",
                candidate_sha=candidate,
                expected_ref="refs/heads/main",
                actual_ref="refs/heads/main",
            )

    def test_release_identity_rejects_non_main_dispatch(self):
        with (
            patch.object(CONTRACTS, "run") as runner,
            self.assertRaisesRegex(CONTRACTS.VerificationError, "refs/heads/main"),
        ):
            CONTRACTS.validate_release_identity(
                tag="v0.16.0-alpha.1",
                candidate_sha="a" * 40,
                expected_ref="refs/heads/main",
                actual_ref="refs/heads/topic",
            )

        runner.assert_not_called()

    def test_windows_vcpkg_uses_resolved_executable_and_publishes_environment(self):
        triplet = CONTRACTS.VcpkgTriplet.from_target(
            "x86_64-pc-windows-msvc", "md"
        )
        status = type("Status", (), {"status_bytes": 12, "update_bytes": 0})()
        with TemporaryDirectory() as temporary:
            github_environment = Path(temporary) / "github-env"
            with (
                patch.object(
                    CONTRACTS,
                    "locate_vcpkg_executable",
                    return_value=Path("C:/vcpkg/vcpkg.exe"),
                ),
                patch.object(
                    CONTRACTS,
                    "vcpkg_root_candidates",
                    return_value=("candidate",),
                ),
                patch.object(
                    CONTRACTS,
                    "resolve_vcpkg_root",
                    return_value=type("Root", (), {"path": Path("C:/vcpkg")})(),
                ),
                patch.object(CONTRACTS, "install_vcpkg_packages") as installer,
                patch.object(
                    CONTRACTS,
                    "ensure_vcpkg_status_compatibility",
                    return_value=status,
                ),
                patch.object(CONTRACTS, "append_github_assignments") as publisher,
            ):
                CONTRACTS.configure_windows_vcpkg(
                    target=triplet.rust_target,
                    crt=triplet.crt,
                    packages=("freetype", "sdl3"),
                    runner_temp=Path(temporary),
                    github_environment=github_environment,
                )

            installer.assert_called_once_with(
                ("freetype", "sdl3"),
                triplet,
                executable=Path("C:/vcpkg/vcpkg.exe"),
            )
            self.assertEqual(publisher.call_args.args[0], github_environment)

    def test_windows_mingw_import_check_writes_complete_lf_evidence(self):
        inspection = type(
            "Inspection", (), {"evidence_text": "Checking a.exe\r\nDLL Name: KERNEL32.dll\r\n"}
        )()
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "logs" / "imports.txt"
            with patch.object(
                CONTRACTS, "verify_mingw_imports", return_value=inspection
            ):
                CONTRACTS.check_windows_mingw_imports(
                    deps_directory=Path("deps"),
                    objdump=Path("objdump.exe"),
                    evidence=evidence,
                )

            self.assertEqual(
                evidence.read_bytes(),
                b"Checking a.exe\nDLL Name: KERNEL32.dll\n",
            )

    def test_windows_cli_preserves_failure_evidence_and_exit_code(self):
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "logs" / "imports.txt"
            failure = CONTRACTS.CommandError(
                ("objdump.exe", "-p", "fixture.exe"),
                9,
                "objdump diagnostic\n",
            )
            diagnostic = io.StringIO()
            with (
                patch.object(CONTRACTS, "verify_mingw_imports", side_effect=failure),
                redirect_stderr(diagnostic),
            ):
                result = CONTRACTS.main(
                    (
                        "windows-mingw-imports",
                        "--deps",
                        "deps",
                        "--objdump",
                        "objdump.exe",
                        "--evidence",
                        str(evidence),
                    )
                )

            self.assertEqual(result, 9)
            self.assertIn("objdump diagnostic", diagnostic.getvalue())
            self.assertIn("exit code 9", evidence.read_text(encoding="utf-8"))

    def test_runtime_parser_owns_stable_child_budgets(self):
        parser = CONTRACTS._build_parser()
        test_engine = parser.parse_args(("test-engine-runtime",))
        viewport = parser.parse_args(("multi-viewport-smoke",))
        sdl3_glow = parser.parse_args(("sdl3-glow-multi-viewport-smoke",))
        ash_vulkan = parser.parse_args(("ash-vulkan-validation-smoke",))

        self.assertEqual(test_engine.child_timeout, 120.0)
        self.assertEqual(viewport.child_timeout, 180.0)
        self.assertEqual(sdl3_glow.child_timeout, 180.0)
        self.assertEqual(ash_vulkan.child_timeout, 180.0)
        self.assertEqual(test_engine.build_timeout, 900.0)
        self.assertEqual(viewport.build_timeout, 900.0)
        self.assertEqual(sdl3_glow.build_timeout, 900.0)
        self.assertEqual(ash_vulkan.build_timeout, 900.0)

    def test_runtime_disposition_only_defers_first_infrastructure_failure(self):
        with TemporaryDirectory() as temporary:
            github_output = Path(temporary) / "github-output"
            first_infrastructure = RUNTIME.GateResult(
                "fixture",
                False,
                RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                "missing display",
                attempt=1,
            )
            retried_infrastructure = RUNTIME.GateResult(
                "fixture",
                False,
                RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                "missing display",
                attempt=2,
            )
            product = RUNTIME.GateResult(
                "fixture",
                False,
                RUNTIME.GateCategory.PRODUCT_FAILURE,
                "renderer failed",
                attempt=1,
            )
            with (
                patch.dict(os.environ, {"GITHUB_OUTPUT": str(github_output)}),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(
                    CONTRACTS._runtime_exit_code(
                        first_infrastructure,
                        defer_infrastructure_retry=True,
                    ),
                    0,
                )
                self.assertEqual(
                    CONTRACTS._runtime_exit_code(
                        retried_infrastructure,
                        defer_infrastructure_retry=True,
                    ),
                    1,
                )
                self.assertEqual(
                    CONTRACTS._runtime_exit_code(
                        product,
                        defer_infrastructure_retry=True,
                    ),
                    1,
                )

            outputs = github_output.read_text(encoding="utf-8")
            self.assertIn("retry_eligible=true\n", outputs)
            self.assertIn("gate_category=ProductFailure\n", outputs)

    def test_runtime_preparation_failure_is_structured_and_retryable_once(self):
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "evidence"
            github_output = Path(temporary) / "github-output"
            with patch.dict(os.environ, {"GITHUB_OUTPUT": str(github_output)}):
                exit_code = CONTRACTS.main(
                    (
                        "runtime-preparation-failure",
                        "--gate",
                        "multi-viewport-smoke",
                        "--evidence-dir",
                        str(evidence),
                        "--attempt",
                        "1",
                    )
                )

            self.assertEqual(exit_code, 0)
            result = json.loads(
                (evidence / "gate-result.json").read_text(encoding="utf-8")
            )
            self.assertEqual(result["category"], "InfrastructureUnavailable")
            self.assertEqual(result["details"], {"phase": "preparation"})
            self.assertTrue(result["retry"]["eligible"])
            self.assertIn(
                "retry_eligible=true\n",
                github_output.read_text(encoding="utf-8"),
            )


class RuntimeGateTests(unittest.TestCase):
    def test_test_engine_gate_executes_and_classifies_every_contract(self):
        expectations = {
            expectation.name: expectation
            for expectation in RUNTIME.TEST_ENGINE_SCENARIOS
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
            viewport_environment = {}

            def run_scenario(command, **kwargs):
                arguments = [os.fspath(argument) for argument in command]
                name = arguments[arguments.index("--scenario") + 1]
                result_path = Path(arguments[arguments.index("--json-output") + 1])
                expectation = expectations[name]
                tested = 1 if expectation.outcome in {"Passed", "Failed"} else 0
                succeeded = 1 if expectation.outcome == "Passed" else 0
                error = "injected infrastructure error" if expectation.infrastructure else None
                result_path.write_text(
                    json.dumps(
                        {
                            "schema_version": 1,
                            "outcome": expectation.outcome,
                            "infrastructure": expectation.infrastructure,
                            "tested": tested,
                            "success": succeeded,
                            "in_queue": 0,
                            "frames": 0,
                            "cleanup_frames": 0,
                            "error": error,
                        }
                    ),
                    encoding="utf-8",
                )
                return bounded_result(
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                    returncode=expectation.returncode,
                )

            with (
                patch.object(RUNTIME, "_run_example_build", return_value=build),
                patch.object(RUNTIME, "_example_binary", return_value=binary),
                patch.object(RUNTIME, "run_bounded", side_effect=run_scenario),
                patch.object(
                    RUNTIME,
                    "_run_dear_app_graphical_smoke",
                    return_value={"result": dear_app_smoke_payload()},
                ) as graphical_smoke,
            ):
                result = RUNTIME.run_test_engine_runtime(
                    workspace_root=root,
                    evidence_dir=evidence,
                )

            self.assertTrue(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PASSED)
            scenarios = result.details["scenarios"]
            self.assertEqual(len(scenarios), len(expectations))
            categories = {
                scenario["scenario"]: scenario["category"] for scenario in scenarios
            }
            self.assertEqual(categories["timeout"], "TestTimedOut")
            self.assertEqual(categories["ffi-failure"], "InfrastructureUnavailable")
            graphical_smoke.assert_called_once_with(
                workspace_root=root,
                evidence_dir=evidence,
                binary=binary,
                child_timeout=120.0,
            )
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

    def test_dear_app_smoke_schema_requires_wiring_terminal_and_teardown_proof(self):
        valid = dear_app_smoke_payload()
        self.assertEqual(RUNTIME._validate_dear_app_smoke_payload(valid), [])

        invalid = dear_app_smoke_payload(
            test_engine_calls=3,
            terminal_observed=False,
            tested=0,
            success=0,
            runtime_teardown_complete=False,
            error="incomplete runtime",
        )
        errors = RUNTIME._validate_dear_app_smoke_payload(invalid)
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
                patch.object(RUNTIME, "_run_example_build", return_value=build),
                patch.object(RUNTIME, "_example_binary", return_value=binary),
                patch.object(RUNTIME, "run_bounded", return_value=timed_out),
            ):
                result = RUNTIME.run_test_engine_runtime(
                    workspace_root=root,
                    evidence_dir=evidence,
                    child_timeout=0.5,
                )

            self.assertFalse(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.HARNESS_TIMEOUT)

    def test_false_pass_and_retry_policy_cannot_hide_product_failures(self):
        self.assertEqual(
            RUNTIME._contract_failure_category(RUNTIME.GateCategory.PASSED),
            RUNTIME.GateCategory.PRODUCT_FAILURE,
        )
        self.assertEqual(
            RUNTIME._contract_failure_category(
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
                RUNTIME, "_require_linux_runtime_tools", side_effect=unavailable
            ):
                result = RUNTIME.run_multi_viewport_smoke(
                    workspace_root=REPO_ROOT,
                    evidence_dir=evidence,
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

        self.assertEqual(RUNTIME._validate_viewport_payload(valid), [])
        valid["merge_observed"] = False
        self.assertRegex(
            "\n".join(RUNTIME._validate_viewport_payload(valid)),
            "merge_observed",
        )
        valid["merge_observed"] = True
        valid["secondary_present_submitted_before_main_acquire_viewport_ids"] = [99]
        self.assertRegex(
            "\n".join(RUNTIME._validate_viewport_payload(valid)),
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
            RUNTIME._validate_upstream_viewport_suite_payload(valid), []
        )

        invalid_bool = dict(valid)
        invalid_bool["in_queue"] = False
        self.assertIn(
            "in_queue must be a nonnegative integer",
            RUNTIME._validate_upstream_viewport_suite_payload(invalid_bool),
        )

        invalid_manifest = dict(valid)
        invalid_manifest["registered_tests"] = ["viewport_basic", "viewport_basic"]
        errors = RUNTIME._validate_upstream_viewport_suite_payload(invalid_manifest)
        self.assertIn(
            "registered_tests must contain unique, nonempty test names",
            errors,
        )

        incomplete = dict(valid)
        incomplete["success"] = len(registered_tests) - 1
        errors = RUNTIME._validate_upstream_viewport_suite_payload(incomplete)
        self.assertIn(
            "upstream viewport suite requires every dynamically registered test "
            "to finish successfully",
            errors,
        )

    def test_ash_vulkan_success_requires_validation_callbacks_and_teardown(self):
        valid = ash_vulkan_smoke_payload()

        self.assertEqual(RUNTIME._validate_ash_vulkan_viewport_payload(valid), [])
        valid["validation_error_count"] = 1
        self.assertRegex(
            "\n".join(RUNTIME._validate_ash_vulkan_viewport_payload(valid)),
            "validation_error_count expected 0",
        )
        valid["validation_error_count"] = 0
        valid["validation_warning_count"] = 1
        self.assertRegex(
            "\n".join(RUNTIME._validate_ash_vulkan_viewport_payload(valid)),
            "validation_warning_count expected 0",
        )
        valid["validation_warning_count"] = 0
        valid["texture_retirement_fence_completion_count"] = 1
        self.assertRegex(
            "\n".join(RUNTIME._validate_ash_vulkan_viewport_payload(valid)),
            "texture_retirement_fence_completion_count must be at least 2",
        )
        valid["texture_retirement_fence_completion_count"] = 2
        valid["secondary_present_submitted_viewport_ids"] = [99]
        self.assertRegex(
            "\n".join(RUNTIME._validate_ash_vulkan_viewport_payload(valid)),
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

        self.assertEqual(RUNTIME._validate_sdl3_glow_viewport_payload(valid), [])
        valid["secondary_swap_succeeded_before_main_present_viewport_ids"] = [99]
        self.assertRegex(
            "\n".join(RUNTIME._validate_sdl3_glow_viewport_payload(valid)),
            "must share a viewport ID",
        )

    def test_new_invocation_invalidates_owned_stale_success_evidence(self):
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "evidence"
            evidence.mkdir()
            (evidence / "gate-result.json").write_text(
                '{"success":true}', encoding="utf-8"
            )
            (evidence / "pass.json").write_text('{"outcome":"Passed"}', encoding="utf-8")

            RUNTIME._prepare_evidence(
                evidence_dir=evidence,
                gate="test-engine-runtime",
                attempt=2,
                owned_files=("pass.json",),
            )

            self.assertFalse((evidence / "gate-result.json").exists())
            self.assertFalse((evidence / "pass.json").exists())
            invocation = json.loads(
                (evidence / "gate-invocation.json").read_text(encoding="utf-8")
            )
            self.assertEqual(invocation["status"], "Running")
            self.assertEqual(invocation["attempt"], 2)

    def test_third_fresh_runner_attempt_is_rejected_without_building(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            with patch.object(RUNTIME, "_run_example_build") as builder:
                result = RUNTIME.run_test_engine_runtime(
                    workspace_root=root,
                    evidence_dir=evidence,
                    attempt=3,
                )

            builder.assert_not_called()
            self.assertFalse(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PRODUCT_FAILURE)
            self.assertFalse(result.retry_eligible)
if __name__ == "__main__":
    unittest.main()
