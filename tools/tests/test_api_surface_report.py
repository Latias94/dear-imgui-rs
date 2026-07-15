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
        self.manifest = root / "Cargo.toml"
        self.policy = root / "policy.json"
        self.snapshot = root / "snapshot.json"
        self.rust_source.mkdir()

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

        self.assertEqual(len(declarations), 748)
        self.assertEqual(audit.unexpected, frozenset())
        self.assertEqual(audit.stale_policy, frozenset())
        self.assertFalse(drift.has_drift())
        self.assertEqual(
            len(audit.aliased) + len(audit.policy_decided),
            len(groups),
        )
        self.assertNotIn(
            "safe-equivalent",
            {decision.classification for decision in policy.values()},
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
