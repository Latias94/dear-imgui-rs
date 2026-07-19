import hashlib
import io
import json
import subprocess
import sys
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock, patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_TOOLS = REPO_ROOT / "tools" / "ci"
sys.path.insert(0, str(CI_TOOLS))

import release_evidence  # noqa: E402


SHA = "0123456789abcdef0123456789abcdef01234567"
OTHER_SHA = "89abcdef0123456789abcdef0123456789abcdef"
FIXTURE_REQUIREMENTS = (
    release_evidence.EvidenceRequirement("artifacts", "metadata.json"),
    release_evidence.EvidenceRequirement("logs", "run.log"),
)


class CandidateShaTests(unittest.TestCase):
    def test_accepts_only_full_lowercase_sha(self):
        self.assertEqual(release_evidence.parse_candidate_sha(SHA), SHA)
        for invalid in (SHA[:-1], SHA + "0", SHA.upper(), f" {SHA}", f"{SHA}\n"):
            with self.subTest(invalid=invalid), self.assertRaises(
                release_evidence.EvidenceError
            ):
                release_evidence.parse_candidate_sha(invalid)

    def test_resolves_head_with_injected_runner_and_verifies_expected(self):
        runner = Mock(
            return_value=subprocess.CompletedProcess(
                args=[], returncode=0, stdout=f"{SHA}\n", stderr=""
            )
        )

        actual = release_evidence.resolve_candidate_sha(
            Path("repo"), SHA, runner=runner
        )

        self.assertEqual(actual, SHA)
        runner.assert_called_once_with(
            ["git", "rev-parse", "--verify", "HEAD"],
            cwd=Path("repo"),
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )

    def test_rejects_wrong_or_malformed_head(self):
        for stdout in (f"{OTHER_SHA}\n", f"{SHA.upper()}\n", f"{SHA}\nextra\n"):
            runner = Mock(
                return_value=subprocess.CompletedProcess(
                    args=[], returncode=0, stdout=stdout, stderr=""
                )
            )
            with self.subTest(stdout=stdout), self.assertRaises(
                release_evidence.EvidenceError
            ):
                release_evidence.resolve_candidate_sha(
                    Path("repo"), SHA, runner=runner
                )

    def test_verify_candidate_cli_accepts_the_checked_out_workflow_revision(self):
        root = Path("repo")
        with patch.object(
            release_evidence, "resolve_candidate_sha", return_value=SHA
        ) as resolver:
            result = release_evidence.main(
                [
                    "verify-candidate",
                    "--repo-root",
                    str(root),
                    "--candidate-sha",
                    SHA,
                ]
            )

        self.assertEqual(result, 0)
        resolver.assert_called_once_with(root, SHA)

    def test_verify_candidate_cli_fails_closed_on_identity_mismatch(self):
        root = Path("repo")
        diagnostic = io.StringIO()
        mismatch = release_evidence.EvidenceError(
            f"candidate HEAD mismatch: expected {SHA}, found {OTHER_SHA}"
        )
        with (
            patch.object(
                release_evidence, "resolve_candidate_sha", side_effect=mismatch
            ) as resolver,
            redirect_stderr(diagnostic),
            self.assertRaises(SystemExit) as raised,
        ):
            release_evidence.main(
                [
                    "verify-candidate",
                    "--repo-root",
                    str(root),
                    "--candidate-sha",
                    SHA,
                ]
            )

        self.assertEqual(raised.exception.code, 2)
        resolver.assert_called_once_with(root, SHA)
        self.assertIn("candidate HEAD mismatch", diagnostic.getvalue())


class CellEvidenceTests(unittest.TestCase):
    def test_writes_stable_lf_json_with_streamed_checksums(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "artifacts" / "library.bin"
            log = root / "logs" / "run.log"
            artifact.parent.mkdir()
            log.parent.mkdir()
            artifact.write_bytes(b"a" * (2 * 1024 * 1024 + 17))
            log.write_text("line one\nline two\n", encoding="utf-8", newline="\n")
            output = root / "cell.json"

            record = release_evidence.write_cell_evidence(
                output,
                cell_id="linux-runtime",
                candidate_sha=SHA,
                conclusion="success",
                artifacts=[artifact],
                logs=[log],
                target="x86_64-unknown-linux-gnu",
                evidence_root=root,
            )
            first = output.read_bytes()
            release_evidence.write_cell_evidence(
                output,
                cell_id="linux-runtime",
                candidate_sha=SHA,
                conclusion="success",
                artifacts=[artifact],
                logs=[log],
                target="x86_64-unknown-linux-gnu",
                evidence_root=root,
            )

            self.assertEqual(output.read_bytes(), first)
            self.assertTrue(first.endswith(b"\n"))
            self.assertNotIn(b"\r\n", first)
            self.assertEqual(record["version"], release_evidence.SCHEMA_VERSION)
            self.assertEqual(
                record["artifacts"][0]["sha256"],
                hashlib.sha256(artifact.read_bytes()).hexdigest(),
            )
            self.assertEqual(record["logs"][0]["path"], "logs/run.log")

    def test_rejects_payload_outside_evidence_root(self):
        with TemporaryDirectory() as directory, TemporaryDirectory() as outside:
            root = Path(directory)
            payload = Path(outside) / "payload.log"
            payload.write_text("outside", encoding="utf-8")

            with self.assertRaisesRegex(
                release_evidence.EvidenceError, "escapes evidence root"
            ):
                release_evidence.write_cell_evidence(
                    root / "cell.json",
                    cell_id="cell",
                    candidate_sha=SHA,
                    conclusion="success",
                    logs=[payload],
                    evidence_root=root,
                )

    def test_cell_payload_must_stay_in_the_cell_directory(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            cell_root = root / "cell"
            cell_root.mkdir()
            payload = root / "sibling.log"
            payload.write_text("outside cell", encoding="utf-8")

            with self.assertRaisesRegex(
                release_evidence.EvidenceError, "escapes evidence root"
            ):
                release_evidence.write_cell_evidence(
                    cell_root / "evidence.json",
                    cell_id="cell",
                    candidate_sha=SHA,
                    conclusion="success",
                    logs=[payload],
                    evidence_root=root,
                )


class AggregateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.output = self.root / release_evidence.GATE_RESULT_NAME

    def tearDown(self):
        self.temporary.cleanup()

    def write_cell(
        self,
        cell_id,
        *,
        conclusion="success",
        candidate_sha=SHA,
        target=None,
        crt=None,
        suffix="",
    ):
        cell_root = self.root / f"{cell_id}{suffix}"
        cell_root.mkdir(parents=True)
        log = cell_root / "run.log"
        metadata = cell_root / "metadata.json"
        log.write_text(f"{cell_id}\n", encoding="utf-8")
        metadata.write_text("{}\n", encoding="utf-8")
        evidence = cell_root / "evidence.json"
        release_evidence.write_cell_evidence(
            evidence,
            cell_id=cell_id,
            candidate_sha=candidate_sha,
            conclusion=conclusion,
            artifacts=[metadata],
            logs=[log],
            target=target,
            crt=crt,
            evidence_root=cell_root,
        )
        return evidence

    @staticmethod
    def expected(cell_id, target=None, crt=None, requirements=FIXTURE_REQUIREMENTS):
        return release_evidence.ExpectedCell(
            cell_id,
            target,
            crt,
            requirements=requirements,
        )

    def aggregate(self, evidence, expected):
        normalized = [
            self.expected(value) if isinstance(value, str) else value
            for value in expected
        ]
        return release_evidence.aggregate_release_evidence(
            evidence,
            expected_cells=normalized,
            expected_candidate_sha=SHA,
            evidence_root=self.root,
            output_path=self.output,
        )

    def assert_no_go_with(self, result, text):
        self.assertEqual(result["decision"], "No-Go")
        self.assertTrue(
            any(text in error for check in result["checks"] for error in check["errors"]),
            result,
        )
        self.assertEqual(json.loads(self.output.read_text(encoding="utf-8")), result)

    def test_only_exact_complete_success_inventory_is_go(self):
        alpha = self.write_cell("alpha", target="linux", crt="md")
        beta = self.write_cell("beta")

        result = self.aggregate(
            [beta, alpha],
            [self.expected("alpha", "linux", "md"), "beta"],
        )

        self.assertEqual(result["decision"], "Go")
        self.assertEqual(result["summary"]["successful_checks"], 2)
        self.assertEqual([check["cell_id"] for check in result["checks"]], ["alpha", "beta"])

    def test_rejects_missing_duplicate_and_unexpected_cells_together(self):
        first = self.write_cell("alpha")
        duplicate = self.write_cell("alpha", suffix="-duplicate")
        unexpected = self.write_cell("gamma")

        result = self.aggregate([unexpected, duplicate, first], ["alpha", "beta"])

        self.assert_no_go_with(result, "duplicate cell evidence")
        self.assert_no_go_with(result, "required cell evidence is missing")
        self.assert_no_go_with(result, "not present in the expected inventory")
        self.assertEqual(len(result["checks"]), 3)

    def test_rejects_wrong_sha_and_every_non_success_conclusion(self):
        cases = (("wrong-sha", "success", OTHER_SHA),)
        cases += tuple(
            (f"cell-{conclusion}", conclusion, SHA)
            for conclusion in (
                "failure",
                "failed",
                "skipped",
                "cancelled",
                "timed_out",
            )
        )
        evidence = [
            self.write_cell(cell_id, conclusion=conclusion, candidate_sha=sha)
            for cell_id, conclusion, sha in cases
        ]

        result = self.aggregate(evidence, [cell_id for cell_id, _c, _s in cases])

        self.assertEqual(result["decision"], "No-Go")
        self.assertEqual(result["summary"]["failed_checks"], len(cases))
        errors = [error for check in result["checks"] for error in check["errors"]]
        self.assertTrue(any("candidate SHA mismatch" in error for error in errors))
        for conclusion in ("failure", "failed", "skipped", "cancelled", "timed_out"):
            self.assertTrue(any(repr(conclusion) in error for error in errors), errors)

    def test_rejects_empty_payload_lists_for_a_required_cell(self):
        cell_root = self.root / "empty"
        cell_root.mkdir()
        evidence = cell_root / "evidence.json"
        release_evidence.write_cell_evidence(
            evidence,
            cell_id="empty",
            candidate_sha=SHA,
            conclusion="success",
            evidence_root=cell_root,
        )

        result = self.aggregate([evidence], [self.expected("empty")])

        self.assert_no_go_with(result, "required artifacts evidence is missing")
        self.assert_no_go_with(result, "required logs evidence is missing")

    def test_rejects_missing_required_evidence_item(self):
        cell_root = self.root / "partial"
        cell_root.mkdir()
        log = cell_root / "run.log"
        log.write_text("partial\n", encoding="utf-8")
        evidence = cell_root / "evidence.json"
        release_evidence.write_cell_evidence(
            evidence,
            cell_id="partial",
            candidate_sha=SHA,
            conclusion="success",
            logs=[log],
            evidence_root=cell_root,
        )

        result = self.aggregate([evidence], [self.expected("partial")])

        self.assert_no_go_with(result, "pattern 'metadata.json'")

    def test_rejects_required_evidence_in_the_wrong_collection(self):
        cell_root = self.root / "misclassified"
        cell_root.mkdir()
        metadata = cell_root / "metadata.json"
        log = cell_root / "run.log"
        metadata.write_text("{}\n", encoding="utf-8")
        log.write_text("run\n", encoding="utf-8")
        evidence = cell_root / "evidence.json"
        release_evidence.write_cell_evidence(
            evidence,
            cell_id="misclassified",
            candidate_sha=SHA,
            conclusion="success",
            artifacts=[log],
            logs=[metadata],
            evidence_root=cell_root,
        )

        result = self.aggregate([evidence], [self.expected("misclassified")])

        self.assert_no_go_with(result, "classified as logs")
        self.assert_no_go_with(result, "classified as artifacts")

    def test_required_patterns_are_anchored_to_the_cell_root(self):
        cell_root = self.root / "prefixed"
        artifact = cell_root / "extra" / "metadata" / "result.json"
        log = cell_root / "run.log"
        artifact.parent.mkdir(parents=True)
        artifact.write_text("{}\n", encoding="utf-8")
        log.write_text("run\n", encoding="utf-8")
        evidence = cell_root / "evidence.json"
        release_evidence.write_cell_evidence(
            evidence,
            cell_id="prefixed",
            candidate_sha=SHA,
            conclusion="success",
            artifacts=[artifact],
            logs=[log],
            evidence_root=cell_root,
        )
        expected = self.expected(
            "prefixed",
            requirements=(
                release_evidence.EvidenceRequirement(
                    "artifacts", "metadata/*.json"
                ),
                release_evidence.EvidenceRequirement("logs", "run.log"),
            ),
        )

        result = self.aggregate([evidence], [expected])

        self.assert_no_go_with(result, "pattern 'metadata/*.json'")

    def test_custom_inventory_requires_explicit_artifact_and_log_contracts(self):
        evidence = self.write_cell("alpha")

        result = self.aggregate(
            [evidence],
            [release_evidence.ExpectedCell("alpha")],
        )

        self.assert_no_go_with(result, "non-empty tuple of evidence requirements")

    def test_authoritative_inventory_accepts_complete_required_evidence(self):
        evidence_paths = []
        for expected in release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY:
            cell_root = self.root / expected.cell_id
            artifacts = []
            logs = []
            for requirement in expected.requirements:
                relative = requirement.pattern.replace("*", "sample")
                payload = cell_root.joinpath(*relative.split("/"))
                payload.parent.mkdir(parents=True, exist_ok=True)
                payload.write_text(f"{expected.cell_id}: {relative}\n", encoding="utf-8")
                if requirement.collection == "artifacts":
                    artifacts.append(payload)
                else:
                    logs.append(payload)
            evidence = cell_root / "evidence.json"
            release_evidence.write_cell_evidence(
                evidence,
                cell_id=expected.cell_id,
                candidate_sha=SHA,
                conclusion="success",
                artifacts=artifacts,
                logs=logs,
                target=expected.target,
                crt=expected.crt,
                evidence_root=cell_root,
            )
            evidence_paths.append(evidence)

        result = release_evidence.aggregate_release_evidence(
            evidence_paths,
            expected_cells=release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY,
            expected_candidate_sha=SHA,
            evidence_root=self.root,
            output_path=self.output,
        )

        self.assertEqual(result["decision"], "Go")
        self.assertEqual(result["summary"]["expected_cells"], 13)
        self.assertEqual(result["summary"]["failed_checks"], 0)

    def test_rejects_checksum_mismatch_after_payload_tampering(self):
        evidence = self.write_cell("alpha")
        (evidence.parent / "run.log").write_text("tampered\n", encoding="utf-8")

        result = self.aggregate([evidence], ["alpha"])

        self.assert_no_go_with(result, "checksum mismatch")

    def test_rejects_lexical_path_escape(self):
        evidence = self.write_cell("alpha")
        value = json.loads(evidence.read_text(encoding="utf-8"))
        value["logs"][0]["path"] = "../outside.log"
        evidence.write_text(json.dumps(value), encoding="utf-8")

        result = self.aggregate([evidence], ["alpha"])

        self.assert_no_go_with(result, "unsafe component")

    def test_rejects_symlink_path_escape(self):
        with TemporaryDirectory() as outside_directory:
            evidence = self.write_cell("alpha")
            outside = Path(outside_directory) / "outside.log"
            outside.write_text("outside\n", encoding="utf-8")
            link = evidence.parent / "escaped.log"
            try:
                link.symlink_to(outside)
            except OSError as error:
                self.skipTest(f"file symlinks are unavailable: {error}")
            value = json.loads(evidence.read_text(encoding="utf-8"))
            value["logs"][0] = {
                "path": "escaped.log",
                "sha256": hashlib.sha256(outside.read_bytes()).hexdigest(),
            }
            evidence.write_text(json.dumps(value), encoding="utf-8")

            result = self.aggregate([evidence], ["alpha"])

            self.assert_no_go_with(result, "escapes evidence root")

    def test_malformed_evidence_still_writes_all_expected_checks(self):
        valid = self.write_cell("alpha")
        malformed = self.root / "broken.json"
        malformed.write_text('{"cell_id": "broken",', encoding="utf-8")

        result = self.aggregate([malformed, valid], ["alpha", "beta"])

        self.assertEqual(result["decision"], "No-Go")
        self.assertEqual(
            [check["cell_id"] for check in result["checks"]],
            ["alpha", "beta", None],
        )
        self.assertEqual(len(result["checks"]), 3)

    def test_duplicate_expected_inventory_is_no_go_with_one_cell_check(self):
        evidence = self.write_cell("alpha")

        result = self.aggregate([evidence], ["alpha", "alpha"])

        self.assert_no_go_with(result, "expected cell inventory repeats")
        self.assertEqual(
            [check["cell_id"] for check in result["checks"]],
            ["__inventory__", "alpha"],
        )

    def test_gate_json_is_stable_and_uses_lf(self):
        evidence = self.write_cell("alpha")

        first_result = self.aggregate([evidence], ["alpha"])
        first_bytes = self.output.read_bytes()
        second_result = self.aggregate([evidence], ["alpha"])

        self.assertEqual(second_result, first_result)
        self.assertEqual(self.output.read_bytes(), first_bytes)
        self.assertTrue(first_bytes.endswith(b"\n"))
        self.assertNotIn(b"\r\n", first_bytes)

    def test_default_inventory_covers_required_release_cells(self):
        ids = set(release_evidence.DEFAULT_EXPECTED_CELL_IDS)
        self.assertEqual(len(ids), 13)
        self.assertTrue(
            {
                "linux-test-engine-runtime",
                "linux-multi-viewport-smoke",
                "linux-wasm",
                "windows-vcpkg",
                "windows-platform-md",
                "windows-platform-mt",
                "windows-gnu",
                "macos-build",
            }.issubset(ids)
        )
        self.assertEqual(sum(cell_id.startswith("prebuilt-") for cell_id in ids), 5)
        for cell in release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY:
            self.assertEqual(
                {requirement.collection for requirement in cell.requirements},
                {"artifacts", "logs"},
            )

        requirements = {
            cell.cell_id: {
                (requirement.collection, requirement.pattern)
                for requirement in cell.requirements
            }
            for cell in release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY
        }
        self.assertIn(
            ("artifacts", "runtime/gate-result.json"),
            requirements["linux-test-engine-runtime"],
        )
        self.assertIn(
            ("artifacts", "runtime/viewport-result.json"),
            requirements["linux-multi-viewport-smoke"],
        )
        self.assertIn(
            ("artifacts", "metadata/binding-hashes.json"),
            requirements["linux-wasm"],
        )
        self.assertIn(
            ("artifacts", "metadata/vcpkg.json"),
            requirements["windows-vcpkg"],
        )
        self.assertIn(
            ("artifacts", "metadata/mingw-imports.txt"),
            requirements["windows-gnu"],
        )
        for cell_id in ids:
            if cell_id.startswith("prebuilt-"):
                self.assertIn(
                    ("artifacts", "packages/dear-imgui-*.tar.gz"),
                    requirements[cell_id],
                )
                self.assertIn(
                    ("artifacts", "metadata/prebuilt-manifests.json"),
                    requirements[cell_id],
                )

    def test_production_cli_uses_authoritative_inventory_with_zero_evidence(self):
        with patch.object(
            release_evidence, "resolve_candidate_sha", return_value=SHA
        ) as resolve:
            result = release_evidence.main(
                [
                    "aggregate",
                    "--repo-root",
                    str(self.root),
                    "--candidate-sha",
                    SHA,
                    "--evidence-root",
                    str(self.root),
                    "--output",
                    str(self.output),
                ]
            )

        self.assertEqual(result, 1)
        resolve.assert_called_once_with(self.root, SHA)
        gate = json.loads(self.output.read_text(encoding="utf-8"))
        self.assertEqual(gate["decision"], "No-Go")
        self.assertEqual(gate["summary"]["expected_cells"], 13)
        self.assertEqual(gate["summary"]["failed_checks"], 13)
        self.assertTrue(
            all(
                "required cell evidence is missing" in check["errors"]
                for check in gate["checks"]
            )
        )

    def test_production_cli_rejects_callers_that_try_to_override_inventory(self):
        parser = release_evidence._build_parser()
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
            parser.parse_args(
                [
                    "aggregate",
                    "--repo-root",
                    str(self.root),
                    "--candidate-sha",
                    SHA,
                    "--evidence-root",
                    str(self.root),
                    "--output",
                    str(self.output),
                    "--expected-cell",
                    "only",
                ]
            )

        self.assertEqual(error.exception.code, 2)


class GateResultVerificationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.gate = self.root / release_evidence.GATE_RESULT_NAME

    def tearDown(self):
        self.temporary.cleanup()

    def complete_gate(self):
        count = len(release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY)
        return {
            "version": release_evidence.SCHEMA_VERSION,
            "candidate_sha": SHA,
            "decision": "Go",
            "checks": [
                {
                    "cell_id": cell.cell_id,
                    "conclusion": "success",
                    "evidence_paths": [f"{cell.cell_id}/cell.json"],
                    "errors": [],
                    "status": "success",
                }
                for cell in release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY
            ],
            "summary": {
                "expected_cells": count,
                "successful_checks": count,
                "failed_checks": 0,
            },
        }

    def write_gate(self, value):
        self.gate.write_text(
            json.dumps(value, sort_keys=True) + "\n",
            encoding="utf-8",
            newline="\n",
        )

    def test_accepts_only_complete_same_sha_go(self):
        value = self.complete_gate()
        self.write_gate(value)

        self.assertEqual(
            release_evidence.verify_gate_result(
                self.gate, expected_candidate_sha=SHA
            ),
            value,
        )

    def test_rejects_wrong_sha_no_go_and_inventory_drift(self):
        cases = []
        wrong_sha = self.complete_gate()
        wrong_sha["candidate_sha"] = OTHER_SHA
        cases.append(("candidate SHA mismatch", wrong_sha))
        no_go = self.complete_gate()
        no_go["decision"] = "No-Go"
        cases.append(("decision must be 'Go'", no_go))
        missing = self.complete_gate()
        missing["checks"].pop()
        cases.append(("cell inventory mismatch", missing))
        duplicate = self.complete_gate()
        duplicate["checks"][-1]["cell_id"] = duplicate["checks"][0]["cell_id"]
        cases.append(("repeat a cell_id", duplicate))

        for message, value in cases:
            with self.subTest(message=message):
                self.write_gate(value)
                with self.assertRaisesRegex(release_evidence.EvidenceError, message):
                    release_evidence.verify_gate_result(
                        self.gate, expected_candidate_sha=SHA
                    )

    def test_rejects_unsuccessful_or_unretained_cell_and_summary_drift(self):
        unsuccessful = self.complete_gate()
        unsuccessful["checks"][0]["status"] = "failure"
        unretained = self.complete_gate()
        unretained["checks"][0]["evidence_paths"] = []
        summary = self.complete_gate()
        summary["summary"]["successful_checks"] -= 1

        for message, value in (
            ("not a successful release cell", unsuccessful),
            ("must name retained evidence", unretained),
            ("summary does not describe a complete Go", summary),
        ):
            with self.subTest(message=message):
                self.write_gate(value)
                with self.assertRaisesRegex(release_evidence.EvidenceError, message):
                    release_evidence.verify_gate_result(
                        self.gate, expected_candidate_sha=SHA
                    )

    def test_verify_cli_binds_gate_to_resolved_head(self):
        self.write_gate(self.complete_gate())
        with patch.object(
            release_evidence, "resolve_candidate_sha", return_value=SHA
        ) as resolve:
            result = release_evidence.main(
                [
                    "verify",
                    "--repo-root",
                    str(self.root),
                    "--candidate-sha",
                    SHA,
                    "--gate-result",
                    str(self.gate),
                ]
            )

        self.assertEqual(result, 0)
        resolve.assert_called_once_with(self.root, SHA)


if __name__ == "__main__":
    unittest.main()
