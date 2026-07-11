import importlib.util
import io
import json
import subprocess
import sys
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


class PublishCommandTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = release_metadata.load_workspace_metadata(REPO_ROOT)

    def test_search_and_publish_explicitly_target_crates_io(self):
        search_result = subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout='dear-imgui-sys = "0.16.0" # bindings\n',
            stderr="",
        )
        with patch.object(PUBLISH.subprocess, "run", return_value=search_result) as run:
            self.assertTrue(PUBLISH.check_crate_published("dear-imgui-sys", "0.16.0"))
        self.assertEqual(
            run.call_args.args[0],
            [
                "cargo",
                "search",
                "dear-imgui-sys",
                "--limit",
                "1",
                "--registry",
                "crates-io",
            ],
        )

        with (
            patch.object(PUBLISH, "run_command", return_value=0) as run_command,
            redirect_stdout(io.StringIO()),
        ):
            self.assertTrue(
                PUBLISH.publish_crate(
                    "dear-imgui-sys",
                    Path("dear-imgui-sys"),
                    "0.16.0",
                    REPO_ROOT,
                    dry_run=True,
                )
            )
        command = run_command.call_args.args[0]
        self.assertIn("--locked", command)
        self.assertEqual(command[-2:], ["--registry", "crates-io"])

    def test_publish_guard_runs_after_search_before_cargo_publish(self):
        with (
            patch.object(PUBLISH, "check_crate_published", return_value=False),
            patch.object(PUBLISH, "run_command") as run_command,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertFalse(
                PUBLISH.publish_crate(
                    "dear-imgui-sys",
                    Path("dear-imgui-sys"),
                    "0.16.0",
                    REPO_ROOT,
                    source_guard=lambda: False,
                )
            )
        run_command.assert_not_called()

    def test_strict_preflight_failure_runs_no_publish_command(self):
        argv = ["publish.py", "--crates", "dear-imgui-sys", "--wait", "0"]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "run_release_preflight", return_value=37),
            patch.object(PUBLISH, "load_workspace_metadata") as load_metadata,
            patch.object(PUBLISH, "publish_crate") as publish_crate,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 37)

        load_metadata.assert_not_called()
        publish_crate.assert_not_called()

    def test_actual_upload_uses_preflight_and_authoritative_metadata(self):
        argv = ["publish.py", "--crates", "dear-imgui-sys", "--wait", "0"]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
            patch.object(PUBLISH, "run_release_preflight", return_value=0) as preflight,
            patch.object(
                PUBLISH, "load_workspace_metadata", return_value=self.metadata
            ) as load_metadata,
            patch.object(PUBLISH, "publish_crate", return_value=True) as publish_crate,
            patch("builtins.input", return_value="y"),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 0)

        preflight.assert_called_once_with(REPO_ROOT)
        load_metadata.assert_called_once_with(REPO_ROOT)
        publish_crate.assert_called_once()

    def test_cargo_dry_run_is_also_guarded_by_strict_preflight(self):
        argv = [
            "publish.py",
            "--cargo-dry-run",
            "--crates",
            "dear-imgui-sys",
            "--wait",
            "0",
        ]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
            patch.object(PUBLISH, "run_release_preflight", return_value=0) as preflight,
            patch.object(PUBLISH, "load_workspace_metadata", return_value=self.metadata),
            patch.object(PUBLISH, "publish_crate", return_value=True),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 0)

        preflight.assert_called_once_with(REPO_ROOT)

    def test_dangerous_bypass_is_explicit_and_still_source_guarded(self):
        argv = [
            "publish.py",
            "--crates",
            "dear-imgui-sys",
            "--wait",
            "0",
            "--dangerously-skip-release-check",
        ]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "verify_release_fingerprint", return_value=True),
            patch.object(PUBLISH, "run_release_preflight") as preflight,
            patch.object(PUBLISH, "load_workspace_metadata", return_value=self.metadata),
            patch.object(PUBLISH, "publish_crate", return_value=True),
            patch("builtins.input", return_value="y"),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 0)

        preflight.assert_not_called()

    def test_source_change_during_preflight_refuses_to_load_or_publish(self):
        argv = ["publish.py", "--crates", "dear-imgui-sys", "--wait", "0"]
        with (
            patch.object(PUBLISH.sys, "argv", argv),
            patch.object(PUBLISH, "capture_release_fingerprint", return_value="head"),
            patch.object(PUBLISH, "verify_release_fingerprint", return_value=False),
            patch.object(PUBLISH, "run_release_preflight", return_value=0),
            patch.object(PUBLISH, "load_workspace_metadata") as load_metadata,
            patch.object(PUBLISH, "publish_crate") as publish_crate,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(PUBLISH.main(), 1)

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

    def test_publish_check_validates_changelog_structure(self):
        workflow = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        job_start = workflow.index("  publish-check:")
        job_end = workflow.index("\n  fmt:", job_start)
        job = workflow[job_start:job_end]

        self.assertIn(
            "run: python3 -B tools/changelog.py check-unreleased",
            job,
        )


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
            ["bash", "tools/ci/verify_packaged_core.sh"],
            cwd=Path("/repo"),
            capture=True,
            show_output=True,
        )

    def test_package_gate_verifies_every_source_archive(self):
        script = (REPO_ROOT / "tools/ci/verify_packaged_core.sh").read_text(
            encoding="utf-8"
        )

        self.assertNotIn("--no-verify", script)
        self.assertIn("Create every publishable workspace source archive", script)

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
