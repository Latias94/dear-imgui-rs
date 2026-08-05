import io
import json
import subprocess
import tarfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import replace
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock, patch

from tools.tests.release_test_support import (
    REPO_ROOT,
    load_tool,
    package,
    release_metadata,
)


PREPUBLISH = load_tool("pre_publish_check")
PUBLISH = load_tool("publish")


class PublishConfigurationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = release_metadata.load_workspace_metadata(REPO_ROOT)

    def test_publish_order_exactly_covers_current_workspace(self):
        self.assertEqual(
            release_metadata.validate_publish_order(
                self.metadata, PUBLISH.PUBLISH_ORDER, REPO_ROOT
            ),
            [],
        )

    def test_prepublish_checks_the_exact_shared_publish_order(self):
        with (
            patch.object(PREPUBLISH, "PUBLISH_ORDER", PUBLISH.PUBLISH_ORDER[:-1]),
            redirect_stdout(io.StringIO()),
        ):
            success, errors = PREPUBLISH.check_version_consistency(
                REPO_ROOT, self.metadata
            )

        self.assertFalse(success)
        self.assertTrue(any("PUBLISH_ORDER is missing" in error for error in errors))

    def test_publish_configuration_rejects_order_drift(self):
        with patch.object(PUBLISH, "PUBLISH_ORDER", PUBLISH.PUBLISH_ORDER[:-1]):
            errors = PUBLISH.validate_release_configuration(self.metadata, REPO_ROOT)

        self.assertTrue(any("PUBLISH_ORDER is missing" in error for error in errors))

    def test_publish_configuration_rejects_duplicates(self):
        duplicate = [*PUBLISH.PUBLISH_ORDER, PUBLISH.PUBLISH_ORDER[0]]

        errors = release_metadata.validate_publish_order(
            self.metadata, duplicate, REPO_ROOT
        )

        self.assertTrue(any("repeats package" in error for error in errors), errors)

    def test_publish_configuration_requires_dependencies_first(self):
        reversed_core = [
            PUBLISH.PUBLISH_ORDER[1],
            PUBLISH.PUBLISH_ORDER[0],
            *PUBLISH.PUBLISH_ORDER[2:],
        ]

        errors = release_metadata.validate_publish_order(
            self.metadata, reversed_core, REPO_ROOT
        )

        self.assertTrue(
            any(
                "dear-imgui-sys before internal dependency dear-imgui-build-support"
                in error
                for error in errors
            ),
            errors,
        )


class RegistryPublicationStateTests(unittest.TestCase):
    @staticmethod
    def crate_archive(
        crate_name: str,
        version: str,
        candidate_sha: str,
        *,
        dirty: bool = False,
        extra_members: int = 0,
    ) -> bytes:
        vcs_info = json.dumps(
            {
                "git": {"sha1": candidate_sha, "dirty": dirty},
                "path_in_vcs": "crates/example",
            }
        ).encode("utf-8")
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w:gz") as archive:
            for index in range(extra_members):
                member = tarfile.TarInfo(f"dummy-{index}")
                member.size = 0
                archive.addfile(member, io.BytesIO())
            member = tarfile.TarInfo(
                f"{crate_name}-{version}/.cargo_vcs_info.json"
            )
            member.size = len(vcs_info)
            archive.addfile(member, io.BytesIO(vcs_info))
        return output.getvalue()

    def test_exact_version_query_distinguishes_present_absent_and_unavailable(self):
        response = Mock()
        response.status = 200
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)
        missing = PUBLISH.HTTPError(
            "https://crates.io/api/v1/crates/example/1.2.3",
            404,
            "not found",
            {},
            None,
        )

        with patch.object(PUBLISH, "urlopen", return_value=response):
            self.assertEqual(
                PUBLISH.query_crate_version("example", "1.2.3"),
                PUBLISH.RegistryState.PRESENT,
            )
        with patch.object(PUBLISH, "urlopen", side_effect=missing):
            self.assertEqual(
                PUBLISH.query_crate_version("example", "1.2.3"),
                PUBLISH.RegistryState.ABSENT,
            )
        with patch.object(PUBLISH, "urlopen", side_effect=OSError("offline")):
            self.assertEqual(
                PUBLISH.query_crate_version("example", "1.2.3"),
                PUBLISH.RegistryState.UNAVAILABLE,
            )

    def test_registry_state_retry_is_bounded_and_preserves_unavailability(self):
        with (
            patch.object(
                PUBLISH,
                "query_crate_version",
                side_effect=(
                    PUBLISH.RegistryState.UNAVAILABLE,
                    PUBLISH.RegistryState.UNAVAILABLE,
                    PUBLISH.RegistryState.PRESENT,
                ),
            ) as query,
            patch.object(PUBLISH.time, "sleep") as sleep,
        ):
            state = PUBLISH.resolve_registry_state(
                "example", "1.2.3", attempts=3, retry_delay=0.5
            )

        self.assertIs(state, PUBLISH.RegistryState.PRESENT)
        self.assertEqual(query.call_count, 3)
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [0.5, 1.0])

        with (
            patch.object(
                PUBLISH,
                "query_crate_version",
                return_value=PUBLISH.RegistryState.UNAVAILABLE,
            ) as query,
            patch.object(PUBLISH.time, "sleep") as sleep,
        ):
            state = PUBLISH.resolve_registry_state(
                "example", "1.2.3", attempts=2, retry_delay=0
            )

        self.assertIs(state, PUBLISH.RegistryState.UNAVAILABLE)
        self.assertEqual(query.call_count, 2)
        sleep.assert_not_called()

    def test_cargo_index_lookup_is_bounded(self):
        with patch.object(
            PUBLISH.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["cargo", "info"], 30),
        ):
            self.assertFalse(
                PUBLISH.crate_version_is_indexed("example", "1.2.3", timeout=30)
            )

    def test_published_archive_exposes_clean_candidate_sha(self):
        candidate_sha = "a" * 40
        response = Mock()
        response.read.return_value = self.crate_archive(
            "example", "1.2.3", candidate_sha
        )
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)

        with patch.object(PUBLISH, "urlopen", return_value=response) as urlopen:
            self.assertEqual(
                PUBLISH.query_crate_candidate_sha("example", "1.2.3"),
                candidate_sha,
            )

        self.assertTrue(urlopen.call_args.args[0].full_url.endswith("/download"))

    def test_published_archive_rejects_dirty_sources(self):
        response = Mock()
        response.read.return_value = self.crate_archive(
            "example", "1.2.3", "a" * 40, dirty=True
        )
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)

        with (
            patch.object(PUBLISH, "urlopen", return_value=response),
            self.assertRaisesRegex(PUBLISH.RegistryProvenanceError, "dirty sources"),
        ):
            PUBLISH.query_crate_candidate_sha("example", "1.2.3")

    def test_incomplete_published_archive_response_is_retryable(self):
        with patch.object(
            PUBLISH,
            "urlopen",
            side_effect=PUBLISH.IncompleteRead(b"partial", 10),
        ):
            self.assertIsNone(
                PUBLISH.query_crate_candidate_sha("example", "1.2.3")
            )

    def test_published_archive_enforces_unpacked_size_limit(self):
        response = Mock()
        response.read.return_value = self.crate_archive(
            "example", "1.2.3", "a" * 40
        )
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)

        with (
            patch.object(PUBLISH, "urlopen", return_value=response),
            patch.object(PUBLISH, "MAX_CRATE_UNPACKED_BYTES", 32),
            self.assertRaisesRegex(PUBLISH.RegistryProvenanceError, "unpacked"),
        ):
            PUBLISH.query_crate_candidate_sha("example", "1.2.3")

    def test_published_archive_enforces_compressed_size_limit(self):
        response = Mock()
        response.read.return_value = b"oversized"
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)

        with (
            patch.object(PUBLISH, "urlopen", return_value=response),
            patch.object(PUBLISH, "MAX_CRATE_ARCHIVE_BYTES", 4),
            self.assertRaisesRegex(PUBLISH.RegistryProvenanceError, "safety limit"),
        ):
            PUBLISH.query_crate_candidate_sha("example", "1.2.3")

    def test_published_archive_enforces_vcs_metadata_size_limit(self):
        response = Mock()
        response.read.return_value = self.crate_archive(
            "example", "1.2.3", "a" * 40
        )
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)

        with (
            patch.object(PUBLISH, "urlopen", return_value=response),
            patch.object(PUBLISH, "MAX_VCS_INFO_BYTES", 8),
            self.assertRaisesRegex(PUBLISH.RegistryProvenanceError, "VCS metadata"),
        ):
            PUBLISH.query_crate_candidate_sha("example", "1.2.3")

    def test_published_archive_enforces_member_limit(self):
        response = Mock()
        response.read.return_value = self.crate_archive(
            "example", "1.2.3", "a" * 40, extra_members=1
        )
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)

        with (
            patch.object(PUBLISH, "urlopen", return_value=response),
            patch.object(PUBLISH, "MAX_CRATE_MEMBERS", 1),
            self.assertRaisesRegex(PUBLISH.RegistryProvenanceError, "member"),
        ):
            PUBLISH.query_crate_candidate_sha("example", "1.2.3")

    def test_candidate_mismatch_fails_without_retrying(self):
        with (
            patch.object(
                PUBLISH,
                "query_crate_version",
                return_value=PUBLISH.RegistryState.PRESENT,
            ),
            patch.object(PUBLISH, "crate_version_is_indexed", return_value=True),
            patch.object(PUBLISH, "query_crate_candidate_sha", return_value="b" * 40),
            patch.object(PUBLISH.time, "sleep") as sleep,
            self.assertRaisesRegex(
                PUBLISH.RegistryProvenanceError,
                "not release candidate",
            ),
        ):
            PUBLISH.wait_for_crate_available(
                "example",
                "1.2.3",
                expected_candidate_sha="a" * 40,
                timeout=10,
            )

        sleep.assert_not_called()

    def test_already_published_exact_version_is_an_idempotent_success(self):
        with (
            patch.object(
                PUBLISH,
                "resolve_registry_state",
                return_value=PUBLISH.RegistryState.PRESENT,
            ),
            patch.object(
                PUBLISH, "wait_for_crate_available", return_value=True
            ) as wait,
            patch.object(PUBLISH, "run_command") as run_command,
            redirect_stdout(io.StringIO()),
        ):
            status = PUBLISH.publish_crate(
                "dear-imgui-sys",
                Path("dear-imgui-sys"),
                "0.16.0-alpha.1",
                REPO_ROOT,
                candidate_sha="a" * 40,
            )

        self.assertEqual(status, PUBLISH.PublicationStatus.ALREADY_PUBLISHED)
        run_command.assert_not_called()
        self.assertEqual(
            wait.call_args.kwargs["expected_candidate_sha"],
            "a" * 40,
        )

    def test_already_published_different_candidate_stops_before_cargo(self):
        with (
            patch.object(
                PUBLISH,
                "resolve_registry_state",
                return_value=PUBLISH.RegistryState.PRESENT,
            ),
            patch.object(
                PUBLISH,
                "wait_for_crate_available",
                side_effect=PUBLISH.RegistryProvenanceError(
                    "published crate came from another release candidate"
                ),
            ),
            patch.object(PUBLISH, "run_command") as run_command,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            status = PUBLISH.publish_crate(
                "dear-imgui-sys",
                Path("dear-imgui-sys"),
                "0.16.0-alpha.1",
                REPO_ROOT,
                candidate_sha="b" * 40,
            )

        self.assertIsNone(status)
        run_command.assert_not_called()

    def test_publish_timeout_is_reconciled_against_registry_state(self):
        with (
            patch.object(
                PUBLISH,
                "resolve_registry_state",
                return_value=PUBLISH.RegistryState.ABSENT,
            ),
            patch.object(PUBLISH, "run_command", return_value=101) as run_command,
            patch.object(PUBLISH, "wait_for_crate_available", return_value=True),
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            status = PUBLISH.publish_crate(
                "dear-imgui-sys",
                Path("dear-imgui-sys"),
                "0.16.0-alpha.1",
                REPO_ROOT,
                no_verify=True,
                candidate_sha="a" * 40,
                publish_timeout=17,
            )

        self.assertEqual(status, PUBLISH.PublicationStatus.PUBLISHED)
        command = run_command.call_args.args[0]
        self.assertIn("--no-verify", command)
        registry = command.index("--registry")
        self.assertEqual(command[registry : registry + 2], ["--registry", "crates-io"])
        self.assertEqual(run_command.call_args.kwargs["timeout"], 17)

    def test_ci_upload_is_noninteractive_and_does_not_repeat_local_preflight(self):
        argv = [
            "publish.py",
            "--yes",
            "--no-verify",
        ]
        metadata = release_metadata.load_workspace_metadata(REPO_ROOT)
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
            patch.object(PUBLISH, "run_release_preflight") as preflight,
            patch.object(PUBLISH, "load_workspace_metadata", return_value=metadata),
            patch.object(
                PUBLISH,
                "publish_crate",
                return_value=PUBLISH.PublicationStatus.PUBLISHED,
            ) as publish_crate,
            patch("builtins.input", side_effect=AssertionError("stdin was read")),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 0)

        preflight.assert_not_called()
        self.assertEqual(publish_crate.call_count, len(PUBLISH.PUBLISH_ORDER))


class PublishCommandTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = release_metadata.load_workspace_metadata(REPO_ROOT)

    def test_preview_publish_explicitly_targets_crates_io(self):
        with (
            patch.object(PUBLISH, "run_command", return_value=0) as run_command,
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(
                PUBLISH.publish_crate(
                    "dear-imgui-sys",
                    Path("dear-imgui-sys"),
                    "0.16.0",
                    REPO_ROOT,
                    dry_run=True,
                ),
                PUBLISH.PublicationStatus.PREVIEWED,
            )
        command = run_command.call_args.args[0]
        self.assertIn("--locked", command)
        registry = command.index("--registry")
        self.assertEqual(command[registry : registry + 2], ["--registry", "crates-io"])

    def test_command_timeout_returns_stable_exit_code(self):
        with (
            patch.object(
                PUBLISH.subprocess,
                "run",
                side_effect=subprocess.TimeoutExpired(["cargo", "publish"], 1),
            ),
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(
                PUBLISH.run_command(["cargo", "publish"], timeout=1),
                124,
            )

    def test_publish_guard_runs_before_cargo_publish(self):
        with (
            patch.object(PUBLISH, "run_command") as run_command,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertIsNone(
                PUBLISH.publish_crate(
                    "dear-imgui-sys",
                    Path("dear-imgui-sys"),
                    "0.16.0",
                    REPO_ROOT,
                    cargo_dry_run=True,
                    source_guard=lambda: False,
                )
            )
        run_command.assert_not_called()

    def test_actual_upload_rejects_partial_release_train(self):
        argv = [
            "publish.py",
            "--crates",
            "dear-imgui-sys",
        ]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint") as fingerprint,
            patch.object(PUBLISH, "load_workspace_metadata") as load_metadata,
            patch.object(PUBLISH, "publish_crate") as publish_crate,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 1)

        fingerprint.assert_not_called()
        load_metadata.assert_not_called()
        publish_crate.assert_not_called()

    def test_verify_published_records_the_complete_release_train(self):
        with TemporaryDirectory() as directory:
            journal = Path(directory) / "publication.json"
            argv = [
                "publish.py",
                "--verify-published",
                "--journal",
                str(journal),
            ]
            with (
                patch.object(PUBLISH.sys, "argv", argv),
                patch.object(
                    PUBLISH,
                    "capture_release_fingerprint",
                    return_value="a" * 40,
                ),
                patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
                patch.object(
                    PUBLISH, "load_workspace_metadata", return_value=self.metadata
                ),
                patch.object(
                    PUBLISH, "wait_for_crate_available", return_value=True
                ) as wait,
                redirect_stdout(io.StringIO()),
            ):
                self.assertEqual(PUBLISH.main(), 0)

            payload = json.loads(journal.read_text(encoding="utf-8"))
            self.assertTrue(payload["complete"])
            self.assertEqual(wait.call_count, len(PUBLISH.PUBLISH_ORDER))
            self.assertEqual(
                wait.call_args.kwargs["expected_candidate_sha"],
                "a" * 40,
            )
            self.assertEqual(
                {package["status"] for package in payload["packages"]},
                {"already-published"},
            )

    def test_verify_published_stops_at_the_first_missing_version(self):
        with TemporaryDirectory() as directory:
            journal = Path(directory) / "publication.json"
            argv = [
                "publish.py",
                "--verify-published",
                "--journal",
                str(journal),
            ]
            with (
                patch.object(PUBLISH.sys, "argv", argv),
                patch.object(
                    PUBLISH,
                    "capture_release_fingerprint",
                    return_value="a" * 40,
                ),
                patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
                patch.object(
                    PUBLISH, "load_workspace_metadata", return_value=self.metadata
                ),
                patch.object(
                    PUBLISH, "wait_for_crate_available", return_value=False
                ) as wait,
                redirect_stdout(io.StringIO()),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(PUBLISH.main(), 1)

            payload = json.loads(journal.read_text(encoding="utf-8"))
            self.assertFalse(payload["complete"])
            self.assertEqual(wait.call_count, 1)
            self.assertEqual(payload["packages"][0]["status"], "failed")
            self.assertEqual(
                {package["status"] for package in payload["packages"][1:]},
                {"pending"},
            )

    def test_cargo_dry_run_is_also_guarded_by_strict_preflight(self):
        argv = [
            "publish.py",
            "--cargo-dry-run",
            "--crates",
            "dear-imgui-sys",
        ]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
            patch.object(PUBLISH, "run_release_preflight", return_value=0) as preflight,
            patch.object(PUBLISH, "load_workspace_metadata", return_value=self.metadata),
            patch.object(
                PUBLISH,
                "publish_crate",
                return_value=PUBLISH.PublicationStatus.VERIFIED,
            ),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 0)

        preflight.assert_called_once_with(REPO_ROOT)

    def test_source_change_during_preflight_refuses_to_load_or_publish(self):
        argv = [
            "publish.py",
            "--cargo-dry-run",
            "--crates",
            "dear-imgui-sys",
        ]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(
                PUBLISH,
                "verify_release_fingerprint",
                return_value=False,
            ) as fingerprint,
            patch.object(
                PUBLISH, "run_release_preflight", return_value=0
            ) as preflight,
            patch.object(PUBLISH, "load_workspace_metadata") as load_metadata,
            patch.object(PUBLISH, "publish_crate") as publish_crate,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 1)

        preflight.assert_called_once_with(REPO_ROOT)
        fingerprint.assert_called_once_with(REPO_ROOT, "head")
        load_metadata.assert_not_called()
        publish_crate.assert_not_called()

    def test_dependency_order_does_not_ignore_internal_registry_edges(self):
        core = self.metadata.package("dear-imgui-rs")
        dependencies = tuple(
            replace(dependency, path=None)
            if dependency.name == "dear-imgui-sys"
            else dependency
            for dependency in core.dependencies
        )
        metadata = replace(
            self.metadata,
            packages=tuple(
                replace(package, dependencies=dependencies)
                if package.name == core.name
                else package
                for package in self.metadata.packages
            ),
        )
        order = list(PUBLISH.PUBLISH_ORDER)
        sys_index = next(
            index for index, (name, _path) in enumerate(order)
            if name == "dear-imgui-sys"
        )
        core_index = next(
            index for index, (name, _path) in enumerate(order)
            if name == "dear-imgui-rs"
        )
        order[sys_index], order[core_index] = order[core_index], order[sys_index]

        errors = release_metadata.validate_publish_order(metadata, order, REPO_ROOT)

        self.assertTrue(
            any(
                "dear-imgui-rs before internal dependency dear-imgui-sys" in error
                for error in errors
            ),
            errors,
        )


if __name__ == "__main__":
    unittest.main()
