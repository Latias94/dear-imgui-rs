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
CANDIDATE_SHA = "a" * 40
FIXTURE_VERSION = "1.2.3-alpha.4"


def write_release_workspace(root: Path) -> None:
    (root / "Cargo.toml").write_text(
        f'[workspace.package]\nversion = "{FIXTURE_VERSION}"\n',
        encoding="utf-8",
    )


class ExpectedFailureTests(unittest.TestCase):
    def test_accepts_nonzero_exit_with_every_required_diagnostic(self):
        result = subprocess.CompletedProcess(
            args=[], returncode=101, stdout="first contract\nsecond contract\n"
        )
        with patch.object(CONTRACTS, "run", return_value=result) as runner:
            CONTRACTS.expect_failure(
                "fixture", ("first contract", "second contract"), ("cargo", "check")
            )

        self.assertIsNone(runner.call_args.kwargs["accepted_returncodes"])
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

    def test_clippy_places_only_explicit_allowances_after_deny_warnings(self):
        with (
            patch.dict(
                os.environ,
                {"CLIPPY_HISTORICAL_LINTS": "-A dead_code -A clippy::needless_borrow"},
            ),
            patch.object(CONTRACTS, "run") as runner,
        ):
            CONTRACTS.run_clippy(("--", "-p", "dear-imgui-rs", "--lib"))

        command = tuple(runner.call_args.args[0])
        self.assertEqual(command[:2], ("cargo", "clippy"))
        divider = command.index("--")
        self.assertEqual(command[divider + 1 : divider + 3], ("-D", "warnings"))
        self.assertEqual(
            command[divider + 3 :],
            ("-A", "dead_code", "-A", "clippy::needless_borrow"),
        )

    def test_clippy_rejects_non_allowance_flags(self):
        with (
            patch.dict(os.environ, {"CLIPPY_HISTORICAL_LINTS": "-W dead_code"}),
            self.assertRaisesRegex(
                CONTRACTS.VerificationError, "may contain only -A allowances"
            ),
        ):
            CONTRACTS.run_clippy(("--", "--workspace"))

    def test_release_notes_use_the_validated_workspace_version(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_release_workspace(root)
            output = root / "release-notes.md"
            with patch.object(CONTRACTS, "run") as runner:
                CONTRACTS.prepare_release_notes(
                    f"v{FIXTURE_VERSION}", output, repo_root=root
                )

        commands = [tuple(call.args[0]) for call in runner.call_args_list]
        self.assertEqual(
            [command[2] for command in commands], ["check-soft-wrap", "extract"]
        )
        self.assertEqual(commands[0][-2:], ("--version", FIXTURE_VERSION))
        self.assertEqual(
            commands[1][-4:],
            ("--version", FIXTURE_VERSION, "--output", output),
        )

    def test_release_notes_reject_shell_metacharacters_in_tag(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_release_workspace(root)
            with self.assertRaisesRegex(
                CONTRACTS.VerificationError, "invalid release tag"
            ):
                CONTRACTS.prepare_release_notes(
                    f"v{FIXTURE_VERSION};echo",
                    root / "notes",
                    repo_root=root,
                )

    def test_release_notes_require_the_workspace_release_version(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_release_workspace(root)
            with self.assertRaisesRegex(
                CONTRACTS.VerificationError, "workspace release tag"
            ):
                CONTRACTS.prepare_release_notes(
                    "v9.9.9",
                    root / "notes",
                    repo_root=root,
                )

    def test_release_identity_accepts_an_uncreated_tag_on_exact_main_candidate(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_release_workspace(root)
            head = subprocess.CompletedProcess([], 0, stdout=f"{CANDIDATE_SHA}\n")
            missing_tag = subprocess.CompletedProcess([], 1, stdout="")
            with patch.object(CONTRACTS, "run", side_effect=[head, missing_tag]):
                version = CONTRACTS.validate_release_identity(
                    tag=f"v{FIXTURE_VERSION}",
                    candidate_sha=CANDIDATE_SHA,
                    repo_root=root,
                    expected_ref="refs/heads/main",
                    actual_ref="refs/heads/main",
                )

        self.assertEqual(version, FIXTURE_VERSION)

    def test_release_identity_rejects_an_existing_tag_on_another_commit(self):
        results = [
            subprocess.CompletedProcess([], 0, stdout=f"{CANDIDATE_SHA}\n"),
            subprocess.CompletedProcess([], 0, stdout=""),
            subprocess.CompletedProcess([], 0, stdout=f"{'b' * 40}\n"),
        ]
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_release_workspace(root)
            with (
                patch.object(CONTRACTS, "run", side_effect=results),
                self.assertRaisesRegex(CONTRACTS.VerificationError, "points to"),
            ):
                CONTRACTS.validate_release_identity(
                    tag=f"v{FIXTURE_VERSION}",
                    candidate_sha=CANDIDATE_SHA,
                    repo_root=root,
                    expected_ref="refs/heads/main",
                    actual_ref="refs/heads/main",
                )

    def test_release_identity_rejects_non_main_dispatch_before_git(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_release_workspace(root)
            with (
                patch.object(CONTRACTS, "run") as runner,
                self.assertRaisesRegex(CONTRACTS.VerificationError, "refs/heads/main"),
            ):
                CONTRACTS.validate_release_identity(
                    tag=f"v{FIXTURE_VERSION}",
                    candidate_sha=CANDIDATE_SHA,
                    repo_root=root,
                    expected_ref="refs/heads/main",
                    actual_ref="refs/heads/topic",
                )

        runner.assert_not_called()

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
            with (
                patch.dict(os.environ, {"GITHUB_OUTPUT": str(github_output)}),
                patch.object(
                    CONTRACTS,
                    "resolve_candidate_sha",
                    return_value=CANDIDATE_SHA,
                ),
            ):
                exit_code = CONTRACTS.main(
                    (
                        "runtime-preparation-failure",
                        "--gate",
                        "multi-viewport-smoke",
                        "--candidate-sha",
                        CANDIDATE_SHA,
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


if __name__ == "__main__":
    unittest.main()
