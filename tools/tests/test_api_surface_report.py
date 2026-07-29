import contextlib
import copy
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
sys.path.insert(0, str(TOOLS_DIR))

import api_surface_report  # noqa: E402


CIMGUI_REVISION = "1" * 40
IMGUI_REVISION = "2" * 40


def declaration(
    funcname="Foo",
    symbol="igFoo",
    signature="()",
    namespace="ImGui",
    location="imgui:1",
    ret="void",
):
    return {
        "args": signature,
        "argsT": [],
        "cimguiname": symbol.split("_")[0],
        "defaults": {},
        "funcname": funcname,
        "location": location,
        "namespace": namespace,
        "ov_cimguiname": symbol,
        "ret": ret,
        "signature": signature,
        "stname": "" if namespace == "ImGui" else namespace,
    }


def empty_policy():
    return {"schema_version": 2, "scope": "ImGui", "groups": []}


class CliFixture:
    def __init__(self, root: Path):
        self.root = root
        self.definitions = root / "definitions.json"
        self.rust_source = root / "rust"
        self.manifest = root / "source-manifest.toml"
        self.policy = root / "policy.json"
        self.snapshot = root / "snapshot.json"
        self.repository = root / "repository"
        self.repository_manifest = self.repository / "Cargo.toml"
        self.safe_source = self.repository / "safe" / "src" / "lib.rs"
        self.sys_source = self.repository / "raw-sys" / "src" / "lib.rs"
        self.rust_source.mkdir()
        self.safe_source.parent.mkdir(parents=True)
        self.sys_source.parent.mkdir(parents=True)

    def write_repository(
        self,
        safe_source="",
        sys_source='unsafe extern "C" { pub fn igRawEscape(); }\n',
        safe_features="",
        safe_name="fixture-safe",
    ):
        self.repository_manifest.write_text(
            '[workspace]\nmembers = ["safe", "raw-sys"]\nresolver = "2"\n',
            encoding="utf-8",
        )
        (self.safe_source.parents[1] / "Cargo.toml").write_text(
            "[package]\n"
            f'name = "{safe_name}"\n'
            'version = "0.0.0"\n'
            f"{safe_features}",
            encoding="utf-8",
        )
        (self.sys_source.parents[1] / "Cargo.toml").write_text(
            "[package]\n"
            'name = "fixture-sys"\n'
            'version = "0.0.0"\n',
            encoding="utf-8",
        )
        self.safe_source.write_text(safe_source, encoding="utf-8")
        self.sys_source.write_text(sys_source, encoding="utf-8")

    def write(
        self,
        definitions=None,
        rust_source='#[doc(alias = "Foo")]\npub fn foo() {}\n',
        policy=None,
    ):
        if definitions is None:
            definitions = {"Foo": [declaration()]}
        if policy is None:
            policy = empty_policy()
        self.definitions.write_text(json.dumps(definitions), encoding="utf-8")
        (self.rust_source / "lib.rs").write_text(rust_source, encoding="utf-8")
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        self.manifest.write_text(
            "[package]\n"
            'name = "fixture"\n'
            'version = "0.0.0"\n'
            "\n[package.metadata.dear-imgui-sources]\n"
            f'cimgui-revision = "{CIMGUI_REVISION}"\n'
            f'imgui-revision = "{IMGUI_REVISION}"\n',
            encoding="utf-8",
        )
        self.write_repository()
        loaded = api_surface_report._load_public_declarations(self.definitions)
        api_surface_report._write_snapshot(
            self.snapshot,
            loaded,
            {"cimgui": CIMGUI_REVISION, "imgui": IMGUI_REVISION},
        )

    def args(self):
        return [
            "--definitions",
            str(self.definitions),
            "--rust-source",
            str(self.rust_source),
            "--manifest",
            str(self.manifest),
            "--policy",
            str(self.policy),
            "--snapshot",
            str(self.snapshot),
            "--repository-manifest",
            str(self.repository_manifest),
            "--check",
        ]

    def run(self):
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = api_surface_report.main(self.args())
        return result, stdout.getvalue(), stderr.getvalue()


class ApiSurfaceReportTests(unittest.TestCase):
    def test_repository_policy_and_snapshot_cover_the_current_surface(self):
        declarations = api_surface_report._load_public_declarations(
            api_surface_report.DEFS_JSON
        )
        groups = api_surface_report._group_top_level_imgui(declarations)
        aliases = api_surface_report._collect_doc_aliases(
            api_surface_report._iter_rs_files(api_surface_report.DEAR_IMGUI_SRC)
        )
        policy = api_surface_report._load_policy(api_surface_report.POLICY_JSON)
        revisions = api_surface_report._load_source_revisions(
            api_surface_report.MANIFEST_TOML
        )
        snapshot_revisions, snapshot_values = api_surface_report._load_snapshot(
            api_surface_report.SNAPSHOT_JSON
        )

        audit = api_surface_report._audit_surface(groups, aliases, policy)
        drift = api_surface_report._compare_snapshot(
            declarations,
            revisions,
            snapshot_values,
            snapshot_revisions,
        )
        _, packages, source_violations = api_surface_report._audit_source_policy(
            api_surface_report.REPOSITORY_MANIFEST
        )

        self.assertEqual(len(declarations), 748)
        self.assertEqual(audit.unexpected, frozenset())
        self.assertEqual(audit.stale_policy, frozenset())
        self.assertFalse(drift.has_drift())
        self.assertGreaterEqual(len(packages), 30)
        self.assertEqual(source_violations, ())
        self.assertEqual(
            len(audit.aliased) + len(audit.policy_decided),
            len(groups),
        )
        self.assertNotIn(
            "safe-equivalent",
            {decision.classification for decision in policy.values()},
        )
        self.assertEqual(
            {
                name
                for name, decision in policy.items()
                if decision.classification == "unsafe-wrapper"
            },
            set(),
        )

    def test_alias_collector_accepts_only_public_safe_items(self):
        source = r'''
// #[doc(alias = "LineComment")]
/* #[doc(alias = "BlockComment")] */
const NORMAL: &str = "#[doc(alias = \\"NormalString\\")] pub fn fake() {}";
const RAW: &str = r###"#[doc(alias = "RawString")] pub fn fake() {}"###;

#[doc(alias = "PrivateFn")]
fn private_fn() {}

#[doc(alias = "CrateOnly")]
pub(crate) fn crate_only() {}

#[cfg(test)]
#[doc(alias = "CfgTest")]
pub fn cfg_test() {}

#[cfg_attr(test, doc(alias = "CfgAttrTest"))]
pub fn cfg_attr_test() {}

#[doc(alias = "UnsafeFn")]
pub unsafe fn unsafe_fn() {}

#[doc(alias = "HiddenFn")]
#[doc(hidden)]
pub fn hidden_fn() {}

mod private_module {
    #[doc(alias = "PrivateModuleFn")]
    pub fn visible_only_inside_parent() {}
}

#[cfg(test)]
mod tests {
    #[doc(alias = "TestModuleFn")]
    pub fn helper() {}
}

#[doc(alias = "SafeFn")]
pub fn safe_fn<'a>(value: &'a str) -> &'a str { value }

#[doc(alias = "SafeType")]
pub struct SafeType;

impl SafeType {
    #[doc(alias = "SafeMethod")]
    pub fn safe_method(&self) {}
}

pub mod exported {
    #[doc(alias = "NestedSafe")]
    pub fn nested_safe() {}
}

create_token!(
    #[doc(alias = "MacroToken")]
    pub struct Token<'ui>;
    drop { () }
);
'''
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "fixture.rs"
            path.write_text(source, encoding="utf-8")
            aliases = api_surface_report._collect_doc_aliases([path])

        self.assertEqual(
            aliases,
            {"SafeFn", "SafeType", "SafeMethod", "NestedSafe", "MacroToken"},
        )

    def test_removed_source_inventory_rejects_each_structural_rule(self):
        cases = {
            "context-frame-with": ("pub fn f() { frame_with(); }\n", "fixture-safe", ""),
            "renderer-compat-path": (
                "pub fn f() { render::renderer::legacy(); }\n",
                "fixture-safe",
                "",
            ),
            "renderer-compat-module": (
                "pub mod renderer {}\n",
                "dear-imgui-rs",
                "",
            ),
            "glyph-ranges-builder": (
                "pub fn f(_: GlyphRangesBuilder) {}\n",
                "fixture-safe",
                "",
            ),
            "glyph-ranges-type": (
                "pub fn f(_: GlyphRanges) {}\n",
                "fixture-safe",
                "",
            ),
            "glyph-ranges-path": (
                "pub use fonts::glyph_ranges::Legacy;\n",
                "fixture-safe",
                "",
            ),
            "glyph-ranges-module": (
                "pub use glyph_ranges::*;\n",
                "dear-imgui-rs",
                "",
            ),
            "selectable-new": (
                "pub fn f() { Selectable::new(); }\n",
                "fixture-safe",
                "",
            ),
            "horizontal-slider-new": (
                "pub struct Slider; impl Slider {\n"
                "    pub fn new(ui: &Ui, label: &str, min: f32, max: f32) -> Self { Self }\n"
                "}\n",
                "fixture-safe",
                "",
            ),
            "input-flags": ("pub type Alias = InputFlags;\n", "fixture-safe", ""),
            "arrow-direction": ("pub use x::ArrowDirection;\n", "fixture-safe", ""),
            "texture-data-new": (
                "pub fn f() { TextureData::new(); }\n",
                "fixture-safe",
                "",
            ),
            "create-texture-ref": (
                "pub fn f() { create_texture_ref(); }\n",
                "fixture-safe",
                "",
            ),
            "wgpu-texture-manager-mut": (
                "pub fn f() { renderer.texture_manager_mut(); }\n",
                "fixture-safe",
                "",
            ),
            "sdl3-update-gp3-texture": (
                "pub fn f() { update_gp3_texture(); }\n",
                "fixture-safe",
                "",
            ),
            "sdl3-init-for-platform-sdl-gpu": (
                "pub fn f() { init_for_platform_sdl_gpu(); }\n",
                "fixture-safe",
                "",
            ),
            "into-imgui-error": (
                "pub trait IntoImGuiError {}\n",
                "fixture-safe",
                "",
            ),
            "into-imgui-error-method": (
                "pub fn f() { value.into_imgui_error(); }\n",
                "fixture-safe",
                "",
            ),
            "safe-compat-ffi": ("mod compat_ffi {}\n", "fixture-safe", ""),
            "safe-draw-callback-builder": (
                "pub fn f() { add_callback_safe(); }\n",
                "fixture-safe",
                "",
            ),
            "implot3d-validation-helpers": (
                "pub fn validate_nonempty() {}\n",
                "dear-implot3d",
                "",
            ),
            "examples-sdl3-backends": (
                "pub fn clean() {}\n",
                "fixture-safe",
                '\n[features]\nsdl3-backends = []\n',
            ),
        }

        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            for expected_rule, (source, package, features) in cases.items():
                with self.subTest(rule=expected_rule):
                    fixture.write_repository(
                        safe_source=source,
                        safe_features=features,
                        safe_name=package,
                    )
                    _, _, violations = api_surface_report._audit_source_policy(
                        fixture.repository_manifest
                    )
                    self.assertEqual(
                        {violation.rule for violation in violations},
                        {expected_rule},
                    )

    def test_removed_source_inventory_normalizes_raw_identifiers(self):
        cases = {
            "texture-data-new": (
                "pub fn f() { TextureData::r#new(); }\n",
                "fixture-safe",
            ),
            "renderer-compat-module": (
                "pub mod r#renderer {}\n",
                "dear-imgui-rs",
            ),
            "context-frame-with": (
                "pub fn r#frame_with() {}\n",
                "fixture-safe",
            ),
            "safe-draw-callback-builder": (
                "pub fn f() { r#add_callback_safe(); }\n",
                "fixture-safe",
            ),
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            for expected_rule, (source, package) in cases.items():
                with self.subTest(rule=expected_rule):
                    fixture.write_repository(safe_source=source, safe_name=package)
                    _, _, violations = api_surface_report._audit_source_policy(
                        fixture.repository_manifest
                    )
                    self.assertEqual(
                        {violation.rule for violation in violations},
                        {expected_rule},
                    )

    def test_rust_string_decoder_handles_cooked_raw_and_continuation_escapes(self):
        self.assertEqual(
            api_surface_report._decode_rust_string(r'"\x49\u{6d}\n\t\\\"\0"'),
            "Im\n\t" + "\\" + '"' + "\0",
        )
        continuation = '"left' + "\\" + "\n    right" + '"'
        self.assertEqual(
            api_surface_report._decode_rust_string(continuation), "leftright"
        )
        self.assertEqual(
            api_surface_report._decode_rust_string(r'r##"\x49"##'), r"\x49"
        )

    def test_source_policy_rejects_duplicate_safe_extern_from_generated_sys(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            fixture.write_repository(
                safe_source='unsafe extern "C" { pub fn igDuplicate(); }\n',
                sys_source='unsafe extern "C" { pub fn igDuplicate(); }\n',
            )
            result, _, stderr = fixture.run()

        self.assertEqual(result, 1)
        self.assertIn("duplicate-safe-extern", stderr)
        self.assertIn("igDuplicate", stderr)

        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write_repository(
                safe_source=(
                    'mod ffi { unsafe extern "C" { '
                    "pub fn ImPlot_GetPlotPos(); } }\n"
                ),
                sys_source="pub fn unrelated() {}\n",
            )
            _, _, violations = api_surface_report._audit_source_policy(
                fixture.repository_manifest
            )

        self.assertEqual(
            {(violation.rule, violation.symbol) for violation in violations},
            {("duplicate-safe-extern", "ImPlot_GetPlotPos")},
        )

        for escaped_link_name in (
            r'"\x49mPlot_GetPlotPos"',
            r'"\u{49}mPlot_GetPlotPos"',
            'r#"ImPlot_GetPlotPos"#',
        ):
            with self.subTest(link_name=escaped_link_name):
                with tempfile.TemporaryDirectory() as temp_dir:
                    fixture = CliFixture(Path(temp_dir))
                    fixture.write_repository(
                        safe_source=(
                            'unsafe extern "C" { #[link_name = '
                            f"{escaped_link_name}] pub fn renamed_plot_pos(); }}\n"
                        ),
                        sys_source="pub fn unrelated() {}\n",
                    )
                    _, _, violations = api_surface_report._audit_source_policy(
                        fixture.repository_manifest
                    )

                self.assertEqual(
                    {(violation.rule, violation.symbol) for violation in violations},
                    {("duplicate-safe-extern", "ImPlot_GetPlotPos")},
                )

        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write_repository(
                safe_source=(
                    'unsafe extern "C" { #[link_name = "ImPlot_GetPlotPos"] '
                    "pub fn renamed_plot_pos(); }\n"
                ),
                sys_source="pub fn unrelated() {}\n",
            )
            _, _, violations = api_surface_report._audit_source_policy(
                fixture.repository_manifest
            )

        self.assertEqual(
            {(violation.rule, violation.symbol) for violation in violations},
            {("duplicate-safe-extern", "ImPlot_GetPlotPos")},
        )

    def test_source_policy_fails_closed_on_malformed_link_name_escape(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            fixture.write_repository(
                safe_source=(
                    r'unsafe extern "C" { #[link_name = "\q"] pub fn renamed(); }'
                    "\n"
                )
            )
            result, _, stderr = fixture.run()

        self.assertEqual(result, 2)
        self.assertIn("API surface input error", stderr)
        self.assertIn("unsupported Rust string escape", stderr)

    def test_source_policy_preserves_raw_sys_and_callback_escape_hatches(self):
        source = r'''
// frame_with Selectable::new TextureData::new compat_ffi
const TEXT: &str = "InputFlags render::renderer sdl3-backends";
const ESCAPED_TEXT: &str = "\x49nputFlags";
const RAW_TEXT: &str = r#"TextureData::r#new"#;

pub fn use_raw() {
    unsafe { fixture_sys::igRawEscape(); }
    unsafe { fixture_sys::ImPlot_GetPlotPos(); }
    let _ = raw_font_config.GlyphRanges;
    VerticalSlider::new();
    AngleSlider::new();
    OwnedTextureData::new();
}

unsafe extern "C" fn callback() {}
unsafe extern "C" { pub fn local_cpp_shim(); }
pub(crate) fn validate_nonempty() {}
'''
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            fixture.write_repository(
                safe_source=source,
                safe_name="dear-implot3d",
                sys_source=(
                    'unsafe extern "C" { pub fn igRawEscape(); '
                    "pub fn ImPlot_GetPlotPos(); }\n"
                ),
            )
            result, _, stderr = fixture.run()

        self.assertEqual(result, 0)
        self.assertEqual(stderr, "")

    def test_source_policy_prunes_unmaintained_trees_before_discovery(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write_repository()
            for excluded in (".git", "repo-ref", "target", "third-party"):
                excluded_root = fixture.repository / excluded / "nested"
                excluded_root.mkdir(parents=True)
                (excluded_root / "Cargo.toml").write_text("not valid TOML {", encoding="utf-8")
                (excluded_root / "forbidden.rs").write_text(
                    "pub fn frame_with() {}\n", encoding="utf-8"
                )

            _, packages, violations = api_surface_report._audit_source_policy(
                fixture.repository_manifest
            )

        self.assertEqual(
            {package.name for package in packages}, {"fixture-safe", "fixture-sys"}
        )
        self.assertEqual(violations, ())

    def test_policy_schema_is_fail_closed(self):
        valid_group = {
            "classification": "deferred-design",
            "name": "deferred",
            "reason": "Needs a lifetime design.",
            "functions": ["Foo"],
        }
        invalid_policies = {
            "boolean schema": {
                "schema_version": True,
                "scope": "ImGui",
                "groups": [valid_group],
            },
            "unknown top key": {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [valid_group],
                "extra": True,
            },
            "unknown group key": {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [{**valid_group, "extra": True}],
            },
            "safe equivalent": {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [{**valid_group, "classification": "safe-equivalent"}],
            },
            "empty functions": {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [{**valid_group, "functions": []}],
            },
            "duplicate functions": {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [{**valid_group, "functions": ["Foo", "Foo"]}],
            },
            "duplicate group names": {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [valid_group, {**valid_group, "functions": ["Bar"]}],
            },
        }

        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "policy.json"
            for name, policy in invalid_policies.items():
                with self.subTest(name=name):
                    path.write_text(json.dumps(policy), encoding="utf-8")
                    with self.assertRaises(api_surface_report.InputError):
                        api_surface_report._load_policy(path)

    def test_policy_rejects_duplicate_function_decisions(self):
        policy = {
            "schema_version": 2,
            "scope": "ImGui",
            "groups": [
                {
                    "classification": "deferred-design",
                    "name": "first",
                    "reason": "first decision",
                    "functions": ["Foo"],
                },
                {
                    "classification": "intentional-sys-only",
                    "name": "second",
                    "reason": "second decision",
                    "functions": ["Foo"],
                },
            ],
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "policy.json"
            path.write_text(json.dumps(policy), encoding="utf-8")
            with self.assertRaisesRegex(api_surface_report.InputError, "both 'first' and 'second'"):
                api_surface_report._load_policy(path)

    def test_cli_clean_surface_returns_zero(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            result, stdout, stderr = fixture.run()

        self.assertEqual(result, 0)
        self.assertIn("checks passed", stdout)
        self.assertEqual(stderr, "")

    def test_cli_unexpected_and_stale_policy_return_one(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write(rust_source="pub fn unrelated() {}\n")
            result, _, stderr = fixture.run()
            self.assertEqual(result, 1)
            self.assertIn("Unexpected missing", stderr)

            policy = {
                "schema_version": 2,
                "scope": "ImGui",
                "groups": [
                    {
                        "classification": "deferred-design",
                        "name": "stale",
                        "reason": "Covered directly now.",
                        "functions": ["Foo"],
                    }
                ],
            }
            fixture.policy.write_text(json.dumps(policy), encoding="utf-8")
            (fixture.rust_source / "lib.rs").write_text(
                '#[doc(alias = "Foo")]\npub fn foo() {}\n', encoding="utf-8"
            )
            result, _, stderr = fixture.run()
            self.assertEqual(result, 1)
            self.assertIn("Stale API surface policy", stderr)

    def test_cli_detects_overload_signature_and_namespace_drift(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()

            definitions = json.loads(fixture.definitions.read_text(encoding="utf-8"))
            definitions["Foo"].append(
                declaration(symbol="igFoo_Int", signature="(int)")
            )
            fixture.definitions.write_text(json.dumps(definitions), encoding="utf-8")
            result, _, stderr = fixture.run()
            self.assertEqual(result, 1)
            self.assertIn("added declarations", stderr)
            self.assertIn("igFoo_Int", stderr)

            fixture.write()
            definitions = json.loads(fixture.definitions.read_text(encoding="utf-8"))
            definitions["Foo"][0]["signature"] = "(float)"
            definitions["Foo"][0]["args"] = "(float value)"
            fixture.definitions.write_text(json.dumps(definitions), encoding="utf-8")
            result, _, stderr = fixture.run()
            self.assertEqual(result, 1)
            self.assertIn("changed declarations", stderr)

            fixture.write()
            definitions = json.loads(fixture.definitions.read_text(encoding="utf-8"))
            definitions["Draw"] = [
                declaration(
                    funcname="AddThing",
                    symbol="ImDrawList_AddThing",
                    namespace="ImDrawList",
                )
            ]
            fixture.definitions.write_text(json.dumps(definitions), encoding="utf-8")
            result, _, stderr = fixture.run()
            self.assertEqual(result, 1)
            self.assertIn("ImDrawList_AddThing", stderr)

    def test_cli_ignores_generator_line_number_only_changes(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            definitions = json.loads(fixture.definitions.read_text(encoding="utf-8"))
            definitions["Foo"][0]["location"] = "imgui:9999"
            fixture.definitions.write_text(json.dumps(definitions), encoding="utf-8")
            result, _, stderr = fixture.run()

        self.assertEqual(result, 0)
        self.assertEqual(stderr, "")

    def test_cli_revision_drift_returns_one(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            fixture.manifest.write_text(
                "[package]\n"
                'name = "fixture"\n'
                'version = "0.0.0"\n'
                "\n[package.metadata.dear-imgui-sources]\n"
                f'cimgui-revision = "{"3" * 40}"\n'
                f'imgui-revision = "{IMGUI_REVISION}"\n',
                encoding="utf-8",
            )
            result, _, stderr = fixture.run()

        self.assertEqual(result, 1)
        self.assertIn("source revision drift", stderr)

    def test_cli_invalid_inputs_return_two(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            cases = {
                "invalid policy json": (fixture.policy, "{"),
                "invalid definitions utf8": (fixture.definitions, b"\xff"),
                "invalid snapshot schema": (
                    fixture.snapshot,
                    json.dumps({"schema_version": True}),
                ),
            }
            for name, (path, content) in cases.items():
                with self.subTest(name=name):
                    fixture.write()
                    if isinstance(content, bytes):
                        path.write_bytes(content)
                    else:
                        path.write_text(content, encoding="utf-8")
                    result, _, stderr = fixture.run()
                    self.assertEqual(result, 2)
                    self.assertIn("API surface input error", stderr)

            fixture.write()
            fixture.snapshot.unlink()
            result, _, stderr = fixture.run()
            self.assertEqual(result, 2)
            self.assertIn("API surface input error", stderr)

    def test_snapshot_schema_rejects_unknown_and_malformed_fields(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = CliFixture(Path(temp_dir))
            fixture.write()
            snapshot = json.loads(fixture.snapshot.read_text(encoding="utf-8"))
            invalid_values = []

            unknown = copy.deepcopy(snapshot)
            unknown["extra"] = True
            invalid_values.append(unknown)

            malformed = copy.deepcopy(snapshot)
            malformed["declarations"][0]["arguments"] = "not an array"
            invalid_values.append(malformed)

            boolean_schema = copy.deepcopy(snapshot)
            boolean_schema["schema_version"] = True
            invalid_values.append(boolean_schema)

            for value in invalid_values:
                fixture.snapshot.write_text(json.dumps(value), encoding="utf-8")
                with self.assertRaises(api_surface_report.InputError):
                    api_surface_report._load_snapshot(fixture.snapshot)


if __name__ == "__main__":
    unittest.main()
