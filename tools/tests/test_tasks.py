import importlib.util
import io
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("dear_imgui_tasks", REPO_ROOT / "tools/tasks.py")
TASKS = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(TASKS)

UPDATER_SPEC = importlib.util.spec_from_file_location(
    "dear_imgui_binding_updater",
    REPO_ROOT / "tools/update_submodule_and_bindings.py",
)
UPDATER = importlib.util.module_from_spec(UPDATER_SPEC)
assert UPDATER_SPEC.loader is not None
UPDATER_SPEC.loader.exec_module(UPDATER)


class BindingTaskTests(unittest.TestCase):
    def test_updater_uses_the_idempotent_canonical_generation_command(self):
        command = UPDATER.binding_command()
        self.assertEqual(
            command[-3:],
            ["verify-bindings", "--update", "--allow-dirty"],
        )
        self.assertNotIn("--check-only", command)

    def test_core_uses_shared_three_profile_xtask(self):
        args = SimpleNamespace(
            crates="dear-imgui-sys,dear-implot-sys",
            update_submodules=False,
            dry_run=False,
        )
        with patch.object(TASKS, "run_command", return_value=0) as run_command:
            self.assertEqual(TASKS.task_bindings(args, REPO_ROOT), 0)

        commands = [call.args[0] for call in run_command.call_args_list]
        self.assertIn("--skip-core-bindings", commands[0])
        self.assertEqual(
            commands[1],
            [
                "cargo", "run", "-p", "xtask", "--", "verify-bindings",
                "--update", "--allow-dirty",
            ],
        )
        self.assertEqual(len(commands), 2)

    def test_extension_only_keeps_the_existing_update_flow(self):
        args = SimpleNamespace(
            crates="dear-implot-sys",
            update_submodules=True,
            dry_run=False,
        )
        with patch.object(TASKS, "run_command", return_value=0) as run_command:
            self.assertEqual(TASKS.task_bindings(args, REPO_ROOT), 0)

        self.assertEqual(run_command.call_count, 1)
        command = run_command.call_args.args[0]
        self.assertNotIn("--skip-core-bindings", command)
        self.assertEqual(command[-2:], ["--submodules", "update"])

    def test_dry_run_prints_core_commands_without_executing_them(self):
        args = SimpleNamespace(crates=None, update_submodules=False, dry_run=True)
        output = io.StringIO()
        with (
            patch.object(TASKS, "run_command", return_value=0) as run_command,
            redirect_stdout(output),
        ):
            self.assertEqual(TASKS.task_bindings(args, REPO_ROOT), 0)

        self.assertEqual(run_command.call_count, 1)
        self.assertIn("--dry-run", run_command.call_args.args[0])
        self.assertEqual(output.getvalue().count("xtask -- verify-bindings"), 1)


class ReleaseTaskTests(unittest.TestCase):
    def release_args(self, **overrides):
        values = {
            "version": "0.17.0",
            "dry_run": False,
            "skip_tool_tests": False,
        }
        values.update(overrides)
        return SimpleNamespace(**values)

    def test_release_prepare_requires_a_completely_clean_worktree(self):
        status = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=" M Cargo.toml\n?? notes.txt\n", stderr=""
        )
        with (
            patch.object(TASKS.subprocess, "run", return_value=status),
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()) as stderr,
        ):
            self.assertEqual(TASKS.require_clean_worktree(REPO_ROOT), 1)

        self.assertIn("completely clean worktree", stderr.getvalue())

    def test_release_prepare_runs_mutating_steps_in_order(self):
        args = self.release_args()
        with (
            patch.object(TASKS, "require_clean_worktree", return_value=0),
            patch.object(TASKS, "run_command", return_value=0) as run_command,
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(TASKS.task_release_prepare(args, REPO_ROOT), 0)

        commands = [call.args[0] for call in run_command.call_args_list]
        self.assertEqual(
            commands,
            [
                [
                    "cargo",
                    "run",
                    "--locked",
                    "-p",
                    "xtask",
                    "--",
                    "release-version",
                    "0.17.0",
                ],
                ["cargo", "metadata", "--no-deps", "--format-version", "1"],
                [
                    "cargo",
                    "metadata",
                    "--locked",
                    "--no-deps",
                    "--format-version",
                    "1",
                ],
                [
                    TASKS.sys.executable,
                    "tools/update_submodule_and_bindings.py",
                    "--crates",
                    "all",
                    "--profile",
                    "release",
                    "--submodules",
                    "skip",
                    "--skip-core-bindings",
                ],
                [
                    "cargo",
                    "run",
                    "-p",
                    "xtask",
                    "--",
                    "verify-bindings",
                    "--update",
                    "--allow-dirty",
                ],
                [
                    TASKS.sys.executable,
                    "-B",
                    "-m",
                    "unittest",
                    "discover",
                    "-s",
                    "tools/tests",
                    "-p",
                    "test_*.py",
                ],
            ],
        )
        self.assertFalse(
            any("pre_publish_check.py" in command for command in commands)
        )

    def test_release_prepare_dry_run_does_not_run_mutating_commands(self):
        args = self.release_args(dry_run=True)
        output = io.StringIO()
        with (
            patch.object(TASKS, "require_clean_worktree", return_value=0),
            patch.object(TASKS, "run_command", return_value=0) as run_command,
            redirect_stdout(output),
        ):
            self.assertEqual(TASKS.task_release_prepare(args, REPO_ROOT), 0)

        commands = [call.args[0] for call in run_command.call_args_list]
        self.assertEqual(commands[0][-1], "--dry-run")
        self.assertEqual(
            commands[1],
            [
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ],
        )
        self.assertIn("--dry-run", commands[2])
        self.assertEqual(
            commands[3][1:5], ["-B", "-m", "unittest", "discover"]
        )
        self.assertEqual(len(commands), 4)
        self.assertIn("cargo metadata --no-deps --format-version 1", output.getvalue())
        self.assertIn("skipped by --dry-run", output.getvalue())

    def test_release_prepare_stops_on_first_failed_step(self):
        args = self.release_args()
        with (
            patch.object(TASKS, "require_clean_worktree", return_value=0),
            patch.object(TASKS, "run_command", return_value=17) as run_command,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(TASKS.task_release_prepare(args, REPO_ROOT), 17)

        self.assertEqual(run_command.call_count, 1)

    def test_release_prepare_stops_when_lock_refresh_fails(self):
        args = self.release_args()
        with (
            patch.object(TASKS, "require_clean_worktree", return_value=0),
            patch.object(TASKS, "run_command", side_effect=[0, 23]) as run_command,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(TASKS.task_release_prepare(args, REPO_ROOT), 23)

        self.assertEqual(run_command.call_count, 2)
        self.assertEqual(
            run_command.call_args_list[-1].args[0],
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        )

    def test_release_prepare_can_skip_focused_tool_tests(self):
        args = self.release_args(skip_tool_tests=True)
        with (
            patch.object(TASKS, "require_clean_worktree", return_value=0),
            patch.object(TASKS, "run_command", return_value=0) as run_command,
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(TASKS.task_release_prepare(args, REPO_ROOT), 0)

        commands = [call.args[0] for call in run_command.call_args_list]
        self.assertFalse(any("unittest" in command for command in commands))

    def test_release_prepare_stops_before_commands_when_worktree_is_dirty(self):
        with (
            patch.object(TASKS, "require_clean_worktree", return_value=1),
            patch.object(TASKS, "run_command") as run_command,
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(
                TASKS.task_release_prepare(self.release_args(), REPO_ROOT), 1
            )

        run_command.assert_not_called()

    def test_release_check_delegates_to_strict_prepublish_without_skips(self):
        with patch.object(TASKS, "run_command", return_value=0) as run_command:
            self.assertEqual(
                TASKS.task_release_check(SimpleNamespace(), REPO_ROOT), 0
            )

        run_command.assert_called_once_with(
            [TASKS.sys.executable, "tools/pre_publish_check.py"],
            cwd=REPO_ROOT,
        )

    def test_publish_forwards_authoritative_gate_result(self):
        args = SimpleNamespace(
            dry_run=False,
            no_verify=False,
            crates="dear-imgui-sys",
            start_from=None,
            wait=7,
            release_gate_result=Path("artifacts/gate-result.json"),
            dangerously_skip_release_check=False,
        )
        with patch.object(TASKS, "run_command", return_value=0) as run_command:
            self.assertEqual(TASKS.task_publish(args, REPO_ROOT), 0)

        self.assertEqual(
            run_command.call_args.args[0],
            [
                TASKS.sys.executable,
                "tools/publish.py",
                "--crates",
                "dear-imgui-sys",
                "--wait",
                "7",
                "--release-gate-result",
                str(Path("artifacts/gate-result.json")),
            ],
        )

    def test_publish_forwards_explicit_dangerous_bypass(self):
        args = SimpleNamespace(
            dry_run=False,
            no_verify=False,
            crates=None,
            start_from=None,
            wait=None,
            release_gate_result=None,
            dangerously_skip_release_check=True,
        )
        with patch.object(TASKS, "run_command", return_value=0) as run_command:
            self.assertEqual(TASKS.task_publish(args, REPO_ROOT), 0)

        self.assertEqual(
            run_command.call_args.args[0],
            [
                TASKS.sys.executable,
                "tools/publish.py",
                "--dangerously-skip-release-check",
            ],
        )

    def test_check_requires_package_skip_when_git_check_is_skipped(self):
        args = SimpleNamespace(
            skip_git=True,
            skip_doc=False,
            skip_test=False,
            skip_package=False,
        )
        with (
            patch.object(TASKS, "run_command") as run_command,
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(TASKS.task_check(args, REPO_ROOT), 1)
        run_command.assert_not_called()

        args.skip_package = True
        with patch.object(TASKS, "run_command", return_value=0) as run_command:
            self.assertEqual(TASKS.task_check(args, REPO_ROOT), 0)
        command = run_command.call_args.args[0]
        self.assertIn("--skip-git-check", command)
        self.assertIn("--skip-package-check", command)

    def test_updater_dry_run_does_not_require_generated_artifacts(self):
        with (
            patch.object(
                UPDATER.sys,
                "argv",
                [
                    "update_submodule_and_bindings.py",
                    "--crates",
                    "dear-implot-sys",
                    "--profile",
                    "release",
                    "--submodules",
                    "skip",
                    "--dry-run",
                ],
            ),
            redirect_stdout(io.StringIO()) as output,
        ):
            self.assertEqual(UPDATER.main(), 0)

        self.assertIn("verify-bindings --update --allow-dirty", output.getvalue())
        self.assertFalse(hasattr(UPDATER, "find_bindings"))


if __name__ == "__main__":
    unittest.main()
