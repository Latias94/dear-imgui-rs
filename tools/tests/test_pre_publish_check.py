import io
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

from tools.tests.release_test_support import (
    REPO_ROOT,
    load_tool,
    metadata_for,
    package,
    release_metadata,
)


PREPUBLISH = load_tool("pre_publish_check")
PUBLISH = load_tool("publish")


class PrepublishTests(unittest.TestCase):
    def setUp(self):
        self.metadata = metadata_for(
            [package("dear-imgui-rs", "dear-imgui", version="0.16.0")]
        )

    def test_changelog_uses_metadata_release_version(self):
        with (
            patch.object(PREPUBLISH, "run_command", return_value=(0, "", "")) as run,
            redirect_stdout(io.StringIO()),
        ):
            success, errors = PREPUBLISH.check_changelog_release_notes(
                Path("/repo"), self.metadata
            )

        self.assertTrue(success)
        self.assertEqual(errors, [])
        commands = [call.args[0] for call in run.call_args_list]
        changelog_tool = str(Path("/repo") / "tools" / "changelog.py")
        self.assertEqual(
            commands,
            [
                [
                    sys.executable,
                    changelog_tool,
                    "check-unreleased",
                ],
                [
                    sys.executable,
                    changelog_tool,
                    "extract",
                    "--version",
                    "0.16.0",
                ],
                [
                    sys.executable,
                    changelog_tool,
                    "check-soft-wrap",
                    "--version",
                    "0.16.0",
                ],
            ],
        )

    def test_changelog_missing_core_is_reported_without_traceback(self):
        metadata = metadata_for(
            [package("dear-imgui-sys", "dear-imgui-sys", publish=False)]
        )
        with redirect_stdout(io.StringIO()):
            success, errors = PREPUBLISH.check_changelog_release_notes(
                Path("/repo"), metadata
            )

        self.assertFalse(success)
        self.assertTrue(any("workspace package not found" in error for error in errors))

    def test_package_gate_runs_shared_strict_script(self):
        with (
            patch.object(PREPUBLISH, "run_command", return_value=(0, "", "")) as run,
            redirect_stdout(io.StringIO()),
        ):
            success, errors = PREPUBLISH.check_packaged_core(Path("/repo"))

        self.assertTrue(success)
        self.assertEqual(errors, [])
        run.assert_called_once_with(
            [
                sys.executable,
                str(Path("/repo") / "tools" / "ci" / "verify_packaged_core.py"),
            ],
            cwd=Path("/repo"),
            capture=True,
            show_output=True,
        )

    def test_release_contract_gate_stops_on_first_failed_command(self):
        with (
            patch.object(
                PREPUBLISH,
                "run_command",
                side_effect=[(0, "", ""), (23, "", "workflow drift")],
            ) as run,
            redirect_stdout(io.StringIO()),
        ):
            success, errors = PREPUBLISH.check_release_contracts(Path("/repo"))

        self.assertFalse(success)
        self.assertEqual(errors, ["Workflow policy failed: workflow drift"])
        self.assertEqual(run.call_count, 2)

    def test_default_prepublish_includes_release_contract_gate(self):
        passing = (True, [])
        with (
            patch.object(PREPUBLISH.sys, "argv", ["pre_publish_check.py"]),
            patch.object(
                PREPUBLISH,
                "read_locked_workspace_metadata",
                return_value=(self.metadata, passing),
            ),
            patch.object(PREPUBLISH, "check_core_source_contract", return_value=passing),
            patch.object(PREPUBLISH, "check_core_binding_contract", return_value=passing),
            patch.object(PREPUBLISH, "check_version_consistency", return_value=passing),
            patch.object(PREPUBLISH, "check_pregenerated_bindings", return_value=passing),
            patch.object(PREPUBLISH, "check_git_status", return_value=passing),
            patch.object(PREPUBLISH, "check_changelog_release_notes", return_value=passing),
            patch.object(PREPUBLISH, "check_docs_build", return_value=passing),
            patch.object(
                PREPUBLISH, "check_release_contracts", return_value=passing
            ) as release_contracts,
            patch.object(PREPUBLISH, "check_tests", return_value=passing),
            patch.object(PREPUBLISH, "check_packaged_core", return_value=passing),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(PREPUBLISH.main(), 0)

        release_contracts.assert_called_once_with(REPO_ROOT)

    def test_nextest_release_tests_are_per_package_and_cover_integrations(self):
        commands = PREPUBLISH.release_test_commands(use_nextest=True)
        labels = [label for label, _command in commands]
        package_labels = labels[: len(release_metadata.PUBLISH_ORDER)]

        self.assertEqual(
            package_labels,
            [name for name, _path in release_metadata.PUBLISH_ORDER],
        )
        self.assertIn("dear-imgui-reflect", package_labels)
        self.assertIn("dear-file-browser", package_labels)
        self.assertIn("dear-app", package_labels)
        self.assertIn("xtask", labels)
        for _label, command in commands:
            self.assertNotIn("--workspace", command)
            self.assertNotIn("--lib", command)

        stack_command = dict(commands)["dear-imgui-rs stack-layout integration"]
        self.assertEqual(stack_command[-2:], ["--test", "stack_layout_context"])
        self.assertIn("stack-layout", stack_command)

        tracing_command = dict(commands)["dear-imgui-wgpu tracing"]
        self.assertIn("--no-default-features", tracing_command)
        self.assertEqual(tracing_command[-2:], ["--features", "wgpu-30,tracing"])

    def test_cargo_test_fallback_is_serial_for_every_profile(self):
        commands = PREPUBLISH.release_test_commands(use_nextest=False)

        for _label, command in commands:
            self.assertEqual(command[-2:], ["--", "--test-threads=1"])
            self.assertNotIn("--workspace", command)
            self.assertNotIn("--lib", command)

    def test_skip_git_requires_skipping_head_only_package_gate(self):
        stderr = io.StringIO()
        with (
            patch.object(
                sys,
                "argv",
                ["pre_publish_check.py", "--skip-git-check"],
            ),
            redirect_stderr(stderr),
            self.assertRaises(SystemExit) as raised,
        ):
            PREPUBLISH.main()

        self.assertEqual(raised.exception.code, 2)
        self.assertIn("clean clone of HEAD", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
