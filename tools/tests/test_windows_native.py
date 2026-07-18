import importlib
import subprocess
import sys
import tomllib
import unittest
from pathlib import Path, PureWindowsPath
from tempfile import TemporaryDirectory


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

WINDOWS_NATIVE = importlib.import_module("_windows_native")


class VcpkgTripletTests(unittest.TestCase):
    def test_maps_supported_msvc_targets_for_md_and_mt(self):
        expected = {
            ("x86_64-pc-windows-msvc", "md"): "x64-windows-static-md",
            ("x86_64-pc-windows-msvc", "mt"): "x64-windows-static",
            ("i686-pc-windows-msvc", "md"): "x86-windows-static-md",
            ("aarch64-pc-windows-msvc", "mt"): "arm64-windows-static",
            ("thumbv7a-pc-windows-msvc", "md"): "arm-windows-static-md",
        }
        for (target, crt), expected_triplet in expected.items():
            with self.subTest(target=target, crt=crt):
                triplet = WINDOWS_NATIVE.VcpkgTriplet.from_target(target, crt)
                self.assertEqual(triplet.name, expected_triplet)
                self.assertEqual(
                    triplet.package("sdl3"), f"sdl3:{expected_triplet}"
                )

    def test_rejects_non_msvc_target_and_unknown_crt(self):
        with self.assertRaisesRegex(
            WINDOWS_NATIVE.WindowsNativeError, "unsupported MSVC target"
        ):
            WINDOWS_NATIVE.VcpkgTriplet.from_target(
                "x86_64-pc-windows-gnu", "md"
            )
        with self.assertRaisesRegex(
            WINDOWS_NATIVE.WindowsNativeError, "expected 'md' or 'mt'"
        ):
            WINDOWS_NATIVE.VcpkgTriplet.from_target(
                "x86_64-pc-windows-msvc", "dynamic"
            )


class VcpkgRootTests(unittest.TestCase):
    def test_candidates_preserve_precedence_and_deduplicate_windows_paths(self):
        candidates = WINDOWS_NATIVE.vcpkg_root_candidates(
            {
                "VCPKG_ROOT": r"C:\Program Files\vcpkg",
                "VCPKG_INSTALLATION_ROOT": "c:\\program files\\vcpkg\\",
            },
            r"D:\tools\vcpkg\vcpkg.exe",
        )

        self.assertEqual(
            [(candidate.source, str(candidate.path)) for candidate in candidates],
            [
                ("VCPKG_ROOT", r"C:\Program Files\vcpkg"),
                ("vcpkg executable", r"D:\tools\vcpkg"),
            ],
        )

    def test_candidates_skip_missing_and_blank_environment_values(self):
        candidates = WINDOWS_NATIVE.vcpkg_root_candidates(
            {"VCPKG_ROOT": "", "VCPKG_INSTALLATION_ROOT": "   "},
            "/opt/vcpkg/vcpkg",
        )
        self.assertEqual(len(candidates), 1)
        self.assertEqual(candidates[0].path, Path("/opt/vcpkg"))

    def test_root_resolution_selects_first_marker_and_reports_all_failures(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "missing"
            second = root / "vcpkg with spaces"
            second.mkdir()
            (second / ".vcpkg-root").touch()
            candidates = (
                WINDOWS_NATIVE.VcpkgRootCandidate(first, "VCPKG_ROOT"),
                WINDOWS_NATIVE.VcpkgRootCandidate(
                    second, "VCPKG_INSTALLATION_ROOT"
                ),
            )
            self.assertEqual(
                WINDOWS_NATIVE.resolve_vcpkg_root(candidates).path, second
            )
            with self.assertRaisesRegex(
                WINDOWS_NATIVE.VcpkgRootError, "VCPKG_ROOT=.*missing"
            ):
                WINDOWS_NATIVE.resolve_vcpkg_root(candidates[:1])


class VcpkgStatusTests(unittest.TestCase):
    def _root(self, temporary):
        root = Path(temporary) / "vcpkg"
        root.mkdir()
        (root / ".vcpkg-root").touch()
        return root

    def test_repairs_only_missing_updates_directory_when_status_is_nonempty(self):
        with TemporaryDirectory() as temporary:
            root = self._root(temporary)
            status = root / "installed" / "vcpkg" / "status"
            status.parent.mkdir(parents=True)
            status.write_text("Package: sdl3\n", encoding="utf-8")

            before = WINDOWS_NATIVE.inspect_vcpkg_status(root)
            self.assertTrue(before.needs_updates_directory)
            repaired = WINDOWS_NATIVE.ensure_vcpkg_status_compatibility(root)

            self.assertFalse(repaired.needs_updates_directory)
            self.assertGreater(repaired.status_bytes, 0)
            self.assertEqual(status.read_text(encoding="utf-8"), "Package: sdl3\n")

    def test_rejects_missing_and_empty_status_without_updates(self):
        for create_empty_status in (False, True):
            with self.subTest(create_empty_status=create_empty_status):
                with TemporaryDirectory() as temporary:
                    root = self._root(temporary)
                    if create_empty_status:
                        status = root / "installed" / "vcpkg" / "status"
                        status.parent.mkdir(parents=True)
                        status.touch()
                    with self.assertRaisesRegex(
                        WINDOWS_NATIVE.VcpkgStatusError, "missing or empty"
                    ):
                        WINDOWS_NATIVE.ensure_vcpkg_status_compatibility(root)

    def test_accepts_nonempty_update_status_when_main_status_is_missing(self):
        with TemporaryDirectory() as temporary:
            root = self._root(temporary)
            updates = root / "installed" / "vcpkg" / "updates"
            updates.mkdir(parents=True)
            (updates / "000002").write_text("Package: freetype\n", encoding="utf-8")
            (updates / "000001").touch()

            status = WINDOWS_NATIVE.ensure_vcpkg_status_compatibility(root)

            self.assertTrue(status.has_status_data)
            self.assertEqual(
                [path.name for path in status.update_files], ["000001", "000002"]
            )


class VcpkgInstallTests(unittest.TestCase):
    def test_uses_resolved_executable_and_explicit_triplet(self):
        triplet = WINDOWS_NATIVE.VcpkgTriplet.from_target(
            "x86_64-pc-windows-msvc", "md"
        )
        observed = {}

        def runner(command, **_kwargs):
            observed["command"] = tuple(command)
            return subprocess.CompletedProcess(command, 0)

        WINDOWS_NATIVE.install_vcpkg_packages(
            ("freetype", "sdl3"),
            triplet,
            executable=PureWindowsPath(r"C:\Program Files\vcpkg\vcpkg.exe"),
            runner=runner,
        )

        self.assertEqual(
            observed["command"],
            (
                r"C:\Program Files\vcpkg\vcpkg.exe",
                "install",
                "freetype:x64-windows-static-md",
                "sdl3:x64-windows-static-md",
            ),
        )


class GithubOutputTests(unittest.TestCase):
    def test_environment_output_is_exact_lf_utf8_without_bom(self):
        with TemporaryDirectory() as temporary:
            output = Path(temporary) / "github-env"
            WINDOWS_NATIVE.append_github_assignments(
                output,
                (
                    ("VCPKG_ROOT", r"C:\Program Files\vcpkg"),
                    ("VCPKGRS_TRIPLET", "x64-windows-static-md"),
                    ("PKG_CONFIG_PATH", ""),
                ),
            )
            expected = (
                b"VCPKG_ROOT=C:\\Program Files\\vcpkg\n"
                b"VCPKGRS_TRIPLET=x64-windows-static-md\n"
                b"PKG_CONFIG_PATH=\n"
            )
            self.assertEqual(output.read_bytes(), expected)
            self.assertFalse(output.read_bytes().startswith(b"\xef\xbb\xbf"))
            self.assertNotIn(b"\r\n", output.read_bytes())

    def test_path_output_is_exact_and_rejects_multiline_values(self):
        self.assertEqual(
            WINDOWS_NATIVE.github_path_bytes((r"C:\msys64\mingw64\bin",)),
            b"C:\\msys64\\mingw64\\bin\n",
        )
        with self.assertRaisesRegex(
            WINDOWS_NATIVE.WindowsNativeError, "must fit on one line"
        ):
            WINDOWS_NATIVE.github_assignment_bytes((("VALUE", "first\nsecond"),))


class Sdl3ConsumerTests(unittest.TestCase):
    def test_windows_repository_path_with_spaces_is_valid_toml_and_lf(self):
        with TemporaryDirectory() as temporary:
            consumer = WINDOWS_NATIVE.create_sdl3_vcpkg_consumer(
                Path(temporary) / "consumer with spaces",
                PureWindowsPath(r"C:\source trees\dear-imgui-rs"),
            )
            manifest_bytes = consumer.manifest.read_bytes()
            parsed = tomllib.loads(manifest_bytes.decode("utf-8"))

            dependency = parsed["build-dependencies"]["build-support"]
            self.assertEqual(
                dependency["path"],
                r"C:\source trees\dear-imgui-rs\tools\build-support",
            )
            self.assertFalse(manifest_bytes.startswith(b"\xef\xbb\xbf"))
            self.assertNotIn(b"\r\n", manifest_bytes)
            self.assertEqual(
                consumer.command.arguments,
                ("cargo", "check", "--manifest-path", str(consumer.manifest)),
            )
            self.assertEqual(consumer.command.cwd, consumer.root)

    def test_unix_repository_path_is_valid_and_runner_contract_propagates(self):
        with TemporaryDirectory() as temporary:
            workspace = Path(temporary) / "consumer"
            observed = {}

            def runner(command, **kwargs):
                observed["command"] = tuple(command)
                observed["cwd"] = kwargs["cwd"]
                return subprocess.CompletedProcess(command, 23)

            result = WINDOWS_NATIVE.check_sdl3_vcpkg_consumer(
                workspace, "/source trees/dear-imgui-rs", runner=runner
            )
            parsed = tomllib.loads(
                (workspace / "Cargo.toml").read_text(encoding="utf-8")
            )

            self.assertEqual(result.returncode, 23)
            self.assertEqual(
                parsed["build-dependencies"]["build-support"]["path"],
                "/source trees/dear-imgui-rs/tools/build-support",
            )
            self.assertEqual(observed["cwd"], workspace)
            self.assertEqual(observed["command"][0:2], ("cargo", "check"))


class MinGwEnvironmentTests(unittest.TestCase):
    def test_windows_path_is_prepended_once_case_insensitively(self):
        environment = WINDOWS_NATIVE.calculate_mingw_environment(
            r"C:\msys64",
            r"C:\Windows;C:\MSYS64\MINGW64\BIN;C:\Tools",
        )
        self.assertEqual(
            environment.bin_directory,
            PureWindowsPath(r"C:\msys64\mingw64\bin"),
        )
        self.assertEqual(
            environment.path,
            r"C:\msys64\mingw64\bin;C:\Windows;C:\Tools",
        )
        self.assertEqual(
            environment.github_environment,
            (("MINGW_BIN", r"C:\msys64\mingw64\bin"),),
        )
        self.assertEqual(
            environment.tool("objdump.exe"),
            PureWindowsPath(r"C:\msys64\mingw64\bin\objdump.exe"),
        )

    def test_unix_path_uses_requested_separator(self):
        environment = WINDOWS_NATIVE.calculate_mingw_environment(
            "/opt/msys", "/usr/bin:/bin", path_separator=":"
        )
        self.assertEqual(
            environment.path, "/opt/msys/mingw64/bin:/usr/bin:/bin"
        )


class MinGwImportTests(unittest.TestCase):
    def test_finder_returns_zero_one_and_multiple_in_stable_order(self):
        with TemporaryDirectory() as temporary:
            deps = Path(temporary)
            self.assertEqual(WINDOWS_NATIVE.find_mingw_test_executables(deps), ())
            second = deps / "dear_imgui_sys-B.exe"
            first = deps / "dear_imgui_sys-a.exe"
            second.touch()
            first.touch()
            (deps / "dear_imgui_sys-object.exe").mkdir()

            self.assertEqual(
                WINDOWS_NATIVE.find_mingw_test_executables(deps), (first, second)
            )

    def test_zero_matches_is_a_typed_error(self):
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(
                WINDOWS_NATIVE.MissingTestBinaryError, "no dear_imgui_sys"
            ):
                WINDOWS_NATIVE.inspect_mingw_imports(
                    temporary, "objdump.exe", runner=lambda *_args, **_kwargs: None
                )

    def test_multiple_binaries_preserve_output_and_parse_imports(self):
        outputs = {
            "dear_imgui_sys-a.exe": "DLL Name: KERNEL32.dll\nmarker-a\n",
            "dear_imgui_sys-b.exe": "marker-b\n  DLL Name: USER32.dll\n",
        }
        with TemporaryDirectory() as temporary:
            deps = Path(temporary)
            for name in reversed(tuple(outputs)):
                (deps / name).touch()

            def runner(command, **_kwargs):
                output = outputs[Path(command[-1]).name]
                return subprocess.CompletedProcess(command, 0, stdout=output)

            inspection = WINDOWS_NATIVE.verify_mingw_imports(
                deps, "objdump.exe", runner=runner
            )

            self.assertEqual(
                [item.binary.name for item in inspection.evidence],
                ["dear_imgui_sys-a.exe", "dear_imgui_sys-b.exe"],
            )
            self.assertEqual(inspection.evidence[0].imports, ("KERNEL32.dll",))
            self.assertIn("marker-a", inspection.evidence_text)
            self.assertIn("marker-b", inspection.evidence_text)

    def test_objdump_failure_preserves_exit_and_output(self):
        with TemporaryDirectory() as temporary:
            binary = Path(temporary) / "dear_imgui_sys-a.exe"
            binary.touch()

            def runner(command, **_kwargs):
                return subprocess.CompletedProcess(
                    command, 9, stdout="objdump diagnostic\n"
                )

            with self.assertRaises(WINDOWS_NATIVE.CommandError) as failure:
                WINDOWS_NATIVE.inspect_mingw_imports(
                    temporary, "objdump.exe", runner=runner
                )

            self.assertEqual(failure.exception.returncode, 9)
            self.assertIn("objdump diagnostic", failure.exception.output)

    def test_forbidden_import_is_case_insensitive_and_retains_evidence(self):
        with TemporaryDirectory() as temporary:
            binary = Path(temporary) / "dear_imgui_sys-a.exe"
            binary.touch()
            raw_output = (
                "private diagnostic\n"
                "  DLL Name: KERNEL32.dll\n"
                "  DLL Name: LiBsTdC++-6.DlL\n"
            )

            def runner(command, **_kwargs):
                return subprocess.CompletedProcess(command, 0, stdout=raw_output)

            with self.assertRaises(WINDOWS_NATIVE.ForbiddenImportError) as failure:
                WINDOWS_NATIVE.verify_mingw_imports(
                    temporary, "objdump.exe", runner=runner
                )

            inspection = failure.exception.inspection
            self.assertEqual(len(inspection.forbidden_evidence), 1)
            self.assertIn(raw_output, inspection.evidence_text)
            self.assertIn("private diagnostic", str(failure.exception))


if __name__ == "__main__":
    unittest.main()
