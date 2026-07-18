import hashlib
import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_TOOLS = REPO_ROOT / "tools" / "ci"
sys.path.insert(0, str(CI_TOOLS))

import release_evidence  # noqa: E402


SHA = "0123456789abcdef0123456789abcdef01234567"
OTHER_SHA = "89abcdef0123456789abcdef0123456789abcdef"


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
        log.write_text(f"{cell_id}\n", encoding="utf-8")
        evidence = cell_root / "evidence.json"
        release_evidence.write_cell_evidence(
            evidence,
            cell_id=cell_id,
            candidate_sha=candidate_sha,
            conclusion=conclusion,
            logs=[log],
            target=target,
            crt=crt,
            evidence_root=cell_root,
        )
        return evidence

    def aggregate(self, evidence, expected):
        return release_evidence.aggregate_release_evidence(
            evidence,
            expected_cells=expected,
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
            [release_evidence.ExpectedCell("alpha", "linux", "md"), "beta"],
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


if __name__ == "__main__":
    unittest.main()
