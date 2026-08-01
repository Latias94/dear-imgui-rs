import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import upstream_contract  # noqa: E402


REVISION_A = "a" * 40
REVISION_B = "b" * 40


def fact(kind: str, name: str, value: object) -> upstream_contract.ApiFact:
    return upstream_contract.ApiFact("fixture", kind, name, value)


def snapshot(*facts: upstream_contract.ApiFact, revision: str = REVISION_A):
    return upstream_contract.ApiSnapshot({"source:fixture": revision}, facts)


def manifest(
    baseline: upstream_contract.ApiSnapshot,
    candidate: upstream_contract.ApiSnapshot,
    groups: tuple[upstream_contract.ReviewGroup, ...],
) -> upstream_contract.ReviewManifest:
    return upstream_contract.ReviewManifest(baseline.digest, candidate.digest, groups)


class UpstreamContractTests(unittest.TestCase):
    def test_generator_collection_covers_every_required_fact_kind(self):
        root = Path("fixture-output")
        values = {
            root / "definitions.json": {
                "igDo": [
                    {
                        "location": "imgui:100",
                        "ov_cimguiname": "igDo",
                        "cimguiname": "igDo",
                        "funcname": "Do",
                        "namespace": "ImGui",
                        "stname": "",
                        "ret": "void",
                        "argsT": [],
                        "signature": "()",
                        "defaults": {},
                    }
                ]
            },
            root / "constants.json": {"IMGUI_FIXTURE": "1"},
            root / "typedefs_dict.json": {"FixtureAlias": "int"},
            root / "structs_and_enums.json": {
                "enums": {
                    "FixtureEnum_": [
                        {"name": "FixtureEnum_Value", "value": "1", "calc_value": 1}
                    ]
                },
                "enumtypes": {"FixtureEnum": "int"},
                "locations": {
                    "FixtureEnum_": "imgui:101",
                    "FixtureStruct": "imgui:102",
                    "FixtureAlias": "imgui:103",
                },
                "structs": {"FixtureStruct": [{"name": "field", "type": "int"}]},
            },
        }

        facts = upstream_contract.collect_generator_facts(
            "fixture", root, ("imgui",), json_reader=values.__getitem__
        )

        self.assertEqual(
            {item.kind for item in facts},
            {
                "function",
                "constant",
                "enum",
                "enum-variant",
                "field",
                "layout",
                "typedef",
            },
        )

    def test_generator_collection_includes_layout_widths_bitfields_and_unlocated_typedefs(self):
        root = Path("fixture-output")
        values = {
            root / "definitions.json": {},
            root / "constants.json": {},
            root / "typedefs_dict.json": {"PublicAliasWithoutLocation": "unsigned int"},
            root / "structs_and_enums.json": {
                "enums": {},
                "enumtypes": {},
                "locations": {"Packed": "imgui:1"},
                "structs": {
                    "Packed": [
                        {
                            "name": "flags",
                            "type": "unsigned int",
                            "size": 4,
                            "bitfield": 3,
                        }
                    ]
                },
            },
        }

        facts = upstream_contract.collect_generator_facts(
            "fixture", root, ("imgui",), json_reader=values.__getitem__
        )
        by_id = {item.id: item for item in facts}

        self.assertEqual(
            by_id["fixture:field:Packed::flags"].value,
            {
                "type": "unsigned int",
                "template_type": None,
                "size": 4,
                "bitfield": 3,
            },
        )
        self.assertIn("fixture:typedef:PublicAliasWithoutLocation", by_id)

    def test_rust_binding_collection_tracks_the_final_public_surface(self):
        source = """// dear-imgui-rs-binding-provenance-v1 crate=fixture\n
#[repr(C)]
pub struct Opaque {
    _unused: [u8; 0],
}
#[repr(C)]
pub struct Config {
    pub count: u32,
}
pub const FIXTURE_FLAG: u32 = 3;
pub type Callback = ::std::option::Option<
    unsafe extern "C" fn(value: u32) -> bool,
>;
unsafe extern "C" {
    pub fn fixture_do(value: u32, callback: Callback) -> u32;
    pub fn fixture_callback(
        callback: ::std::option::Option<unsafe extern "C" fn(value: u32) -> bool>,
    );
}
"""

        facts = upstream_contract.collect_rust_bindings_facts(
            "fixture", Path("bindings.rs"), source_text=source
        )
        by_id = {item.id: item for item in facts}

        self.assertEqual(
            by_id["fixture:function:fixture_do"].value,
            {
                "abi": "C",
                "arguments": [
                    {"name": "value", "type": "u32"},
                    {"name": "callback", "type": "Callback"},
                ],
                "return_type": "u32",
            },
        )
        self.assertIn("fixture:typedef:Callback", by_id)
        self.assertIn("fixture:field:Opaque::_unused", by_id)
        self.assertIn("fixture:layout:Config", by_id)
        self.assertEqual(
            by_id["fixture:function:fixture_callback"].value["arguments"][0]["type"],
            '::std::option::Option<unsafe extern "C" fn(value: u32) -> bool>',
        )

    def test_rust_binding_collection_fails_closed_for_unknown_public_syntax(self):
        source = """// dear-imgui-rs-binding-provenance-v1 crate=fixture
pub union Unreviewed {
    value: u32,
}
"""

        with self.assertRaisesRegex(upstream_contract.ContractInputError, "unsupported public syntax"):
            upstream_contract.collect_rust_bindings_facts(
                "fixture", Path("bindings.rs"), source_text=source
            )

    def test_compare_reports_added_removed_changed_for_every_fact_kind(self):
        kinds = (
            "function",
            "constant",
            "enum",
            "enum-variant",
            "field",
            "layout",
            "typedef",
        )
        baseline = snapshot(
            *(fact(kind, f"removed-{kind}", 1) for kind in kinds),
            *(fact(kind, f"changed-{kind}", 1) for kind in kinds),
        )
        candidate = snapshot(
            *(fact(kind, f"added-{kind}", 1) for kind in kinds),
            *(fact(kind, f"changed-{kind}", 2) for kind in kinds),
            revision=REVISION_B,
        )

        deltas = upstream_contract.compare_snapshots(baseline, candidate)
        pairs = {(delta.operation, delta.kind) for delta in deltas}
        for kind in kinds:
            self.assertIn(("added", kind), pairs)
            self.assertIn(("removed", kind), pairs)
            self.assertIn(("changed", kind), pairs)
        self.assertIn(("changed", "source-pin"), pairs)

    def test_safe_runtime_shaped_delta_requires_compile_and_runtime_evidence(self):
        baseline = snapshot()
        candidate = snapshot(fact("function", "igDo", {"signature": "()"}))
        group = upstream_contract.ReviewGroup(
            "safe function",
            "reviewed",
            "safe-wrapper",
            "Wrapped by a tested safe API.",
            ("added:fixture:function:igDo",),
            ("Ui::do_it",),
            (),
        )
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(
                upstream_contract.ContractInputError, "compile usability evidence"
            ):
                upstream_contract.audit_review_manifest(
                    baseline, candidate, manifest(baseline, candidate, (group,)), repo_root=Path(directory)
                )

            evidence = Path(directory) / "proof.rs"
            evidence.write_text("compile marker\nruntime marker\n", encoding="utf-8")
            with_compile_only = upstream_contract.ReviewGroup(
                group.name,
                group.status,
                group.classification,
                group.rationale,
                group.items,
                group.safe_api,
                (upstream_contract.ReviewEvidence("compile", Path("proof.rs"), "compile marker"),),
            )
            with self.assertRaisesRegex(
                upstream_contract.ContractInputError, "runtime usability evidence"
            ):
                upstream_contract.audit_review_manifest(
                    baseline,
                    candidate,
                    manifest(baseline, candidate, (with_compile_only,)),
                    repo_root=Path(directory),
                )

            complete = upstream_contract.ReviewGroup(
                group.name,
                group.status,
                group.classification,
                group.rationale,
                group.items,
                group.safe_api,
                (
                    upstream_contract.ReviewEvidence("compile", Path("proof.rs"), "compile marker"),
                    upstream_contract.ReviewEvidence("runtime", Path("proof.rs"), "runtime marker"),
                ),
            )
            audit = upstream_contract.audit_review_manifest(
                baseline,
                candidate,
                manifest(baseline, candidate, (complete,)),
                repo_root=Path(directory),
            )
            self.assertEqual(audit.reviewed_items, 1)

    def test_missing_or_stale_decisions_fail_closed(self):
        baseline = snapshot()
        candidate = snapshot(fact("constant", "FIXTURE", "2"))
        no_groups = manifest(baseline, candidate, ())
        with self.assertRaisesRegex(upstream_contract.ContractInputError, "unreviewed"):
            upstream_contract.audit_review_manifest(baseline, candidate, no_groups)

        stale_group = upstream_contract.ReviewGroup(
            "stale",
            "reviewed",
            "internal",
            "No public Rust surface.",
            ("added:fixture:constant:DOES_NOT_EXIST",),
            (),
            (),
        )
        with self.assertRaisesRegex(upstream_contract.ContractInputError, "stale or unknown"):
            upstream_contract.audit_review_manifest(
                baseline, candidate, manifest(baseline, candidate, (stale_group,))
            )

    def test_evidence_marker_and_manifest_hashes_fail_closed(self):
        baseline = snapshot()
        candidate = snapshot(fact("constant", "FIXTURE", "2"))
        group = upstream_contract.ReviewGroup(
            "safe constant",
            "reviewed",
            "safe-alias",
            "A safe constant alias is exposed.",
            ("added:fixture:constant:FIXTURE",),
            ("Fixture",),
            (upstream_contract.ReviewEvidence("compile", Path("proof.rs"), "missing marker"),),
        )
        with tempfile.TemporaryDirectory() as directory:
            (Path(directory) / "proof.rs").write_text("other marker\n", encoding="utf-8")
            with self.assertRaisesRegex(upstream_contract.ContractInputError, "absent"):
                upstream_contract.audit_review_manifest(
                    baseline,
                    candidate,
                    manifest(baseline, candidate, (group,)),
                    repo_root=Path(directory),
                )

        stale = upstream_contract.ReviewManifest("0" * 64, candidate.digest, (group,))
        with self.assertRaisesRegex(upstream_contract.ContractInputError, "baseline hash is stale"):
            upstream_contract.audit_review_manifest(baseline, candidate, stale)

    def test_manifest_rejects_windows_and_noncanonical_evidence_paths(self):
        raw = {
            "schema": upstream_contract.REVIEW_SCHEMA,
            "baseline_sha256": "0" * 64,
            "candidate_sha256": "1" * 64,
            "groups": [
                {
                    "name": "fixture",
                    "status": "reviewed",
                    "classification": "safe-alias",
                    "rationale": "fixture",
                    "items": ["added:fixture:constant:Flag"],
                    "safe_api": ["fixture"],
                    "evidence": [
                        {"kind": "compile", "path": "C:/outside/proof.rs", "contains": "x"}
                    ],
                }
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "review.json"
            for invalid in (
                "C:/outside/proof.rs",
                "C:\\outside\\proof.rs",
                "C:outside/proof.rs",
                "\\\\server\\share\\proof.rs",
                "\\outside\\proof.rs",
                "tests\\..\\..\\outside\\proof.rs",
                "../outside/proof.rs",
                "proof//again.rs",
            ):
                with self.subTest(path=invalid):
                    raw["groups"][0]["evidence"][0]["path"] = invalid
                    path.write_text(json.dumps(raw), encoding="utf-8")
                    with self.assertRaisesRegex(
                        upstream_contract.ContractInputError, "repository-relative path"
                    ):
                        upstream_contract.load_review_manifest(path)

    def test_audit_revalidates_direct_evidence_paths_and_symlink_containment(self):
        baseline = snapshot()
        candidate = snapshot(fact("constant", "Flag", "1"))

        def safe_group(evidence: upstream_contract.ReviewEvidence):
            return upstream_contract.ReviewGroup(
                "fixture",
                "reviewed",
                "safe-alias",
                "fixture",
                ("added:fixture:constant:Flag",),
                ("fixture",),
                (evidence,),
            )

        direct_escape = manifest(
            baseline,
            candidate,
            (safe_group(upstream_contract.ReviewEvidence("compile", Path("../outside.rs"), "proof")),),
        )
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "repo"
            repo.mkdir()
            with self.assertRaisesRegex(upstream_contract.ContractInputError, "repository-relative path"):
                upstream_contract.audit_review_manifest(
                    baseline, candidate, direct_escape, repo_root=repo
                )

            inside = repo / "inside.rs"
            inside.write_text("proof", encoding="utf-8")
            link = repo / "inside-link.rs"
            try:
                link.symlink_to(inside.name)
            except OSError as error:
                self.skipTest(f"symbolic links are unavailable in this test environment: {error}")
            accepted = manifest(
                baseline,
                candidate,
                (safe_group(upstream_contract.ReviewEvidence("compile", Path("inside-link.rs"), "proof")),),
            )
            audit = upstream_contract.audit_review_manifest(
                baseline, candidate, accepted, repo_root=repo
            )
            self.assertEqual(audit.reviewed_items, 1)

            outside = Path(directory) / "outside.rs"
            outside.write_text("proof", encoding="utf-8")
            escape = repo / "escape.rs"
            try:
                escape.symlink_to(outside)
            except OSError as error:
                self.skipTest(f"symbolic links are unavailable in this test environment: {error}")
            escaped = manifest(
                baseline,
                candidate,
                (safe_group(upstream_contract.ReviewEvidence("compile", Path("escape.rs"), "proof")),),
            )
            with self.assertRaisesRegex(upstream_contract.ContractInputError, "outside the repository"):
                upstream_contract.audit_review_manifest(
                    baseline, candidate, escaped, repo_root=repo
                )

    def test_review_template_lists_each_delta_once(self):
        baseline = snapshot(fact("field", "Fixture::value", "int"))
        candidate = snapshot(
            fact("field", "Fixture::value", "float"),
            fact("layout", "Fixture", []),
        )
        template = upstream_contract.review_template(baseline, candidate)
        items = [item for group in template["groups"] for item in group["items"]]
        self.assertEqual(items, [delta.key for delta in upstream_contract.compare_snapshots(baseline, candidate)])
        self.assertTrue(all(group["status"] == "pending" for group in template["groups"]))

    def test_snapshot_loader_rejects_unsorted_facts(self):
        raw = snapshot(fact("constant", "A", "1"), fact("constant", "B", "2")).as_json()
        raw["facts"] = list(reversed(raw["facts"]))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "snapshot.json"
            path.write_text(json.dumps(raw), encoding="utf-8")
            with self.assertRaisesRegex(upstream_contract.ContractInputError, "sorted"):
                upstream_contract.load_snapshot(path)

    def test_snapshot_loader_rejects_duplicate_facts(self):
        raw = snapshot(fact("constant", "A", "1"), fact("constant", "B", "2")).as_json()
        raw["facts"].append(raw["facts"][1])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "snapshot.json"
            path.write_text(json.dumps(raw), encoding="utf-8")
            with self.assertRaisesRegex(upstream_contract.ContractInputError, "duplicate fact"):
                upstream_contract.load_snapshot(path)

    def test_repository_contract_rejects_live_source_drift_before_loading_decisions(self):
        baseline = snapshot()
        accepted = snapshot(fact("constant", "accepted", "1"))
        live = snapshot(
            fact("constant", "accepted", "1"),
            fact("constant", "drifted", "2"),
        )

        with (
            mock.patch.object(
                upstream_contract, "load_snapshot", side_effect=(baseline, accepted)
            ),
            mock.patch.object(
                upstream_contract, "collect_repository_snapshot", return_value=live
            ),
            mock.patch.object(upstream_contract, "load_review_manifest") as load_manifest,
        ):
            with self.assertRaisesRegex(
                upstream_contract.ContractInputError, "differ from the accepted snapshot"
            ):
                upstream_contract.audit_repository_contract(
                    repo_root=Path("fixture-repository"),
                    baseline_path=Path("baseline.json"),
                    snapshot_path=Path("snapshot.json"),
                    decisions_path=Path("decisions.json"),
                )

        load_manifest.assert_not_called()

    def test_repository_contract_loads_decisions_after_live_source_matches(self):
        baseline = snapshot()
        accepted = snapshot()
        manifest = upstream_contract.ReviewManifest(
            baseline.digest,
            accepted.digest,
            (),
        )

        with (
            mock.patch.object(
                upstream_contract, "load_snapshot", side_effect=(baseline, accepted)
            ),
            mock.patch.object(
                upstream_contract, "collect_repository_snapshot", return_value=accepted
            ),
            mock.patch.object(
                upstream_contract, "load_review_manifest", return_value=manifest
            ) as load_manifest,
        ):
            audit = upstream_contract.audit_repository_contract(
                repo_root=Path("fixture-repository"),
                baseline_path=Path("baseline.json"),
                snapshot_path=Path("snapshot.json"),
                decisions_path=Path("decisions.json"),
            )

        self.assertEqual(audit.reviewed_items, 0)
        load_manifest.assert_called_once_with(Path("decisions.json"))


if __name__ == "__main__":
    unittest.main()
