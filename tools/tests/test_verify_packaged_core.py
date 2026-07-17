import importlib
import io
import os
import subprocess
import sys
import tarfile
import tomllib
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
TOOLS_DIR = REPO_ROOT / "tools"
for import_path in (CI_DIR, TOOLS_DIR):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))
ARCHIVE = importlib.import_module("_archive")
PREBUILT = importlib.import_module("_prebuilt")
PROCESS = importlib.import_module("_process")
SOURCE_PACKAGES = importlib.import_module("_source_packages")
VERIFICATION = importlib.import_module("_verification")
CLI = importlib.import_module("verify_packaged_core")


PROFILE_FEATURES = {
    "normal": "platform-io-aggregate-hooks,wchar32",
    "freetype": "platform-io-aggregate-hooks,freetype,wchar32",
    "stack-layout": "platform-io-aggregate-hooks,stack-layout,wchar32",
    "stack-layout-freetype": (
        "platform-io-aggregate-hooks,stack-layout,freetype,wchar32"
    ),
}


def write_archive(path: Path, members: dict[str, bytes]) -> None:
    with tarfile.open(path, "w:gz") as archive:
        for name, content in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(content)
            archive.addfile(info, io.BytesIO(content))


def write_special_archive(
    path: Path, name: str, member_type: bytes, *, linkname: str = ""
) -> None:
    with tarfile.open(path, "w:gz") as archive:
        info = tarfile.TarInfo(name)
        info.type = member_type
        info.linkname = linkname
        archive.addfile(info)


def write_prebuilt_archive(
    directory: Path,
    profile: str,
    *,
    target: str = "x86_64-unknown-linux-gnu",
    crt: str = "",
    suffix: str = "",
) -> Path:
    path = directory / f"dear-imgui-{profile}{suffix}.tar.gz"
    manifest = "\n".join(
        (
            "dear-imgui artifact",
            f"target={target}",
            f"crt={crt}",
            f"features={PROFILE_FEATURES[profile]}",
            "",
        )
    ).encode()
    write_archive(path, {"manifest.txt": manifest, "lib/.keep": b""})
    return path


class PrebuiltArchiveSelectionTests(unittest.TestCase):
    def test_selects_each_required_profile_exactly_once(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            expected = {
                profile: write_prebuilt_archive(package_dir, profile)
                for profile in PROFILE_FEATURES
            }

            selected = PREBUILT.select_core_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "",
                profile_scope="all",
            )

        self.assertEqual(selected, expected)

    def test_filters_target_and_crt_before_enforcing_uniqueness(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            expected = {}
            for profile in ("normal", "stack-layout"):
                expected[profile] = write_prebuilt_archive(
                    package_dir, profile, crt="static"
                )
                write_prebuilt_archive(
                    package_dir,
                    profile,
                    target="aarch64-apple-darwin",
                    crt="static",
                    suffix="-other-target",
                )
                write_prebuilt_archive(
                    package_dir,
                    profile,
                    crt="dynamic",
                    suffix="-other-crt",
                )

            selected = PREBUILT.select_core_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "static",
                profile_scope="base",
            )

        self.assertEqual(selected, expected)

    def test_rejects_an_unknown_matching_profile(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            for profile in ("normal", "stack-layout"):
                write_prebuilt_archive(package_dir, profile)
            write_archive(
                package_dir / "dear-imgui-unknown.tar.gz",
                {
                    "manifest.txt": (
                        b"dear-imgui artifact\n"
                        b"target=x86_64-unknown-linux-gnu\n"
                        b"crt=\nfeatures=unknown-feature\n"
                    )
                },
            )

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "unsupported dear_imgui artifact profile",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )

    def test_rejects_an_archive_without_a_root_manifest(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            write_archive(
                package_dir / "dear-imgui-missing-manifest.tar.gz",
                {"nested/manifest.txt": b"not at root"},
            )

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "must contain exactly one root manifest.txt",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )

    def test_rejects_duplicate_required_profile(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            write_prebuilt_archive(package_dir, "normal")
            write_prebuilt_archive(package_dir, "normal", suffix="-duplicate")
            write_prebuilt_archive(package_dir, "stack-layout")

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "expected exactly one normal archive",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )


class ArchiveSafetyTests(unittest.TestCase):
    def test_safe_extract_rejects_parent_directory_traversal(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "malicious.tar.gz"
            destination = root / "output"
            write_archive(archive, {"../escaped.txt": b"no"})

            with self.assertRaisesRegex(
                ARCHIVE.VerificationError, "unsafe archive member"
            ):
                ARCHIVE.safe_extract_tar(archive, destination)

            self.assertFalse((root / "escaped.txt").exists())

    def test_safe_extract_rejects_absolute_drive_and_backslash_paths(self):
        unsafe_names = (
            "/absolute.txt",
            "C:/drive-qualified.txt",
            "C:\\backslash-drive.txt",
            "nested\\backslash.txt",
        )
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, unsafe_name in enumerate(unsafe_names):
                with self.subTest(name=unsafe_name):
                    archive = root / f"malicious-{index}.tar.gz"
                    write_archive(archive, {unsafe_name: b"no"})
                    with self.assertRaisesRegex(
                        ARCHIVE.VerificationError, "unsafe archive member"
                    ):
                        ARCHIVE.safe_extract_tar(archive, root / f"output-{index}")

    def test_safe_extract_rejects_symbolic_and_hard_links(self):
        link_types = (tarfile.SYMTYPE, tarfile.LNKTYPE)
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, member_type in enumerate(link_types):
                with self.subTest(member_type=member_type):
                    archive = root / f"link-{index}.tar.gz"
                    write_special_archive(
                        archive,
                        "link",
                        member_type,
                        linkname="../outside",
                    )
                    with self.assertRaisesRegex(
                        ARCHIVE.VerificationError, "unsafe archive link"
                    ):
                        ARCHIVE.safe_extract_tar(archive, root / f"output-{index}")

    def test_safe_extract_rejects_special_members(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "fifo.tar.gz"
            write_special_archive(archive, "named-pipe", tarfile.FIFOTYPE)

            with self.assertRaisesRegex(
                ARCHIVE.VerificationError, "unsupported archive member type"
            ):
                ARCHIVE.safe_extract_tar(archive, root / "output")

    def test_source_archive_requires_every_configured_sentinel(self):
        with TemporaryDirectory() as temporary:
            archive_dir = Path(temporary)
            write_archive(
                archive_dir / "example-sys-1.2.3.crate",
                {"example-sys-1.2.3/src/native.cpp": b"source"},
            )
            packages = (ARCHIVE.PackageRecord("example-sys", Path("example"), "1.2.3"),)
            sentinels = {"example-sys": ("src/native.cpp", "src/missing.cpp")}

            with self.assertRaisesRegex(
                ARCHIVE.VerificationError, "missing native source sentinel"
            ):
                ARCHIVE.verify_source_archives(
                    archive_dir, packages, sys_sentinels=sentinels
                )

    def test_source_archive_rejects_git_metadata(self):
        with TemporaryDirectory() as temporary:
            archive_dir = Path(temporary)
            write_archive(
                archive_dir / "example-sys-1.2.3.crate",
                {
                    "example-sys-1.2.3/src/native.cpp": b"source",
                    "example-sys-1.2.3/.git/config": b"metadata",
                },
            )
            packages = (ARCHIVE.PackageRecord("example-sys", Path("example"), "1.2.3"),)

            with self.assertRaisesRegex(ARCHIVE.VerificationError, r"\.git entry"):
                ARCHIVE.verify_source_archives(
                    archive_dir,
                    packages,
                    sys_sentinels={"example-sys": ("src/native.cpp",)},
                )


class SharedHelperTests(unittest.TestCase):
    def test_command_runner_can_explicitly_accept_any_return_code(self):
        completed = subprocess.CompletedProcess(("tool",), returncode=-9)
        with patch.object(PROCESS.subprocess, "run", return_value=completed):
            result = PROCESS.run(("tool",), accepted_returncodes=None)

        self.assertIs(result, completed)

    def test_temporary_workspace_uses_and_cleans_runner_temp(self):
        with TemporaryDirectory() as temporary:
            runner_temp = Path(temporary)
            with patch.dict(
                os.environ, {"RUNNER_TEMP": os.fspath(runner_temp)}, clear=False
            ):
                with VERIFICATION.temporary_workspace("package-test.") as workspace:
                    self.assertEqual(workspace.parent, runner_temp)
                    self.assertTrue(workspace.is_dir())
                self.assertFalse(workspace.exists())

    def test_temporary_workspace_rejects_an_invalid_runner_temp(self):
        with TemporaryDirectory() as temporary:
            missing = Path(temporary) / "missing"
            with patch.dict(
                os.environ, {"RUNNER_TEMP": os.fspath(missing)}, clear=False
            ):
                with self.assertRaisesRegex(
                    VERIFICATION.VerificationError,
                    "RUNNER_TEMP is not a directory",
                ):
                    with VERIFICATION.temporary_workspace("package-test."):
                        self.fail(
                            "invalid RUNNER_TEMP unexpectedly created a workspace"
                        )


class ProfileMismatchTests(unittest.TestCase):
    def test_accepts_only_the_strict_profile_diagnostic(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"),
            returncode=101,
            stdout="error: selected an incompatible dear_imgui artifact\n",
        )

        PREBUILT.verify_profile_mismatch_result("normal-with-stack", result)

    def test_rejects_an_unexpected_success(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"), returncode=0, stdout=""
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError, "profile mismatch unexpectedly succeeded"
        ):
            PREBUILT.verify_profile_mismatch_result("normal-with-stack", result)

    def test_rejects_a_failure_without_the_strict_diagnostic(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"), returncode=101, stdout="different failure\n"
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError,
            "failed without the strict artifact profile diagnostic",
        ):
            PREBUILT.verify_profile_mismatch_result("normal-with-stack", result)


class ConsumerContractTests(unittest.TestCase):
    def test_consumer_manifest_round_trips_windows_paths_and_profile_features(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source root"
            source_root.mkdir()
            source_root.joinpath("dear-imgui").mkdir()
            source_root.joinpath("Cargo.lock").write_text(
                "version = 4\n", encoding="utf-8"
            )
            destination = root / "consumer"

            PREBUILT.write_prebuilt_consumer(
                destination, source_root, "stack-layout-freetype"
            )

            manifest = tomllib.loads(
                destination.joinpath("Cargo.toml").read_text(encoding="utf-8")
            )
            dependency = manifest["dependencies"]["dear-imgui-rs"]
            self.assertEqual(
                Path(dependency["path"]), (source_root / "dear-imgui").resolve()
            )
            self.assertFalse(dependency["default-features"])
            self.assertEqual(
                dependency["features"], ["prebuilt", "stack-layout", "freetype"]
            )

    def test_cargo_path_patch_is_valid_toml_for_the_host_path(self):
        with TemporaryDirectory() as temporary:
            dependency_path = Path(temporary) / "path with spaces"
            config = SOURCE_PACKAGES.cargo_path_patch("example-sys", dependency_path)

            parsed = tomllib.loads(config)

        self.assertEqual(
            parsed["patch"]["crates-io"]["example-sys"]["path"],
            os.fspath(dependency_path.resolve()),
        )

    def test_prebuilt_environment_removes_ambient_build_modes(self):
        ambient = {
            "IMGUI_SYS_FORCE_BUILD": "1",
            "IMGUI_SYS_PREBUILT_URL": "https://invalid.example",
            "IMGUI_SYS_USE_PREBUILT": "1",
        }
        with patch.dict(os.environ, ambient, clear=False):
            result = PREBUILT.prebuilt_consumer_environment(
                Path("artifact"), Path("target")
            )

        for name in ambient:
            self.assertNotIn(name, result)
        self.assertEqual(result["IMGUI_SYS_SKIP_CC"], "1")
        self.assertEqual(result["IMGUI_SYS_LIB_DIR"], os.fspath(Path("artifact/lib")))
        self.assertEqual(result["CARGO_TARGET_DIR"], os.fspath(Path("target")))

    def test_native_build_environments_remove_ambient_build_modes(self):
        ambient = {
            "IMGUI_SYS_FORCE_BUILD": "ambient",
            "IMGUI_SYS_LIB_DIR": "ambient",
            "IMGUI_SYS_PKG_FEATURES": "ambient",
            "IMGUI_SYS_PREBUILT_URL": "https://invalid.example",
            "IMGUI_SYS_SKIP_CC": "ambient",
            "IMGUI_SYS_USE_PREBUILT": "ambient",
        }
        with (
            patch.dict(os.environ, ambient, clear=False),
            patch.object(PREBUILT, "run") as run_command,
        ):
            PREBUILT.build_host_prebuilt_packages(
                Path("repository"), Path("target"), Path("packages")
            )

        normal_env = run_command.call_args_list[0].kwargs["env"]
        stack_env = run_command.call_args_list[1].kwargs["env"]
        normal_command = run_command.call_args_list[0].args[0]
        stack_command = run_command.call_args_list[1].args[0]
        for name in (
            "IMGUI_SYS_LIB_DIR",
            "IMGUI_SYS_PREBUILT_URL",
            "IMGUI_SYS_SKIP_CC",
            "IMGUI_SYS_USE_PREBUILT",
        ):
            self.assertNotIn(name, normal_env)
            self.assertNotIn(name, stack_env)
        self.assertNotIn("IMGUI_SYS_PKG_FEATURES", normal_env)
        self.assertEqual(normal_env["IMGUI_SYS_FORCE_BUILD"], "1")
        self.assertEqual(stack_env["IMGUI_SYS_FORCE_BUILD"], "1")
        self.assertEqual(stack_env["IMGUI_SYS_PKG_FEATURES"], "stack-layout")
        self.assertNotIn("--no-default-features", normal_command)
        self.assertIn("package-bin", normal_command)
        self.assertIn("--no-default-features", stack_command)
        self.assertIn("package-bin,stack-layout", stack_command)


class PackageWorkspaceTests(unittest.TestCase):
    def test_lock_commit_keeps_commit_message_and_dirty_error_distinct(self):
        command_results = (
            subprocess.CompletedProcess(("git", "add"), 0),
            subprocess.CompletedProcess(("git", "diff"), 1),
            subprocess.CompletedProcess(("git", "commit"), 0),
        )
        with (
            patch.object(
                SOURCE_PACKAGES, "run", side_effect=command_results
            ) as run_command,
            patch.object(
                SOURCE_PACKAGES, "_require_clean_package_workspace"
            ) as require_clean,
        ):
            SOURCE_PACKAGES._commit_lockfile_if_changed(
                Path("repository"),
                "ci: test lock commit",
                "temporary workspace remained dirty",
            )

        commit_command = run_command.call_args_list[2].args[0]
        self.assertEqual(commit_command[-1], "ci: test lock commit")
        require_clean.assert_called_once_with(
            Path("repository"), "temporary workspace remained dirty"
        )

    def test_package_submodules_follow_the_declared_nested_topology(self):
        nested = SOURCE_PACKAGES.PACKAGE_NESTED_SUBMODULES
        declarations = [{"top-level": "top-level-module"}]
        declarations.extend(
            {item.path.as_posix(): f"nested-{index}"}
            for index, item in enumerate(nested)
        )
        with (
            patch.object(
                SOURCE_PACKAGES,
                "_git_submodule_declarations",
                side_effect=declarations,
            ),
            patch.object(SOURCE_PACKAGES, "_configure_local_submodule") as configure,
            patch.object(SOURCE_PACKAGES, "run") as run_command,
        ):
            SOURCE_PACKAGES.initialize_package_submodules(
                Path("source"), Path("repository")
            )

        self.assertEqual(configure.call_count, len(nested) + 1)
        commands = [call.args[0] for call in run_command.call_args_list]
        self.assertEqual(
            commands[0],
            (
                "git",
                "-C",
                Path("repository"),
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "update",
                "--init",
            ),
        )
        self.assertEqual(
            commands[1:],
            [
                (
                    "git",
                    "-C",
                    Path("repository").joinpath(item.parent),
                    "-c",
                    "protocol.file.allow=always",
                    "submodule",
                    "update",
                    "--init",
                    item.path.as_posix(),
                )
                for item in nested
            ],
        )


class CliTests(unittest.TestCase):
    def test_default_and_full_commands_run_the_complete_gate(self):
        with patch.object(CLI, "verify_packaged_core") as verify:
            self.assertEqual(CLI.main([]), 0)
            self.assertEqual(CLI.main(["full"]), 0)

        self.assertEqual(verify.call_count, 2)

    def test_prebuilt_command_routes_named_arguments(self):
        with patch.object(CLI, "verify_core_prebuilt_packages") as verify:
            result = CLI.main(
                ["prebuilt", "packages", "x86_64-unknown-linux-gnu", "static"]
            )

        self.assertEqual(result, 0)
        verify.assert_called_once_with(
            Path("packages"),
            "x86_64-unknown-linux-gnu",
            crt="static",
            source_root=CLI.WORKSPACE_ROOT,
            profile_scope="all",
        )

    def test_legacy_prebuilt_alias_remains_compatible(self):
        with patch.object(CLI, "verify_core_prebuilt_packages") as verify:
            result = CLI.main(
                [
                    "--verify-prebuilt-packages",
                    "packages",
                    "aarch64-apple-darwin",
                ]
            )

        self.assertEqual(result, 0)
        verify.assert_called_once_with(
            Path("packages"),
            "aarch64-apple-darwin",
            crt="",
            source_root=CLI.WORKSPACE_ROOT,
            profile_scope="all",
        )

    def test_verification_failure_returns_one(self):
        stderr = io.StringIO()
        with (
            patch.object(
                CLI,
                "verify_packaged_core",
                side_effect=CLI.VerificationError("failed"),
            ),
            redirect_stderr(stderr),
        ):
            result = CLI.main([])

        self.assertEqual(result, 1)
        self.assertEqual(stderr.getvalue(), "::error::failed\n")

    def test_usage_failure_exits_with_two(self):
        with (
            redirect_stderr(io.StringIO()),
            self.assertRaises(SystemExit) as raised,
        ):
            CLI.main(["prebuilt", "packages"])

        self.assertEqual(raised.exception.code, 2)

    def test_help_names_the_prebuilt_arguments(self):
        help_text = CLI._build_parser().format_help()

        self.assertIn("prebuilt PACKAGE_DIR TARGET [CRT]", help_text)
        self.assertIn("--verify-prebuilt-packages PACKAGE_DIR TARGET [CRT]", help_text)


if __name__ == "__main__":
    unittest.main()
