import importlib
import os
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

CONTRACTS = importlib.import_module("run_contract")


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

    def test_release_notes_publish_validated_tag_outputs(self):
        with TemporaryDirectory() as temporary:
            output = Path(temporary) / "release-notes.md"
            github_output = Path(temporary) / "github-output.txt"
            with patch.object(CONTRACTS, "run") as runner:
                CONTRACTS.prepare_release_notes("v0.16.0", output, github_output)

            self.assertEqual(
                github_output.read_text(encoding="utf-8"),
                "tag=v0.16.0\nversion=0.16.0\n",
            )
            self.assertEqual(runner.call_count, 2)
            extract_command = tuple(runner.call_args_list[1].args[0])
            self.assertEqual(extract_command[-2:], ("--output", output))

    def test_release_notes_reject_shell_metacharacters_in_tag(self):
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(
                CONTRACTS.VerificationError, "invalid release tag"
            ):
                CONTRACTS.prepare_release_notes(
                    "v0.16.0;echo", Path(temporary) / "notes", Path(temporary) / "out"
                )

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


class WorkflowPortabilityTests(unittest.TestCase):
    def test_workflows_contain_no_explicit_bash_control_flow(self):
        workflows = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml"))
        )

        self.assertNotIn("shell: bash", workflows)
        self.assertNotIn("set -euo pipefail", workflows)
        self.assertNotIn("PIPESTATUS", workflows)

    def test_repository_contains_no_owned_shell_script_files(self):
        ignored_roots = {".git", "repo-ref", "target", "third-party"}
        scripts = []
        for root, directories, files in os.walk(REPO_ROOT):
            directories[:] = [
                directory for directory in directories if directory not in ignored_roots
            ]
            scripts.extend(
                Path(root) / filename for filename in files if filename.endswith(".sh")
            )

        self.assertEqual(scripts, [])


if __name__ == "__main__":
    unittest.main()
