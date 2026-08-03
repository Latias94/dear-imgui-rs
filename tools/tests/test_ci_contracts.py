import importlib
import io
import json
import os
import subprocess
import sys
import tomllib
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from tools.tests.workflow_semantics import (
    load_workflow,
    named_step,
    require_mapping,
    workflow_jobs,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

CONTRACTS = importlib.import_module("run_contract")
RUNTIME = importlib.import_module("_runtime_gate")
PROCESS = importlib.import_module("_process")
BUILD_GLSLANG = importlib.import_module("build_glslang")


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


class FakeBackground:
    def __init__(self, command, *, stdout_log: Path, stderr_log: Path):
        self.args = tuple(os.fspath(argument) for argument in command)
        self.stdout_log = Path(stdout_log)
        self.stderr_log = Path(stderr_log)
        self.stdout_log.parent.mkdir(parents=True, exist_ok=True)
        self.stdout_log.write_text("background stdout\n", encoding="utf-8")
        self.stderr_log.write_text("background stderr\n", encoding="utf-8")
        self.returncode = None
        self.stream_errors = ()
        self.termination = None

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.returncode = -15
        self.termination = PROCESS.TerminationDiagnostics(
            strategy="fixture",
            attempted=True,
            graceful=True,
            force_kill=False,
            fallback_reason=None,
            notes=(),
            errors=(),
        )

    def poll(self):
        return self.returncode


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
                tested = expectation.expected_test_count
                if tested is None:
                    tested = 1 if name in {"pass", "failure"} else 0
                succeeded = (
                    expectation.expected_test_count
                    if expectation.expected_test_count is not None
                    else 1 if name == "pass" else 0
                )
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

    def test_dear_app_graphical_smoke_uses_xvfb_openbox_and_lavapipe(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            evidence.mkdir()
            binary = root / "imgui_test_engine_basic"
            binary.touch()
            icd = root / "lvp_icd.x86_64.json"
            icd.write_text("{}", encoding="utf-8")
            tools = {
                name: root / name
                for name in (
                    "Xvfb",
                    "openbox",
                    "xdpyinfo",
                    "xprop",
                    "vulkaninfo",
                    "dpkg-query",
                )
            }
            child_environment = {}
            child_arguments = []

            def background(command, **kwargs):
                return FakeBackground(
                    command,
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )

            def stage(command, **kwargs):
                result = bounded_result(
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )
                if result.stdout_log.name == "dear-app-adapter.stdout.log":
                    result.stdout_log.write_text(
                        "deviceName = llvmpipe (LLVM 20)\n", encoding="utf-8"
                    )
                elif result.stdout_log.name == "dear-app-window-manager.stdout.log":
                    result.stdout_log.write_text(
                        "_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x200001\n",
                        encoding="utf-8",
                    )
                elif result.stdout_log.name == "dear-app.stdout.log":
                    child_environment.update(kwargs["env"])
                    child_arguments.extend(os.fspath(argument) for argument in command)
                    (evidence / "dear-app-result.json").write_text(
                        json.dumps(dear_app_smoke_payload()), encoding="utf-8"
                    )
                return result

            with (
                patch.object(RUNTIME, "_require_linux_runtime_tools", return_value=tools),
                patch.object(RUNTIME, "_find_lavapipe_icd", return_value=icd),
                patch.object(RUNTIME, "managed_background", side_effect=background),
                patch.object(RUNTIME, "_wait_for_xvfb"),
                patch.object(RUNTIME, "run_bounded", side_effect=stage),
                patch.object(RUNTIME.time, "sleep"),
            ):
                details = RUNTIME._run_dear_app_graphical_smoke(
                    workspace_root=root,
                    evidence_dir=evidence,
                    binary=binary,
                    child_timeout=120.0,
                )

            self.assertEqual(details["result"]["outcome"], "Passed")
            self.assertTrue(details["xvfb"]["termination"]["attempted"])
            self.assertTrue(details["openbox"]["termination"]["attempted"])
            self.assertIn("--dear-app-smoke", child_arguments)
            self.assertEqual(child_environment["WGPU_BACKEND"], "vulkan")
            self.assertEqual(child_environment["WINIT_UNIX_BACKEND"], "x11")
            self.assertEqual(child_environment["VK_DRIVER_FILES"], str(icd))
            xdg_runtime = Path(details["environment"]["xdg_runtime_dir"])
            self.assertFalse(xdg_runtime.exists())

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

    def test_viewport_tool_profiles_preserve_platform_and_missing_tool_contract(self):
        profiles = (
            (
                RUNTIME._require_linux_runtime_tools,
                "multi-viewport-smoke requires Linux, Xvfb, and Mesa Lavapipe",
                ("Xvfb", "openbox", "xdpyinfo", "xprop", "vulkaninfo", "dpkg-query"),
            ),
            (
                RUNTIME._require_linux_sdl3_glow_tools,
                "sdl3-glow-multi-viewport-smoke requires Linux, Xvfb, and Mesa llvmpipe",
                ("Xvfb", "openbox", "xdpyinfo", "xprop", "glxinfo", "dpkg-query"),
            ),
        )

        for require_tools, platform_error, tool_names in profiles:
            with self.subTest(profile=require_tools.__name__):
                with patch.object(RUNTIME.sys, "platform", "win32"):
                    with self.assertRaises(RUNTIME.RuntimeContractError) as raised:
                        require_tools()
                self.assertEqual(str(raised.exception), platform_error)
                self.assertEqual(
                    raised.exception.category,
                    RUNTIME.GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                )

                checked: list[str] = []

                def which(name):
                    checked.append(name)
                    return None if name == tool_names[4] else f"/usr/bin/{name}"

                with (
                    patch.object(RUNTIME.sys, "platform", "linux"),
                    patch.object(RUNTIME.shutil, "which", side_effect=which),
                ):
                    with self.assertRaises(RUNTIME.RuntimeContractError) as raised:
                        require_tools()
                self.assertEqual(checked, list(tool_names[:5]))
                self.assertEqual(
                    str(raised.exception),
                    f"required runtime program is unavailable: {tool_names[4]}",
                )

                with (
                    patch.object(RUNTIME.sys, "platform", "linux"),
                    patch.object(
                        RUNTIME.shutil,
                        "which",
                        side_effect=lambda name: f"/usr/bin/{name}",
                    ),
                ):
                    tools = require_tools()
                self.assertEqual(list(tools), list(tool_names))
                self.assertEqual(
                    tools,
                    {name: Path(f"/usr/bin/{name}") for name in tool_names},
                )

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

    def test_upstream_viewport_suite_requires_the_exact_official_manifest(self):
        valid = {
            "schema_version": 1,
            "suite": "upstream-viewports",
            "category": "viewport",
            "platform_backend": "Winit",
            "renderer_backend": "WGPU",
            "real_platform_backend": True,
            "runtime_teardown_complete": True,
            "registered_count": len(RUNTIME.UPSTREAM_VIEWPORT_TESTS),
            "registered_tests": list(RUNTIME.UPSTREAM_VIEWPORT_TESTS),
            "tested": len(RUNTIME.UPSTREAM_VIEWPORT_TESTS),
            "success": len(RUNTIME.UPSTREAM_VIEWPORT_TESTS),
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

        valid["registered_tests"] = valid["registered_tests"][:-1]
        valid["success"] = len(RUNTIME.UPSTREAM_VIEWPORT_TESTS) - 1
        errors = RUNTIME._validate_upstream_viewport_suite_payload(valid)
        self.assertIn(
            "registered_tests did not match the pinned upstream viewport manifest",
            errors,
        )
        self.assertIn(
            "upstream viewport suite requires exactly 11 successful terminal tests",
            errors,
        )

    def test_viewport_common_validation_preserves_error_order(self):
        wgpu_errors = RUNTIME._validate_viewport_payload(
            {
                "schema_version": 0,
                "secondary_viewport_while_held_observed": False,
                "merge_observed": False,
                "main_present_bracketed_by_test_engine": False,
                "secondary_render_submitted_before_main_acquire_viewport_ids": [],
                "secondary_present_submitted_before_main_acquire_viewport_ids": [-1],
            }
        )
        self.assertEqual(
            wgpu_errors,
            [
                "schema_version expected 3, got 0",
                "secondary_viewport_while_held_observed expected True, got False",
                "merge_observed expected True, got False",
                "main_present_bracketed_by_test_engine expected True, got False",
                "secondary_render_submitted_before_main_acquire_viewport_ids must be a nonempty u32 array",
                "secondary_present_submitted_before_main_acquire_viewport_ids must contain only u32 values",
                "adapter must be a JSON object",
            ],
        )

        sdl3_glow_errors = RUNTIME._validate_sdl3_glow_viewport_payload(
            {
                "schema_version": 0,
                "merge_observed": False,
                "main_present_bracketed_by_test_engine": False,
            }
        )
        self.assertEqual(
            sdl3_glow_errors,
            [
                "schema_version expected 5, got 0",
                "merge_observed expected True, got False",
                "main_present_bracketed_by_test_engine expected True, got False",
                "external_texture_filters_preserved expected True, got None",
                "sampler_pixels_prove_isolation expected True, got None",
                "raw_callback_typed_state_observed expected True, got None",
                "reset_render_state_recovered expected True, got None",
                "render_state_cleared_after_callback expected True, got None",
                "application_gl_state_restored expected True, got None",
                "sampler_strategy must be sampler_objects or texture_parameters, got None",
                "secondary_context_ready_before_main_present_viewport_ids must be a nonempty u32 array",
                "secondary_draw_issued_before_main_present_viewport_ids must be a nonempty u32 array",
                "secondary_swap_succeeded_before_main_present_viewport_ids must be a nonempty u32 array",
                "renderer must be a JSON object",
            ],
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

    def test_viewport_gate_retains_display_adapter_and_teardown_evidence(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            binary = root / "multi_viewport_wgpu"
            binary.touch()
            icd = root / "lvp_icd.x86_64.json"
            icd.write_text("{}", encoding="utf-8")
            tools = {
                name: root / name
                for name in (
                    "Xvfb",
                    "openbox",
                    "xdpyinfo",
                    "xprop",
                    "vulkaninfo",
                    "dpkg-query",
                )
            }
            build = bounded_result(
                stdout_log=evidence / "build.stdout.log",
                stderr_log=evidence / "build.stderr.log",
            )
            viewport_environment = {}
            upstream_viewport_environment = {}

            def background(command, **kwargs):
                return FakeBackground(
                    command,
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )

            def stage(_command, **kwargs):
                result = bounded_result(
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )
                if result.stdout_log.name == "adapter.stdout.log":
                    result.stdout_log.write_text(
                        "deviceName = llvmpipe (LLVM 20)\n", encoding="utf-8"
                    )
                elif result.stdout_log.name == "window-manager.stdout.log":
                    result.stdout_log.write_text(
                        "_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x200001\n",
                        encoding="utf-8",
                    )
                elif result.stdout_log.name == "viewport.stdout.log":
                    viewport_environment.update(kwargs["env"])
                    (evidence / "viewport-result.json").write_text(
                        json.dumps(
                            {
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
                                "secondary_render_submitted_before_main_acquire_viewport_ids": [42],
                                "secondary_present_submitted_before_main_acquire_viewport_ids": [42],
                                "main_present_bracketed_by_test_engine": True,
                            }
                        ),
                        encoding="utf-8",
                    )
                elif result.stdout_log.name == "upstream-viewports.stdout.log":
                    upstream_viewport_environment.update(kwargs["env"])
                    (evidence / "upstream-viewports-result.json").write_text(
                        json.dumps(
                            {
                                "schema_version": 1,
                                "suite": "upstream-viewports",
                                "category": "viewport",
                                "platform_backend": "Winit",
                                "renderer_backend": "WGPU",
                                "real_platform_backend": True,
                                "runtime_teardown_complete": True,
                                "registered_count": len(
                                    RUNTIME.UPSTREAM_VIEWPORT_TESTS
                                ),
                                "registered_tests": list(
                                    RUNTIME.UPSTREAM_VIEWPORT_TESTS
                                ),
                                "tested": len(RUNTIME.UPSTREAM_VIEWPORT_TESTS),
                                "success": len(RUNTIME.UPSTREAM_VIEWPORT_TESTS),
                                "in_queue": 0,
                                "adapter": {
                                    "name": "llvmpipe (LLVM 20)",
                                    "backend": "Vulkan",
                                    "device_type": "Cpu",
                                    "driver": "llvmpipe",
                                    "driver_info": "Mesa 25",
                                },
                            }
                        ),
                        encoding="utf-8",
                    )
                return result

            with (
                patch.object(RUNTIME, "_require_linux_runtime_tools", return_value=tools),
                patch.object(RUNTIME, "_find_lavapipe_icd", return_value=icd),
                patch.object(RUNTIME, "_run_example_build", return_value=build),
                patch.object(RUNTIME, "_example_binary", return_value=binary),
                patch.object(RUNTIME, "managed_background", side_effect=background),
                patch.object(RUNTIME, "_wait_for_xvfb"),
                patch.object(RUNTIME, "run_bounded", side_effect=stage),
                patch.object(RUNTIME.time, "sleep"),
                patch.dict(
                    os.environ,
                    {"DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE": "1"},
                ),
            ):
                result = RUNTIME.run_multi_viewport_smoke(
                    workspace_root=root,
                    evidence_dir=evidence,
                )

            self.assertTrue(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PASSED)
            self.assertTrue(result.details["xvfb"]["termination"]["attempted"])
            self.assertTrue(result.details["openbox"]["termination"]["attempted"])
            xdg_runtime = Path(result.details["environment"]["xdg_runtime_dir"])
            self.assertNotEqual(xdg_runtime.parent, evidence)
            self.assertTrue(xdg_runtime.name.startswith("dear-imgui-xdg-"))
            self.assertFalse(xdg_runtime.exists())
            self.assertIn("adapter.stdout.log", result.evidence)
            self.assertIn("viewport-result.json", result.evidence)
            self.assertIn("upstream-viewports-result.json", result.evidence)
            self.assertEqual(
                viewport_environment["DEAR_IMGUI_VIEWPORT_DRAG_SMOKE"],
                "1",
            )
            self.assertNotIn(
                "DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE", viewport_environment
            )
            self.assertNotIn(
                "DEAR_IMGUI_VIEWPORT_DRAG_SMOKE", upstream_viewport_environment
            )
            self.assertEqual(
                upstream_viewport_environment[
                    "DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE"
                ],
                "1",
            )

    def test_sdl3_glow_gate_retains_renderer_and_teardown_evidence(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            binary = root / "sdl3_glow_multi_viewport"
            binary.touch()
            sdl3_library = (
                root
                / "target"
                / "debug"
                / "build"
                / "sdl3-sys-test"
                / "out"
                / "lib"
                / "libSDL3.so.0"
            )
            sdl3_library.parent.mkdir(parents=True)
            sdl3_library.touch()
            tools = {
                name: root / name
                for name in (
                    "Xvfb",
                    "openbox",
                    "xdpyinfo",
                    "xprop",
                    "glxinfo",
                    "dpkg-query",
                )
            }
            build = bounded_result(
                stdout_log=evidence / "build.stdout.log",
                stderr_log=evidence / "build.stderr.log",
            )

            def background(command, **kwargs):
                return FakeBackground(
                    command,
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )

            def stage(_command, **kwargs):
                result = bounded_result(
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )
                if result.stdout_log.name == "renderer.stdout.log":
                    result.stdout_log.write_text(
                        "OpenGL renderer string: llvmpipe (LLVM 20)\n",
                        encoding="utf-8",
                    )
                elif result.stdout_log.name == "window-manager.stdout.log":
                    result.stdout_log.write_text(
                        "_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x200001\n",
                        encoding="utf-8",
                    )
                elif result.stdout_log.name in (
                    "viewport-texture-parameters.stdout.log",
                    "viewport-sampler-objects.stdout.log",
                ):
                    self.assertEqual(
                        kwargs["env"]["LD_LIBRARY_PATH"].split(os.pathsep)[0],
                        str(sdl3_library.parent.resolve()),
                    )
                    fallback = (
                        result.stdout_log.name
                        == "viewport-texture-parameters.stdout.log"
                    )
                    expected_strategy = (
                        "texture_parameters" if fallback else "sampler_objects"
                    )
                    self.assertEqual(
                        kwargs["env"]["MESA_GL_VERSION_OVERRIDE"],
                        "3.2" if fallback else "3.3",
                    )
                    if fallback:
                        self.assertEqual(
                            kwargs["env"]["MESA_EXTENSION_OVERRIDE"],
                            "-GL_ARB_sampler_objects",
                        )
                    else:
                        self.assertNotIn("MESA_EXTENSION_OVERRIDE", kwargs["env"])
                    result_path = Path(
                        kwargs["env"]["DEAR_IMGUI_VIEWPORT_SMOKE_JSON"]
                    )
                    result_path.write_text(
                        json.dumps(
                            {
                                "schema_version": 5,
                                "renderer": {
                                    "backend": "OpenGL",
                                    "vendor": "Mesa",
                                    "name": "llvmpipe (LLVM 20)",
                                    "version": "3.2 Mesa 25"
                                    if fallback
                                    else "3.3 Mesa 25",
                                },
                                "sampler_strategy": expected_strategy,
                                "merge_observed": True,
                                "secondary_context_ready_before_main_present_viewport_ids": [7],
                                "secondary_draw_issued_before_main_present_viewport_ids": [7],
                                "secondary_swap_succeeded_before_main_present_viewport_ids": [7],
                                "main_present_bracketed_by_test_engine": True,
                                "external_texture_filters_preserved": True,
                                "sampler_pixels_prove_isolation": True,
                                "raw_callback_typed_state_observed": True,
                                "reset_render_state_recovered": True,
                                "render_state_cleared_after_callback": True,
                                "application_gl_state_restored": True,
                            }
                        ),
                        encoding="utf-8",
                    )
                return result

            with (
                patch.object(
                    RUNTIME, "_require_linux_sdl3_glow_tools", return_value=tools
                ),
                patch.object(RUNTIME, "_run_example_build", return_value=build),
                patch.object(RUNTIME, "_example_binary", return_value=binary),
                patch.object(RUNTIME, "managed_background", side_effect=background),
                patch.object(RUNTIME, "_wait_for_xvfb"),
                patch.object(RUNTIME, "run_bounded", side_effect=stage),
                patch.object(RUNTIME.time, "sleep"),
            ):
                result = RUNTIME.run_sdl3_glow_viewport_smoke(
                    workspace_root=root,
                    evidence_dir=evidence,
                )

            self.assertTrue(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PASSED)
            self.assertIn("renderer.stdout.log", result.evidence)
            self.assertIn("viewport-texture-parameters-result.json", result.evidence)
            self.assertIn("viewport-sampler-objects-result.json", result.evidence)
            self.assertEqual(
                set(result.details["results"]),
                {"texture-parameters", "sampler-objects"},
            )
            self.assertEqual(
                result.details["environment"]["sdl3_library_dirs"],
                [str(sdl3_library.parent.resolve())],
            )

    def test_ash_vulkan_gate_requires_validation_and_retains_teardown_evidence(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            binary = root / "multi_viewport_ash"
            binary.touch()
            icd = root / "lvp_icd.x86_64.json"
            icd.write_text("{}", encoding="utf-8")
            tools = {
                name: root / name
                for name in (
                    "Xvfb",
                    "openbox",
                    "xdpyinfo",
                    "xprop",
                    "vulkaninfo",
                    "dpkg-query",
                )
            }
            build = bounded_result(
                stdout_log=evidence / "build.stdout.log",
                stderr_log=evidence / "build.stderr.log",
            )
            viewport_environment = {}

            def background(command, **kwargs):
                return FakeBackground(
                    command,
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )

            def stage(_command, **kwargs):
                result = bounded_result(
                    stdout_log=kwargs["stdout_log"],
                    stderr_log=kwargs["stderr_log"],
                )
                if result.stdout_log.name == "adapter.stdout.log":
                    result.stdout_log.write_text(
                        "deviceName = llvmpipe (LLVM 20)\n"
                        "VK_LAYER_KHRONOS_validation\n",
                        encoding="utf-8",
                    )
                elif result.stdout_log.name == "window-manager.stdout.log":
                    result.stdout_log.write_text(
                        "_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x200001\n",
                        encoding="utf-8",
                    )
                elif result.stdout_log.name == "viewport.stdout.log":
                    viewport_environment.update(kwargs["env"])
                    payload = ash_vulkan_smoke_payload()
                    (evidence / "viewport-result.json").write_text(
                        json.dumps(payload), encoding="utf-8"
                    )
                return result

            with (
                patch.object(RUNTIME, "_require_linux_runtime_tools", return_value=tools),
                patch.object(RUNTIME, "_find_lavapipe_icd", return_value=icd),
                patch.object(RUNTIME, "_run_example_build", return_value=build) as builder,
                patch.object(RUNTIME, "_example_binary", return_value=binary),
                patch.object(RUNTIME, "managed_background", side_effect=background),
                patch.object(RUNTIME, "_wait_for_xvfb"),
                patch.object(RUNTIME, "run_bounded", side_effect=stage),
                patch.object(RUNTIME.time, "sleep"),
            ):
                result = RUNTIME.run_ash_vulkan_validation_smoke(
                    workspace_root=root,
                    evidence_dir=evidence,
                )

            self.assertTrue(result.success)
            self.assertEqual(result.category, RUNTIME.GateCategory.PASSED)
            self.assertEqual(
                builder.call_args.kwargs["features"],
                "ash-winit-multi-viewport,ash-dynamic-rendering",
            )
            self.assertEqual(
                viewport_environment["DEAR_IMGUI_REQUIRE_VULKAN_VALIDATION"], "1"
            )
            self.assertEqual(
                viewport_environment["VK_ICD_FILENAMES"], str(icd)
            )
            self.assertIn("viewport-result.json", result.evidence)

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


class WorkflowPortabilityTests(unittest.TestCase):
    def _ci_jobs(self):
        workflow = load_workflow(REPO_ROOT / ".github" / "workflows" / "ci.yml")
        return workflow_jobs(workflow)

    def test_sanitizer_linker_includes_the_cxx_runtime(self):
        job = self._ci_jobs()["test-engine-sanitizers"]
        step = named_step(
            job, "Run Test Engine FFI boundary tests with ASan and UBSan"
        )
        environment = require_mapping(
            step.get("env"), "jobs.test-engine-sanitizers.steps.sanitizer.env"
        )

        self.assertIn("-Clinker=clang++", environment["RUSTFLAGS"])
        self.assertIn("-Clink-arg=-lstdc++", environment["RUSTFLAGS"])

    def test_binding_contract_installs_and_selects_the_pinned_libclang(self):
        jobs = self._ci_jobs()
        job = jobs["binding-contract"]
        install = named_step(job, "Install fixed LLVM toolchain")
        install_command = str(install.get("run", ""))
        self.assertNotIn("uses", install)
        self.assertIn("python3 tools/ci/install_llvm.py", install_command)
        self.assertIn("${{ runner.temp }}/llvm", install_command)
        self.assertFalse(
            any(
                "install-llvm-action" in str(step.get("uses", ""))
                for workflow_job in jobs.values()
                for step in workflow_job.get("steps", ())
                if isinstance(step, dict)
            ),
            "JavaScript LLVM setup actions must not re-enter the workflow",
        )

        step = named_step(
            job, "Regenerate and verify every maintained binding profile"
        )
        environment = require_mapping(
            step.get("env"), "jobs.binding-contract.steps.regenerate.env"
        )

        self.assertEqual(
            environment.get("LIBCLANG_PATH"), "${{ runner.temp }}/llvm/lib"
        )
        self.assertFalse(
            any(
                "install_llvm.py" in str(step.get("run", ""))
                for step in jobs["wasm-check"]["steps"]
            ),
            "WASM uses pregenerated bindings and must not install libclang",
        )

    def test_checkout_and_packaged_wasm_providers_use_the_pinned_emsdk(self):
        jobs = self._ci_jobs()
        for job_id in ("publish-check", "wasm-check"):
            setup = named_step(
                jobs[job_id], "Set up pinned Emscripten provider toolchain"
            )
            inputs = require_mapping(setup.get("with"), f"jobs.{job_id}.emsdk.with")
            with self.subTest(job=job_id):
                self.assertEqual(setup.get("uses"), "emscripten-core/setup-emsdk@v16")
                self.assertEqual(str(inputs.get("version")), "5.0.1")
                self.assertEqual(str(inputs.get("emsdk-version")), "5.0.5")
                self.assertNotIn("actions-cache-folder", inputs)

        checkout_provider = str(
            named_step(
                jobs["wasm-check"],
                "Build and structurally verify the Emscripten provider",
            ).get("run", "")
        )
        packaged_provider = str(
            named_step(
                jobs["publish-check"], "Package, unpack, and consume core crates"
            ).get("run", "")
        )
        self.assertIn("verify_wasm_provider.py", checkout_provider)
        self.assertIn("verify_packaged_core.py", packaged_provider)

    def test_ash_shader_contract_rebuilds_with_pinned_glslang(self):
        job = self._ci_jobs()["ash-routes"]
        install = named_step(job, "Build pinned glslangValidator")
        install_command = str(install.get("run", ""))
        self.assertEqual(
            install.get("if"),
            "matrix.platform == 'Winit' && matrix.rendering == 'classic'",
        )
        self.assertNotIn("shell", install)
        self.assertIn("tools/ci/build_glslang.py", install_command)
        self.assertIn("--work-root", install_command)
        self.assertIn("--github-env", install_command)
        self.assertEqual(
            BUILD_GLSLANG.GLSLANG_COMMIT,
            "1062752a891c95b2bfeed9e356562d88f9df84ac",
        )

        verify = named_step(job, "Verify checked-in Ash shaders")
        verify_command = str(verify.get("run", ""))
        self.assertIn("tools/generate_ash_shaders.py", verify_command)
        self.assertIn("--check", verify_command)
        self.assertIn("--recompile", verify_command)
        self.assertIn('--compiler "$GLSLANG_VALIDATOR"', verify_command)

    def test_native_runtime_retry_chain_retains_release_evidence(self):
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        runtime = (
            REPO_ROOT / ".github" / "workflows" / "native-runtime.yml"
        ).read_text(encoding="utf-8")
        workflow = f"{ci}\n{runtime}"

        self.assertIn("test-engine-runtime", workflow)
        self.assertIn("multi-viewport-smoke", workflow)
        self.assertIn("sdl3-glow-multi-viewport-smoke", workflow)
        self.assertIn("ash-vulkan-validation-smoke", workflow)
        self.assertIn("xvfb", runtime)
        self.assertIn("openbox", runtime)
        self.assertIn("mesa-vulkan-drivers", runtime)
        self.assertIn("vulkan-tools", runtime)
        self.assertIn("vulkan-validationlayers", runtime)
        self.assertIn("libxkbcommon-x11-dev", runtime)
        self.assertIn("mesa-utils", runtime)
        self.assertIn("retention-days: 30", workflow)
        self.assertRegex(workflow, r"(?m)^\s+if: always\(\)$")
        self.assertIn("--defer-infrastructure-retry", runtime)
        self.assertIn("runtime-preparation-failure", runtime)
        self.assertIn("steps.preparation.outputs.retry_eligible", runtime)
        self.assertIn("gate_attempt: 2", ci)
        self.assertEqual(ci.count("outputs.retry_eligible == 'true'"), 4)

    def test_bevy_software_gpu_contracts_run_serially(self):
        job = self._ci_jobs()["bevy-backend"]
        for step_name in (
            "Run Bevy GPU composition tests",
            "Run Bevy image bind-group GPU contracts",
        ):
            command = str(named_step(job, step_name).get("run", ""))
            self.assertIn("nextest run -j 1", command)

    def test_safe_test_engine_contracts_cover_default_and_capture_serially(self):
        job = self._ci_jobs()["build"]
        default_command = str(
            named_step(job, "Test safe Test Engine runner contracts").get("run", "")
        )
        capture_command = str(
            named_step(job, "Test safe Test Engine capture contracts").get("run", "")
        )

        for command in (default_command, capture_command):
            self.assertIn("nextest run -j 1", command)
            self.assertIn("-p dear-imgui-test-engine", command)
            self.assertIn("--no-default-features", command)
        self.assertNotIn("--features capture", default_command)
        self.assertIn("--features capture", capture_command)

    def test_glow_runtime_contracts_have_native_and_wasm_ci_coverage(self):
        jobs = self._ci_jobs()
        native = str(
            named_step(jobs["build"], "Run Glow renderer acceptance tests").get("run", "")
        )
        self.assertIn("cargo nextest run", native)
        self.assertIn("-p dear-imgui-glow", native)
        self.assertIn("--features multi-viewport", native)

        wasm = str(
            named_step(jobs["wasm-check"], "Check Glow renderer WASM provider").get(
                "run", ""
            )
        )
        self.assertIn("cargo check", wasm)
        self.assertIn("-p dear-imgui-glow", wasm)
        self.assertIn("--target wasm32-unknown-unknown", wasm)
        self.assertIn("--no-default-features", wasm)
        self.assertIn("--features wasm", wasm)


class GlowFeatureManifestTests(unittest.TestCase):
    def test_glow_manifest_exposes_only_runtime_truthful_capability_routes(self):
        manifest = tomllib.loads(
            (REPO_ROOT / "backends" / "dear-imgui-glow" / "Cargo.toml").read_text(
                encoding="utf-8"
            )
        )
        features = manifest["features"]
        self.assertEqual(set(features), {"default", "wasm", "multi-viewport"})
        self.assertEqual(features["default"], [])
        self.assertEqual(features["wasm"], ["dear-imgui-rs/wasm"])
        self.assertEqual(
            features["multi-viewport"], ["dear-imgui-rs/multi-viewport"]
        )


if __name__ == "__main__":
    unittest.main()
