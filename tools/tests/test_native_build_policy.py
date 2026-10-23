"""Real Cargo checks for native build-mode contracts, without compiling C++."""

import json
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
SYS_CRATES = {
    "dear-imgui-sys": "IMGUI_SYS",
    "dear-implot-sys": "IMPLOT_SYS",
    "dear-implot3d-sys": "IMPLOT3D_SYS",
    "dear-imnodes-sys": "IMNODES_SYS",
    "dear-node-editor-sys": "NODE_EDITOR_SYS",
    "dear-imguizmo-sys": "IMGUIZMO_SYS",
    "dear-imguizmo-quat-sys": "IMGUIZMO_QUAT_SYS",
    "dear-imgui-cte-sys": "CTE_SYS",
}


@unittest.skipUnless(shutil.which("cargo"), "Cargo is required")
class NativeBuildPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = TemporaryDirectory(prefix="dear-imgui-build-policy-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.consumer = Path(cls.temporary.name)
        dependencies = []
        for crate in SYS_CRATES:
            path = ROOT / (crate if crate == "dear-imgui-sys" else f"extensions/{crate}")
            dependencies.append(f"{crate} = {{ path = {json.dumps(str(path))} }}")
        dependencies.append(
            "dear-imgui-test-engine-sys = { path = "
            + json.dumps(str(ROOT / "extensions/dear-imgui-test-engine-sys"))
            + ", optional = true }"
        )
        (cls.consumer / "Cargo.toml").write_text(
            '[package]\nname = "native-policy-contract"\nversion = "0.0.0"\n'
            'edition = "2024"\n[dependencies]\n'
            + "\n".join(dependencies)
            + "\n[workspace]\n",
            encoding="utf-8",
        )
        (cls.consumer / "src").mkdir()
        (cls.consumer / "src/lib.rs").write_text("", encoding="utf-8")

    def check(self, overrides=None, features=()):
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith(tuple(SYS_CRATES.values()))
            and key not in ("DOCS_RS", "DEAR_IMGUI_RS_REGEN_BINDINGS", "DEAR_IMGUI_RS_CANDIDATE_SHA")
        }
        env.update({f"{prefix}_BUILD_MODE": "bindings-only" for prefix in SYS_CRATES.values()})
        env.update({"CARGO_TARGET_DIR": str(ROOT / "target"), "CARGO_BUILD_JOBS": "2"})
        for key, value in (overrides or {}).items():
            if value is None:
                env.pop(key, None)
            else:
                env[key] = str(value)
        command = ["cargo", "check", "--manifest-path", str(self.consumer / "Cargo.toml")]
        if features:
            command.extend(("--features", ",".join(features)))
        result = subprocess.run(command, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return result.returncode, result.stdout

    def assert_failure(self, overrides, expected, features=()):
        code, output = self.check(overrides, features)
        self.assertNotEqual(code, 0, output)
        self.assertIn(expected, output)
        self.assertNotIn("undefined symbol", output.lower())

    def test_all_sys_crates_support_bindings_only(self):
        code, output = self.check()
        self.assertEqual(code, 0, output)

    def test_legacy_skip_alone_still_supports_checks(self):
        overrides = {}
        for prefix in SYS_CRATES.values():
            overrides[f"{prefix}_BUILD_MODE"] = None
            overrides[f"{prefix}_SKIP_CC"] = "1"
        code, output = self.check(overrides)
        self.assertEqual(code, 0, output)

    def test_test_engine_legacy_dual_skip_preserves_the_ci_check_only_route(self):
        overrides = {f"{prefix}_BUILD_MODE": None for prefix in SYS_CRATES.values()}
        overrides.update({f"{prefix}_SKIP_CC": "1" for prefix in SYS_CRATES.values()})
        for value in ("1", "True", "Yes", "On"):
            with self.subTest(value=value):
                overrides["IMGUI_SYS_SKIP_CC"] = value
                overrides["IMGUI_TEST_ENGINE_SYS_SKIP_CC"] = value
                code, output = self.check(overrides, ("dear-imgui-test-engine-sys",))
                self.assertEqual(code, 0, output)

    def test_strict_prebuilt_without_an_artifact_fails_in_each_build_script(self):
        for prefix in SYS_CRATES.values():
            with self.subTest(prefix=prefix):
                self.assert_failure({f"{prefix}_BUILD_MODE": "prebuilt"}, "no compatible prebuilt")

    def test_legacy_source_and_skip_conflict_in_every_crate(self):
        for prefix in SYS_CRATES.values():
            with self.subTest(prefix=prefix):
                self.assert_failure({
                    f"{prefix}_BUILD_MODE": None,
                    f"{prefix}_SKIP_CC": "1",
                    f"{prefix}_FORCE_BUILD": "1",
                }, "SKIP_CC conflicts with a required source build")

    def test_strict_prebuilt_rejects_unified_source_requirements(self):
        for crate, prefix in SYS_CRATES.items():
            with self.subTest(crate=crate):
                self.assert_failure(
                    {f"{prefix}_BUILD_MODE": "prebuilt"},
                    "conflicts with a required source build",
                    (f"{crate}/build-from-source",),
                )

    def test_issue_96_extraction_root_reports_the_lib_subdirectory(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "lib").mkdir()
            (root / "lib/libdear_imgui.a").write_bytes(b"")
            (root / "lib/dear_imgui.lib").write_bytes(b"")
            self.assert_failure({
                "IMGUI_SYS_BUILD_MODE": None,
                "IMGUI_SYS_SKIP_CC": "1",
                "IMGUI_SYS_LIB_DIR": root,
            }, f"point to {root / 'lib'} instead")

    def test_missing_explicit_core_directory_does_not_fall_back(self):
        self.assert_failure({
            "IMGUI_SYS_BUILD_MODE": "auto",
            "IMGUI_SYS_LIB_DIR": self.consumer / "missing",
        }, "does not contain")

    def test_bindings_only_rejects_artifact_selectors(self):
        for prefix in SYS_CRATES.values():
            with self.subTest(prefix=prefix):
                self.assert_failure({f"{prefix}_LIB_DIR": self.consumer}, "cannot be combined with a prebuilt selector")

    def test_multiple_explicit_selectors_are_rejected(self):
        self.assert_failure({
            "IMGUI_SYS_BUILD_MODE": "auto",
            "IMGUI_SYS_LIB_DIR": self.consumer,
            "IMGUI_SYS_PACKAGE_DIR": self.consumer,
        }, "set only one of")

    def test_new_and_enabled_legacy_modes_are_not_mixed(self):
        self.assert_failure({"IMGUI_SYS_SKIP_CC": "1"}, "use one build-mode interface")

    def test_disabled_legacy_flags_do_not_override_the_new_mode(self):
        code, output = self.check({"IMGUI_SYS_SKIP_CC": "0", "IMGUI_SYS_FORCE_BUILD": "false"})
        self.assertEqual(code, 0, output)

    def test_download_environment_toggle_requires_download_capability(self):
        self.assert_failure({"IMGUI_SYS_BUILD_MODE": "auto", "IMGUI_SYS_USE_PREBUILT": "1"}, "requires feature `prebuilt`")

    def test_bindings_only_rejects_native_backend_shims(self):
        self.assert_failure({}, "bindings-only mode is incompatible with backend-shim", ("dear-imgui-sys/backend-shim-opengl3",))

    def test_source_mode_rejects_explicit_prebuilt_selectors(self):
        self.assert_failure({"IMGUI_SYS_BUILD_MODE": "source", "IMGUI_SYS_LIB_DIR": self.consumer}, "cannot be combined with a prebuilt selector")

    def test_docs_route_does_not_resolve_native_acquisition(self):
        overrides = {f"{prefix}_BUILD_MODE": "invalid-native-mode" for prefix in SYS_CRATES.values()}
        code, output = self.check({**overrides, "DOCS_RS": "1"})
        self.assertEqual(code, 0, output)

    def test_wasm_features_on_native_keep_native_bindings(self):
        features = tuple(f"{crate}/wasm" for crate in SYS_CRATES if crate != "dear-node-editor-sys")
        code, output = self.check(features=features)
        self.assertEqual(code, 0, output)

    def test_extension_manifest_changes_revalidate_incremental_builds(self):
        with TemporaryDirectory(prefix="dear-imgui-manifest-rerun-") as temporary:
            fixture = Path(temporary)
            (fixture / "src").mkdir()
            (fixture / "src/lib.rs").write_text("", encoding="utf-8")
            (fixture / "artifact/lib").mkdir(parents=True)
            (fixture / "Cargo.toml").write_text(
                '[package]\nname="manifest-rerun-contract"\nversion="0.0.0"\n'
                'edition="2024"\n[build-dependencies]\n'
                'build-support={package="dear-imgui-build-support",path='
                + json.dumps(str(ROOT / "tools/build-support"))
                + ',features=["binding-spec"]}\n[workspace]\n',
                encoding="utf-8",
            )
            (fixture / "build.rs").write_text('''use build_support::binding::{ExtensionArtifactProfile, ExtensionBinding};
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let profile = ExtensionArtifactProfile {
        extension: ExtensionBinding::ImPlot,
        safe_crate_name: "dear-implot".into(),
        sys_crate_name: "dear-implot-sys".into(),
        library_name: "dear_implot".into(),
        archive_name: "fixture.tar.gz".into(),
        version: "0.0.0".into(), candidate_sha: "a".repeat(40),
        target: "fixture-target".into(), link_type: "static".into(), crt: "".into(),
        features: vec!["wchar32".into()],
        core_artifact_identity_hash: "fnv1a64:1111111111111111".into(),
        extension_binding_identity_hash: "fnv1a64:2222222222222222".into(),
    };
    profile.validate_prebuilt_dir(std::path::Path::new("artifact/lib"))
        .expect("incremental manifest validation");
}
''', encoding="utf-8")
            manifest = fixture / "artifact/manifest.txt"
            valid = (
                "dear-implot-sys prebuilt\nversion=0.0.0\ncandidate_sha=" + "a" * 40
                + "\ntarget=fixture-target\nlink=static\ncrt=\nfeatures=wchar32\n"
                "extension=implot\nsafe_crate=dear-implot\nlibrary=dear_implot\n"
                "archive=fixture.tar.gz\ncore_artifact_identity=fnv1a64:1111111111111111\n"
                "extension_binding_identity=fnv1a64:2222222222222222\n"
            )
            manifest.write_text(valid, encoding="utf-8")
            env = {**os.environ, "CARGO_TARGET_DIR": str(ROOT / "target"), "CARGO_BUILD_JOBS": "2"}

            def check_manifest(success):
                result = subprocess.run(
                    ["cargo", "check", "--manifest-path", str(fixture / "Cargo.toml")],
                    env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                )
                self.assertEqual(result.returncode == 0, success, result.stdout)
                if not success:
                    self.assertIn("incremental manifest validation", result.stdout)
                return result.stdout

            check_manifest(True)
            unchanged = check_manifest(True)
            self.assertNotIn("Compiling manifest-rerun-contract", unchanged)
            time.sleep(0.02)
            manifest.write_text(valid.replace("version=0.0.0", "version=wrong"), encoding="utf-8")
            check_manifest(False)
            manifest.write_text(valid, encoding="utf-8")
            check_manifest(True)
            (fixture / "artifact/lib/manifest.txt").write_text("invalid", encoding="utf-8")
            check_manifest(False)
            (fixture / "artifact/lib/manifest.txt").unlink()
            check_manifest(True)
            manifest.unlink()
            check_manifest(False)


if __name__ == "__main__":
    unittest.main()
