import importlib.util
import io
import json
import subprocess
import sys
import tarfile
import tomllib
import unittest
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import replace
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock, patch


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
sys.path.insert(0, str(TOOLS_DIR))

import release_metadata  # noqa: E402


def load_tool(name: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS_DIR / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


PREPUBLISH = load_tool("pre_publish_check")
PUBLISH = load_tool("publish")
CHANGELOG = load_tool("changelog")

TEST_PRIVATE_PACKAGES = (
    "dear-imgui-examples",
    "dear-imgui-web-demo",
    "xtask",
)


def package(
    name: str,
    path: str,
    *,
    version: str = "0.16.0",
    publish=True,
    dependencies=(),
):
    return {
        "id": f"path+file:///repo/{path}#{name}@{version}",
        "name": name,
        "version": version,
        "manifest_path": f"/repo/{path}/Cargo.toml",
        "publish": None if publish else [],
        "dependencies": list(dependencies),
    }


def dependency(name: str, path: str, requirement: str, *, kind=None):
    return {
        "name": name,
        "req": requirement,
        "path": f"/repo/{path}",
        "kind": kind,
    }


def metadata_for(packages):
    packages = list(packages)
    private_policy = {
        item["name"]: {
            "path": Path(item["manifest_path"])
            .parent.relative_to("/repo")
            .as_posix(),
            "version": item["version"],
        }
        for item in packages
        if item.get("publish") == []
    }
    if not private_policy:
        private_policy = {
            "fixture-private": {"path": "private/fixture", "version": "0.1.0"}
        }
    return release_metadata.WorkspaceMetadata.from_json(
        {
            "workspace_root": "/repo",
            "workspace_members": [item["id"] for item in packages],
            "packages": packages,
            "metadata": {
                "dear-imgui-release": {
                    "core-package": "dear-imgui-rs",
                    "private-packages": private_policy,
                }
            },
        }
    )


def complete_release_metadata(
    *,
    version="0.16.0",
    sys_version=None,
    sys_requirement=None,
    private_versions=None,
):
    sys_version = sys_version or version
    sys_requirement = sys_requirement or release_metadata.expected_internal_requirement(
        sys_version
    )
    packages = [
        package(
            "dear-imgui-rs",
            "dear-imgui",
            version=version,
            dependencies=[
                dependency("dear-imgui-sys", "dear-imgui-sys", sys_requirement)
            ],
        ),
        package("dear-imgui-sys", "dear-imgui-sys", version=sys_version),
    ]
    packages.extend(
        package(f"release-{index}", f"release/{index}", version=version)
        for index in range(25)
    )
    private_versions = private_versions or {}
    packages.extend(
        package(
            name,
            f"private/{name}",
            version=private_versions.get(name, "0.1.0"),
            publish=False,
        )
        for name in TEST_PRIVATE_PACKAGES
    )
    metadata = metadata_for(packages)
    policy = replace(
        metadata.release_policy,
        private_packages=tuple(
            replace(package, version="0.1.0")
            for package in metadata.release_policy.private_packages
        ),
    )
    return replace(metadata, release_policy=policy)


class MetadataTests(unittest.TestCase):
    def test_classifies_27_publishable_and_3_private_packages(self):
        packages = [package("dear-imgui-rs", "dear-imgui")]
        packages.extend(
            package(f"release-{index}", f"release/{index}") for index in range(26)
        )
        packages.extend(
            package(f"private-{index}", f"private/{index}", publish=False)
            for index in range(3)
        )

        metadata = metadata_for(packages)

        self.assertEqual(len(metadata.publishable_packages), 27)
        self.assertEqual(len(metadata.private_packages), 3)
        self.assertEqual(metadata.release_version, "0.16.0")

    def test_loads_locked_cargo_metadata_once(self):
        payload = {
            "workspace_root": "/repo",
            "workspace_members": ["core-id"],
            "packages": [
                {
                    "id": "core-id",
                    "name": "dear-imgui-rs",
                    "version": "0.16.0",
                    "manifest_path": "/repo/dear-imgui/Cargo.toml",
                    "publish": None,
                    "dependencies": [],
                }
            ],
            "metadata": {
                "dear-imgui-release": {
                    "core-package": "dear-imgui-rs",
                    "private-packages": {
                        "fixture-private": {
                            "path": "private/fixture",
                            "version": "0.1.0",
                        }
                    },
                }
            },
        }
        runner = Mock(
            return_value=subprocess.CompletedProcess(
                args=[], returncode=0, stdout=json.dumps(payload), stderr=""
            )
        )

        metadata = release_metadata.load_workspace_metadata(
            Path("/repo"), runner=runner
        )

        self.assertEqual(metadata.release_version, "0.16.0")
        runner.assert_called_once_with(
            list(release_metadata.METADATA_COMMAND),
            cwd=Path("/repo"),
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )

    def test_validates_stable_internal_requirement(self):
        metadata = complete_release_metadata(sys_requirement="^0.15")

        errors = release_metadata.validate_release_workspace(metadata)

        self.assertTrue(any("expected ^0.16" in error for error in errors), errors)

    def test_rejects_publishable_version_drift(self):
        metadata = complete_release_metadata(sys_version="0.16.1")

        errors = release_metadata.validate_release_workspace(metadata)

        self.assertTrue(
            any("dear-imgui-sys uses 0.16.1" in error for error in errors), errors
        )

    def test_requires_exact_internal_requirement_for_prerelease(self):
        version = "0.17.0-alpha.1"
        metadata = complete_release_metadata(
            version=version,
            sys_requirement=f"={version}",
        )

        self.assertEqual(release_metadata.validate_release_workspace(metadata), [])

        loose = complete_release_metadata(
            version=version,
            sys_requirement="^0.17",
        )
        errors = release_metadata.validate_release_workspace(loose)
        self.assertTrue(any(f"expected ={version}" in error for error in errors), errors)

    def test_rejects_release_versions_with_build_metadata(self):
        version = "0.17.0+build.1"
        metadata = complete_release_metadata(
            version=version,
            sys_requirement=f"={version}",
        )

        errors = release_metadata.validate_release_workspace(metadata)

        self.assertTrue(any("cannot contain build metadata" in error for error in errors))

    def test_publishable_package_count_is_derived_from_workspace_members(self):
        metadata = complete_release_metadata()
        metadata = replace(
            metadata,
            packages=metadata.packages[:-4] + metadata.packages[-3:],
        )

        errors = release_metadata.validate_release_workspace(metadata)

        self.assertEqual(len(metadata.publishable_packages), 26)
        self.assertFalse(any("publishable packages" in error for error in errors), errors)

    def test_rejects_private_package_identity_and_version_drift(self):
        metadata = complete_release_metadata(
            private_versions={"xtask": "0.2.0"}
        )
        packages = tuple(
            package
            for package in metadata.packages
            if package.name != "dear-imgui-web-demo"
        ) + (
            release_metadata.WorkspacePackage(
                package_id="private-extra",
                name="private-extra",
                version="0.1.0",
                manifest_path=Path("/repo/private/private-extra/Cargo.toml"),
                publish_registries=(),
                dependencies=(),
            ),
        )

        errors = release_metadata.validate_release_workspace(
            replace(metadata, packages=packages)
        )

        self.assertTrue(any("missing: dear-imgui-web-demo" in error for error in errors))
        self.assertTrue(any("unexpected private" in error for error in errors))
        self.assertTrue(any("xtask uses 0.2.0" in error for error in errors))

    def test_rejects_non_crates_io_registry_policy(self):
        metadata = complete_release_metadata()
        core = metadata.package("dear-imgui-rs")
        packages = tuple(
            replace(package, publish_registries=("company",))
            if package.name == core.name
            else package
            for package in metadata.packages
        )

        errors = release_metadata.validate_release_workspace(
            replace(metadata, packages=packages)
        )

        self.assertTrue(any("only targets crates.io" in error for error in errors))

    def test_rejects_internal_dependency_without_local_path(self):
        metadata = complete_release_metadata(sys_requirement="^0.15")
        core = metadata.package("dear-imgui-rs")
        dependency_without_path = replace(core.dependencies[0], path=None)
        packages = tuple(
            replace(package, dependencies=(dependency_without_path,))
            if package.name == core.name
            else package
            for package in metadata.packages
        )

        errors = release_metadata.validate_release_workspace(
            replace(metadata, packages=packages)
        )

        self.assertTrue(any("must use the local workspace path" in error for error in errors))
        self.assertTrue(any("expected ^0.16" in error for error in errors))

    def test_current_workspace_versions_and_internal_requirements_are_valid(self):
        metadata = release_metadata.load_workspace_metadata(REPO_ROOT)

        self.assertEqual(len(metadata.publishable_packages), 27)
        self.assertEqual(len(metadata.private_packages), 3)
        self.assertEqual(release_metadata.validate_release_workspace(metadata), [])


class CurrentReleaseTrainTests(unittest.TestCase):
    def test_alpha_release_train_uses_exact_catalog_requirements(self):
        workspace = tomllib.loads(
            REPO_ROOT.joinpath("Cargo.toml").read_text(encoding="utf-8")
        )
        version = workspace["workspace"]["package"]["version"]
        catalog = workspace["workspace"]["dependencies"]

        self.assertEqual(version, "0.16.0-alpha.2")
        self.assertEqual(
            {
                dependency["version"]
                for dependency in catalog.values()
                if "path" in dependency
            },
            {f"={version}"},
        )

        metadata = release_metadata.load_workspace_metadata(REPO_ROOT)
        self.assertEqual(metadata.release_version, version)
        self.assertEqual(release_metadata.validate_release_workspace(metadata), [])

    def test_standalone_mobile_locks_use_the_alpha_path_packages(self):
        lockfiles = (
            "examples-android/dear-imgui-android-smoke/Cargo.lock",
            "examples-ios/dear-imgui-ios-smoke/Cargo.lock",
            "examples-ios/dear-imgui-ios-sdl3-smoke/Cargo.lock",
        )
        for relative_path in lockfiles:
            with self.subTest(lockfile=relative_path):
                lock = tomllib.loads(
                    REPO_ROOT.joinpath(relative_path).read_text(encoding="utf-8")
                )
                path_packages = [
                    package
                    for package in lock["package"]
                    if package["name"].startswith("dear-imgui")
                    and not package["name"].endswith("-smoke")
                ]
                self.assertTrue(path_packages)
                self.assertEqual(
                    {package["version"] for package in path_packages},
                    {"0.16.0-alpha.2"},
                )


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


class ChangelogTests(unittest.TestCase):
    def test_accepts_unreleased_as_first_release_section(self):
        with TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            changelog.write_text(
                "# Changelog\n\n## [Unreleased]\n\n## [0.16.0]\n\n- Notes.\n",
                encoding="utf-8",
            )

            CHANGELOG.validate_unreleased_first(changelog)

    def test_rejects_version_before_unreleased(self):
        with TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            changelog.write_text(
                "# Changelog\n\n## [0.16.0]\n\n- Notes.\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "must keep .*Unreleased"):
                CHANGELOG.validate_unreleased_first(changelog)

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

    def test_strict_release_contract_commands_are_deterministic(self):
        self.assertEqual(
            PREPUBLISH.release_contract_commands(),
            [
                (
                    "Python contract suite",
                    [
                        sys.executable,
                        "-B",
                        "-m",
                        "unittest",
                        "discover",
                        "-s",
                        "tools/tests",
                        "-p",
                        "test_*.py",
                    ],
                ),
                (
                    "Workflow policy",
                    [sys.executable, "tools/ci/workflow_policy.py", "--check"],
                ),
                (
                    "WASM core and high-level extensions",
                    [
                        "cargo",
                        "check",
                        "--target",
                        "wasm32-unknown-unknown",
                        "--no-default-features",
                        "-p",
                        "dear-imgui-rs",
                        "-p",
                        "dear-imgui-glow",
                        "-p",
                        "dear-implot",
                        "-p",
                        "dear-implot3d",
                        "-p",
                        "dear-imnodes",
                        "-p",
                        "dear-imguizmo",
                        "-p",
                        "dear-imguizmo-quat",
                        "--features",
                        "dear-imgui-rs/wasm,dear-imgui-glow/wasm,dear-implot/wasm,"
                        "dear-implot3d/wasm,dear-imnodes/wasm,"
                        "dear-imguizmo/wasm,dear-imguizmo-quat/wasm",
                    ],
                ),
            ],
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
        self.assertEqual(
            run.call_args_list[-1].args[0],
            [sys.executable, "tools/ci/workflow_policy.py", "--check"],
        )

    def test_release_contract_gate_runs_every_owned_command_in_order(self):
        expected = PREPUBLISH.release_contract_commands()
        with (
            patch.object(
                PREPUBLISH,
                "run_command",
                return_value=(0, "", ""),
            ) as run,
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(
                PREPUBLISH.check_release_contracts(Path("/repo")),
                (True, []),
            )

        self.assertEqual(
            [call.args[0] for call in run.call_args_list],
            [command for _label, command in expected],
        )
        for call in run.call_args_list:
            self.assertEqual(
                call.kwargs,
                {"cwd": Path("/repo"), "capture": False},
            )

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
