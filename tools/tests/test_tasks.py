import importlib.util
import io
import unittest
from contextlib import redirect_stdout
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
    def test_updater_regenerates_again_to_prove_idempotence(self):
        commands = UPDATER.core_binding_commands()
        self.assertEqual(
            commands[0][-3:],
            ["verify-bindings", "--update", "--allow-dirty"],
        )
        self.assertEqual(commands[1][-2:], ["verify-bindings", "--allow-dirty"])
        self.assertNotIn("--check-only", commands[1])

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
        self.assertEqual(
            commands[2],
            [
                "cargo", "run", "-p", "xtask", "--", "verify-bindings",
                "--allow-dirty",
            ],
        )

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
        self.assertEqual(output.getvalue().count("xtask -- verify-bindings"), 2)


if __name__ == "__main__":
    unittest.main()
