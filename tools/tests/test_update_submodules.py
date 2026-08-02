import importlib
import os
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
        self.assertEqual(UPDATE_SUBMODULES.SUBMODULE_COMMANDS, EXPECTED_COMMANDS)

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
        self.assertEqual(
            [call.args[0] for call in sleeper.call_args_list],
            [5, 10],
        )

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
            "Top-level repository submodules must already be initialized", help_text
        )


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

    def test_binding_updater_uses_canonical_xtask_without_out_dir_guessing(self):
        content = (
            REPO_ROOT / "tools" / "update_submodule_and_bindings.py"
        ).read_text(encoding="utf-8")

        self.assertIn("verify-bindings", content)
        self.assertNotIn("find_bindings", content)
        self.assertNotIn("st_mtime", content)

    def test_binding_updater_selects_imguizmo_repositories_independently(self):
        content = (
            REPO_ROOT / "tools" / "update_submodule_and_bindings.py"
        ).read_text(encoding="utf-8")

        self.assertIn('"--cimguizmo-branch"', content)
        self.assertIn('"--cimguizmo-quat-branch"', content)
        self.assertIn("args.cimguizmo_quat_branch", content)

    def test_binding_updater_previews_wasm_generation_without_running_cargo(self):
        result = subprocess.run(
            [
                sys.executable,
                "tools/update_submodule_and_bindings.py",
                "--crates",
                "dear-imgui-sys",
                "--submodules",
                "skip",
                "--skip-core-bindings",
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

    def test_repository_has_no_tracked_non_python_script_entry_points(self):
        result = subprocess.run(
            [
                "git",
                "ls-files",
                "*.sh",
                "*.bash",
                "*.zsh",
                "*.ps1",
                "*.bat",
                "*.cmd",
            ],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        existing = [
            path for path in result.stdout.splitlines() if (REPO_ROOT / path).is_file()
        ]

        self.assertEqual(existing, [])

    def test_repository_has_no_extensionless_shell_entry_points(self):
        result = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
        )
        shell_entry_points = []
        for raw_path in result.stdout.split(b"\0"):
            if not raw_path:
                continue
            path = REPO_ROOT / os.fsdecode(raw_path)
            if not path.is_file():
                continue
            with path.open("rb") as source:
                first_line = source.readline(256)
            if first_line.startswith(b"#!") and any(
                shell in first_line for shell in (b"/sh", b"/bash", b"/zsh", b"/ksh")
            ):
                shell_entry_points.append(os.fspath(path.relative_to(REPO_ROOT)))

        self.assertEqual(shell_entry_points, [])

    def test_active_callers_use_python_entry_points(self):
        active_paths = (
            REPO_ROOT / ".github" / "workflows" / "ci.yml",
            REPO_ROOT / ".github" / "workflows" / "prebuilt-binaries.yml",
            REPO_ROOT / "tools" / "pre_publish_check.py",
            REPO_ROOT / "examples-ios" / "dear-imgui-ios-smoke" / "README.md",
        )
        content = "\n".join(path.read_text(encoding="utf-8") for path in active_paths)

        self.assertNotIn("update_submodules.sh", content)
        self.assertNotIn("verify_packaged_core.sh", content)
        self.assertNotIn("build-xcframework.sh", content)

    def test_workflows_select_a_python_command_available_on_each_runner(self):
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )

        def job_block(name: str) -> str:
            tail = ci.split(f"\n  {name}:\n", 1)[1]
            next_job = re.search(r"\n  [a-zA-Z0-9_-]+:\n", tail)
            return tail if next_job is None else tail[: next_job.start()]

        for job_name in (
            "windows-platform-io-abi",
            "windows-vcpkg-native-deps",
            "windows-gnu",
        ):
            job = job_block(job_name)
            with self.subTest(job=job_name):
                self.assertIn("run: python tools/ci/update_submodules.py", job)

        runner_expression = "runner.os == 'Windows' && 'python' || 'python3'"
        self.assertIn(runner_expression, job_block("build"))

        prebuilt = (
            REPO_ROOT / ".github" / "workflows" / "prebuilt-binaries.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("python: python3", prebuilt)
        self.assertIn("python: python", prebuilt)
        self.assertGreaterEqual(prebuilt.count("${{ matrix.python }}"), 5)


if __name__ == "__main__":
    unittest.main()
