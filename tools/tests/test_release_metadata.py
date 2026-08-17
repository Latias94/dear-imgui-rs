import json
import subprocess
import tomllib
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import Mock

from tools.tests.release_test_support import (
    REPO_ROOT,
    complete_release_metadata,
    metadata_for,
    package,
    release_metadata,
)


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

        self.assertEqual(len(metadata.publishable_packages), 29)
        self.assertEqual(len(metadata.private_packages), 3)
        self.assertEqual(release_metadata.validate_release_workspace(metadata), [])


class CurrentReleaseTrainTests(unittest.TestCase):
    def test_release_train_uses_canonical_catalog_requirements(self):
        workspace = tomllib.loads(
            REPO_ROOT.joinpath("Cargo.toml").read_text(encoding="utf-8")
        )
        version = workspace["workspace"]["package"]["version"]
        catalog = workspace["workspace"]["dependencies"]
        expected_requirement = release_metadata.expected_internal_requirement(
            version
        ).removeprefix("^")

        self.assertEqual(
            {
                dependency["version"]
                for dependency in catalog.values()
                if "path" in dependency
            },
            {expected_requirement},
        )

        metadata = release_metadata.load_workspace_metadata(REPO_ROOT)
        self.assertEqual(metadata.release_version, version)
        self.assertEqual(release_metadata.validate_release_workspace(metadata), [])

    def test_standalone_mobile_locks_use_workspace_path_package_versions(self):
        workspace = tomllib.loads(
            REPO_ROOT.joinpath("Cargo.toml").read_text(encoding="utf-8")
        )
        version = workspace["workspace"]["package"]["version"]
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
                    {version},
                )


if __name__ == "__main__":
    unittest.main()
