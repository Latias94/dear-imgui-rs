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

    def test_github_environment_is_exact_for_md_and_mt(self):
        root = PureWindowsPath(r"C:\Program Files\vcpkg")
        runner_temp = PureWindowsPath(r"D:\runner temp")
        md = WINDOWS_NATIVE.VcpkgTriplet.from_target(
            "x86_64-pc-windows-msvc", "md"
        )
        mt = WINDOWS_NATIVE.VcpkgTriplet.from_target(
            "x86_64-pc-windows-msvc", "mt"
        )

        common = (
            ("VCPKG_ROOT", r"C:\Program Files\vcpkg"),
            ("PKG_CONFIG", r"D:\runner temp\missing-pkg-config.exe"),
            ("PKG_CONFIG_PATH", ""),
        )
        self.assertEqual(
            WINDOWS_NATIVE.vcpkg_github_environment(root, md, runner_temp),
            (common[0], ("VCPKGRS_TRIPLET", "x64-windows-static-md"), *common[1:]),
        )
        self.assertEqual(
            WINDOWS_NATIVE.vcpkg_github_environment(root, mt, runner_temp),
            (
                common[0],
                ("VCPKGRS_TRIPLET", "x64-windows-static"),
                *common[1:],
                ("RUSTFLAGS", "-C target-feature=+crt-static"),
            ),
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


class Sdl3RuntimeTests(unittest.TestCase):
    def test_restores_cached_runtime_to_the_cargo_profile_directory(self):
        with TemporaryDirectory() as temporary:
            target = Path(temporary) / "target"
            cached = (
                target
                / "debug"
                / "build"
                / "sdl3-sys-a"
                / "out"
                / "bin"
                / "SDL3.dll"
            )
            cached.parent.mkdir(parents=True)
            cached.write_bytes(b"MZ-cached-sdl3")

            restored = WINDOWS_NATIVE.restore_cached_sdl3_runtime(target)

            self.assertEqual(restored, target / "debug" / "SDL3.dll")
            self.assertEqual(restored.read_bytes(), b"MZ-cached-sdl3")

    def test_selects_cached_runtime_deterministically(self):
        with TemporaryDirectory() as temporary:
            target = Path(temporary) / "target"
            for build_hash, payload in (
                ("sdl3-sys-b", b"MZ-second"),
                ("sdl3-sys-a", b"MZ-first"),
            ):
                cached = (
                    target
                    / "debug"
                    / "build"
                    / build_hash
                    / "out"
                    / "bin"
                    / "SDL3.dll"
                )
                cached.parent.mkdir(parents=True)
                cached.write_bytes(payload)

            restored = WINDOWS_NATIVE.restore_cached_sdl3_runtime(target)

            self.assertEqual(restored.read_bytes(), b"MZ-first")

    def test_rejects_missing_or_empty_cached_runtime(self):
        with TemporaryDirectory() as temporary:
            target = Path(temporary) / "target"
            with self.assertRaisesRegex(
                WINDOWS_NATIVE.WindowsNativeError,
                "no cached SDL3 runtime DLL",
            ):
                WINDOWS_NATIVE.restore_cached_sdl3_runtime(target)

            cached = (
                target
                / "debug"
                / "build"
                / "sdl3-sys-a"
                / "out"
                / "bin"
                / "SDL3.dll"
            )
            cached.parent.mkdir(parents=True)
            cached.touch()
            with self.assertRaisesRegex(
                WINDOWS_NATIVE.WindowsNativeError,
                "runtime DLL is empty",
            ):
                WINDOWS_NATIVE.restore_cached_sdl3_runtime(target)


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


class WindowsPeEvidenceTests(unittest.TestCase):
    def test_explicit_patterns_require_every_sentinel_and_deduplicate(self):
        with TemporaryDirectory() as temporary:
            deps = Path(temporary)
            core = deps / "numeric_contract-core.exe"
            implot = deps / "dear_implot_sys-smoke.exe"
            core.touch()
            implot.touch()

            binaries = WINDOWS_NATIVE.find_windows_test_executables(
                deps,
                (
                    "numeric_contract-*.exe",
                    "dear_implot_sys-*.exe",
                    "*.exe",
                ),
            )
            self.assertEqual(binaries, (implot, core))

            with self.assertRaisesRegex(
                WINDOWS_NATIVE.MissingTestBinaryError,
                "dear_imnodes_sys-\\*.exe",
            ):
                WINDOWS_NATIVE.find_windows_test_executables(
                    deps,
                    ("numeric_contract-*.exe", "dear_imnodes_sys-*.exe"),
                )

    def test_required_and_forbidden_import_policies_are_case_insensitive(self):
        with TemporaryDirectory() as temporary:
            binary = Path(temporary) / "numeric_contract-core.exe"
            binary.touch()
            raw_output = (
                f"{binary}: file format coff-x86-64\n"
                "  DLL Name: KERNEL32.dll\n"
                "  DLL Name: LiBuNwInD.DlL\n"
            )

            def runner(command, **_kwargs):
                return subprocess.CompletedProcess(command, 0, stdout=raw_output)

            inspection = WINDOWS_NATIVE.verify_windows_pe(
                temporary,
                "llvm-objdump.exe",
                binary_patterns=("numeric_contract-*.exe",),
                required_imports=("libunwind.dll",),
                forbidden_imports=("libstdc++-6.dll", "libc++.dll"),
                expected_machine="coff-x86-64",
                runner=runner,
            )
            self.assertEqual(
                inspection.evidence[0].imports,
                ("KERNEL32.dll", "LiBuNwInD.DlL"),
            )

            with self.assertRaises(WINDOWS_NATIVE.ImportPolicyError) as forbidden:
                WINDOWS_NATIVE.verify_windows_pe(
                    temporary,
                    "llvm-objdump.exe",
                    binary_patterns=("numeric_contract-*.exe",),
                    forbidden_imports=("LIBUNWIND.DLL",),
                    runner=runner,
                )
            self.assertEqual(forbidden.exception.forbidden_imports, ("LiBuNwInD.DlL",))
            self.assertIn("LiBuNwInD.DlL", forbidden.exception.inspection.evidence_text)

            with self.assertRaises(WINDOWS_NATIVE.ImportPolicyError) as missing:
                WINDOWS_NATIVE.verify_windows_pe(
                    temporary,
                    "llvm-objdump.exe",
                    binary_patterns=("numeric_contract-*.exe",),
                    required_imports=("missing-runtime.dll",),
                    runner=runner,
                )
            self.assertEqual(missing.exception.missing_imports, ("missing-runtime.dll",))

    def test_import_policy_is_enforced_for_every_binary(self):
        with TemporaryDirectory() as temporary:
            deps = Path(temporary)
            first = deps / "first-core.exe"
            second = deps / "second-extension.exe"
            first.touch()
            second.touch()
            outputs = {
                first.name: (
                    f"{first}: file format coff-x86-64\n"
                    "  DLL Name: KERNEL32.dll\n"
                    "  DLL Name: libunwind.dll\n"
                ),
                second.name: (
                    f"{second}: file format coff-x86-64\n"
                    "  DLL Name: USER32.dll\n"
                    "  DLL Name: libunwind.dll\n"
                ),
            }

            def runner(command, **_kwargs):
                return subprocess.CompletedProcess(
                    command, 0, stdout=outputs[Path(command[-1]).name]
                )

            patterns = ("first-*.exe", "second-*.exe")
            inspection = WINDOWS_NATIVE.verify_windows_pe(
                deps,
                "llvm-objdump.exe",
                binary_patterns=patterns,
                required_imports=("libunwind.dll",),
                runner=runner,
            )
            self.assertEqual(len(inspection.evidence), 2)

            outputs[second.name] = (
                f"{second}: file format coff-x86-64\n"
                "  DLL Name: USER32.dll\n"
            )
            with self.assertRaises(WINDOWS_NATIVE.ImportPolicyError) as missing:
                WINDOWS_NATIVE.verify_windows_pe(
                    deps,
                    "llvm-objdump.exe",
                    binary_patterns=patterns,
                    required_imports=("libunwind.dll",),
                    runner=runner,
                )
            self.assertEqual(missing.exception.missing_imports, ("libunwind.dll",))
            self.assertIn(first.name, missing.exception.inspection.evidence_text)
            self.assertIn(second.name, missing.exception.inspection.evidence_text)

            outputs[second.name] = (
                f"{second}: file format coff-x86-64\n"
                "  DLL Name: libunwind.dll\n"
                "  DLL Name: libc++.dll\n"
            )
            with self.assertRaises(WINDOWS_NATIVE.ImportPolicyError) as forbidden:
                WINDOWS_NATIVE.verify_windows_pe(
                    deps,
                    "llvm-objdump.exe",
                    binary_patterns=patterns,
                    required_imports=("libunwind.dll",),
                    forbidden_imports=("libc++.dll",),
                    runner=runner,
                )
            self.assertEqual(forbidden.exception.forbidden_imports, ("libc++.dll",))
            self.assertIn(first.name, forbidden.exception.inspection.evidence_text)
            self.assertIn(second.name, forbidden.exception.inspection.evidence_text)

    def test_machine_policy_reports_parsed_and_expected_values(self):
        with TemporaryDirectory() as temporary:
            binary = Path(temporary) / "numeric_contract-arm64.exe"
            binary.touch()
            raw_output = f"{binary}: file format coff-arm64\n"

            def runner(command, **_kwargs):
                return subprocess.CompletedProcess(command, 0, stdout=raw_output)

            inspection = WINDOWS_NATIVE.verify_windows_pe(
                temporary,
                "llvm-objdump.exe",
                binary_patterns=("numeric_contract-*.exe",),
                expected_machine="coff-arm64",
                runner=runner,
            )
            self.assertEqual(inspection.evidence[0].machine, "coff-arm64")
            self.assertIn("Parsed machine: coff-arm64", inspection.evidence_text)

            with self.assertRaises(WINDOWS_NATIVE.MachineTypeError) as failure:
                WINDOWS_NATIVE.verify_windows_pe(
                    temporary,
                    "llvm-objdump.exe",
                    binary_patterns=("numeric_contract-*.exe",),
                    expected_machine="coff-x86-64",
                    runner=runner,
                )
            self.assertEqual(failure.exception.expected_machine, "coff-x86-64")
            self.assertEqual(failure.exception.actual_machines, ("coff-arm64",))
            self.assertIn("coff-arm64", failure.exception.inspection.evidence_text)

    def test_machine_policy_reports_missing_objdump_machine_output(self):
        with TemporaryDirectory() as temporary:
            binary = Path(temporary) / "numeric_contract-core.exe"
            binary.touch()
            raw_output = "  DLL Name: KERNEL32.dll\n"

            def runner(command, **_kwargs):
                return subprocess.CompletedProcess(command, 0, stdout=raw_output)

            with self.assertRaises(WINDOWS_NATIVE.MachineTypeError) as failure:
                WINDOWS_NATIVE.verify_windows_pe(
                    temporary,
                    "llvm-objdump.exe",
                    binary_patterns=("numeric_contract-*.exe",),
                    expected_machine="coff-x86-64",
                    runner=runner,
                )

            self.assertEqual(failure.exception.actual_machines, ("<missing>",))
            self.assertIn(
                "Parsed machine: <missing>",
                failure.exception.inspection.evidence_text,
            )

    def test_command_failure_retains_prior_and_failed_binary_evidence(self):
        with TemporaryDirectory() as temporary:
            deps = Path(temporary)
            first = deps / "first-a.exe"
            second = deps / "second-b.exe"
            first.touch()
            second.touch()

            def runner(command, **_kwargs):
                binary = Path(command[-1])
                if binary == first:
                    return subprocess.CompletedProcess(
                        command,
                        0,
                        stdout=(
                            f"{binary}: file format coff-x86-64\n"
                            "  DLL Name: KERNEL32.dll\n"
                        ),
                    )
                return subprocess.CompletedProcess(
                    command,
                    9,
                    stdout="llvm-objdump diagnostic\n",
                )

            with self.assertRaises(WINDOWS_NATIVE.InspectionCommandError) as failure:
                WINDOWS_NATIVE.inspect_windows_pe(
                    deps,
                    "llvm-objdump.exe",
                    binary_patterns=("first-*.exe", "second-*.exe"),
                    runner=runner,
                )

            self.assertEqual(failure.exception.returncode, 9)
            evidence = failure.exception.inspection.evidence_text
            self.assertIn("Parsed machine: coff-x86-64", evidence)
            self.assertIn("KERNEL32.dll", evidence)
            self.assertIn("llvm-objdump diagnostic", evidence)

    def test_command_timeout_retains_partial_output_and_uses_default_deadline(self):
        with TemporaryDirectory() as temporary:
            binary = Path(temporary) / "numeric_contract-core.exe"
            binary.touch()
            observed = {}

            def runner(command, **kwargs):
                observed.update(kwargs)
                raise subprocess.TimeoutExpired(
                    command,
                    kwargs["timeout"],
                    output="partial stdout\n",
                    stderr="partial stderr\n",
                )

            with self.assertRaises(WINDOWS_NATIVE.InspectionCommandError) as failure:
                WINDOWS_NATIVE.inspect_windows_pe(
                    temporary,
                    "llvm-objdump.exe",
                    binary_patterns=("numeric_contract-*.exe",),
                    runner=runner,
                )

            self.assertEqual(observed["timeout"], 60.0)
            self.assertIsNone(observed["accepted_returncodes"])
            self.assertEqual(failure.exception.returncode, -1)
            self.assertIn("partial stdout", failure.exception.output)
            self.assertIn("partial stderr", failure.exception.output)
            evidence = failure.exception.inspection.evidence_text
            self.assertIn("Exit code: -1", evidence)
            self.assertIn("timed out after 60 seconds", evidence)


if __name__ == "__main__":
    unittest.main()
