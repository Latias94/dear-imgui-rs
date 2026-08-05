import importlib
import re
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import Mock


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

UPDATE_SUBMODULES = importlib.import_module("update_submodules")


EXPECTED_COMMANDS = (
    (
        "git",
        "-C",
        "dear-imgui-sys/third-party/cimgui",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        "imgui",
    ),
    (
        "git",
        "-C",
        "extensions/dear-implot-sys/third-party/cimplot",
        "submodule",
        "update",
        "--init",
        "implot",
    ),
    (
        "git",
        "-C",
        "extensions/dear-implot3d-sys/third-party/cimplot3d",
        "submodule",
        "update",
        "--init",
        "implot3d",
    ),
    (
        "git",
        "-C",
        "extensions/dear-imguizmo-sys/third-party/cimguizmo",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        "ImGuizmo",
    ),
    (
        "git",
        "-C",
        "extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        "imGuIZMO.quat",
    ),
    (
        "git",
        "-C",
        "extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat/imGuIZMO.quat",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        "libs/imgui",
    ),
    (
        "git",
        "-C",
        "extensions/dear-imnodes-sys/third-party/cimnodes",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        "imnodes",
    ),
    (
        "git",
        "-C",
        "extensions/dear-node-editor-sys/third-party/cimnodes_editor",
        "submodule",
        "update",
        "--init",
        "--depth=1",
        "imgui-node-editor",
    ),
)


class UpdateSubmodulesTests(unittest.TestCase):
    def test_command_list_preserves_selective_checkout_contract(self):
        self.assertEqual(
            UPDATE_SUBMODULES.SUBMODULE_PROFILES["all"], EXPECTED_COMMANDS
        )

    def test_runtime_profiles_initialize_only_required_top_level_sources(self):
        core = (
            "git",
            "submodule",
            "update",
            "--init",
            "--depth=1",
            "dear-imgui-sys/third-party/cimgui",
        )
        test_engine = (
            "git",
            "submodule",
            "update",
            "--init",
            "--depth=1",
            "extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine",
        )

        self.assertEqual(
            UPDATE_SUBMODULES.SUBMODULE_PROFILES["runtime-core"],
            (core, EXPECTED_COMMANDS[0]),
        )
        self.assertEqual(
            UPDATE_SUBMODULES.SUBMODULE_PROFILES["runtime-test-engine"],
            (core, test_engine, EXPECTED_COMMANDS[0]),
        )

    def test_retry_uses_exponential_backoff_until_success(self):
        command = ("git", "status")
        runner = Mock(
            side_effect=(
                subprocess.CompletedProcess(command, 1),
                subprocess.CompletedProcess(command, 1),
                subprocess.CompletedProcess(command, 0),
            )
        )
        sleeper = Mock()

        UPDATE_SUBMODULES.retry(command, runner=runner, sleeper=sleeper)

        self.assertEqual(runner.call_count, 3)
        self.assertTrue(
            all(call.kwargs["timeout"] == 180 for call in runner.call_args_list)
        )
        self.assertEqual(
            [call.args[0] for call in sleeper.call_args_list],
            [5, 10],
        )

    def test_retry_recovers_after_a_timed_out_attempt(self):
        command = ("git", "status")
        runner = Mock(
            side_effect=(
                subprocess.TimeoutExpired(command, timeout=30),
                subprocess.CompletedProcess(command, 0),
            )
        )
        sleeper = Mock()

        UPDATE_SUBMODULES.retry(
            command,
            runner=runner,
            sleeper=sleeper,
            timeout_seconds=30,
        )

        self.assertEqual(runner.call_count, 2)
        self.assertEqual(runner.call_args_list[0].kwargs["timeout"], 30)
        sleeper.assert_called_once_with(5)

    def test_retry_propagates_the_final_failure_after_five_attempts(self):
        command = ("git", "status")
        runner = Mock(return_value=subprocess.CompletedProcess(command, returncode=23))
        sleeper = Mock()

        with self.assertRaises(subprocess.CalledProcessError) as raised:
            UPDATE_SUBMODULES.retry(command, runner=runner, sleeper=sleeper)

        self.assertEqual(raised.exception.returncode, 23)
        self.assertEqual(runner.call_count, 5)
        self.assertEqual(
            [call.args[0] for call in sleeper.call_args_list],
            [5, 10, 20, 40],
        )

    def test_help_explains_the_top_level_submodule_precondition(self):
        help_text = UPDATE_SUBMODULES._build_parser().format_help()

        self.assertIn(
            "default profile expects top-level repository submodules", help_text
        )
        self.assertIn("runtime-test-engine", help_text)


class RepositoryScriptContractTests(unittest.TestCase):
    def test_canonical_bindgen_dependency_matches_provenance_contract(self):
        xtask_manifest = (REPO_ROOT / "xtask" / "Cargo.toml").read_text(
            encoding="utf-8"
        )
        build_support = (
            REPO_ROOT / "tools" / "build-support" / "src" / "lib.rs"
        ).read_text(encoding="utf-8")
        dependency = re.search(r'^bindgen = "=([0-9.]+)"$', xtask_manifest, re.MULTILINE)
        contract = re.search(
            r'CANONICAL_BINDGEN_VERSION: &str = "([0-9.]+)"', build_support
        )

        self.assertIsNotNone(dependency)
        self.assertIsNotNone(contract)
        self.assertEqual(dependency.group(1), contract.group(1))

    def test_binding_updater_previews_wasm_generation_without_running_cargo(self):
        result = subprocess.run(
            [
                sys.executable,
                "tools/update_submodule_and_bindings.py",
                "--crates",
                "dear-imgui-sys",
                "--submodules",
                "skip",
                "--wasm",
                "--dry-run",
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("WASM pregenerated bindings would be generated at", result.stdout)
        self.assertIn(
            "cargo check -p dear-imgui-rs -F wasm --target wasm32-unknown-unknown",
            result.stdout,
        )

if __name__ == "__main__":
    unittest.main()
