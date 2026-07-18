import io
import json
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
sys.path.insert(0, str(CI_DIR))

import release_cell  # noqa: E402
import release_evidence  # noqa: E402


SHA = "a" * 40


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    )


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def capture(self, code: str, *, timeout: float = 5.0) -> int:
        return release_cell.capture_command(
            cell_root=self.root,
            execution_path=Path("executions/run.json"),
            stdout_log=Path("logs/stdout.log"),
            stderr_log=Path("logs/stderr.log"),
            command=(sys.executable, "-c", code),
            timeout=timeout,
            termination_grace=0.05,
        )

    def execution(self) -> dict[str, object]:
        return json.loads(
            (self.root / "executions/run.json").read_text(encoding="utf-8")
        )

    def test_success_streams_and_retains_utf8_lf_logs(self):
        result = self.capture(
            "import sys; print('alpha'); print('beta', file=sys.stderr)"
        )

        self.assertEqual(result, 0)
        self.assertEqual((self.root / "logs/stdout.log").read_bytes(), b"alpha\n")
        self.assertEqual((self.root / "logs/stderr.log").read_bytes(), b"beta\n")
        execution = self.execution()
        self.assertEqual(execution["returncode"], 0)
        self.assertFalse(execution["timed_out"])
        self.assertFalse(execution["start_failure"])
        self.assertEqual(execution["evidence_errors"], [])
        raw = (self.root / "executions/run.json").read_bytes()
        self.assertTrue(raw.endswith(b"\n"))
        self.assertNotIn(b"\r\n", raw)

    def test_nonzero_exit_is_preserved_and_propagated(self):
        result = self.capture("raise SystemExit(7)")

        self.assertEqual(result, 7)
        self.assertEqual(self.execution()["returncode"], 7)
        self.assertFalse(self.execution()["timed_out"])

    def test_timeout_is_recorded_and_propagated(self):
        result = self.capture("import time; time.sleep(10)", timeout=0.05)

        self.assertEqual(result, release_cell.TIMEOUT_EXIT_CODE)
        self.assertTrue(self.execution()["timed_out"])
        self.assertFalse(self.execution()["start_failure"])

    def test_start_failure_retains_logs_and_execution_json(self):
        result = release_cell.capture_command(
            cell_root=self.root,
            execution_path=Path("executions/run.json"),
            stdout_log=Path("logs/stdout.log"),
            stderr_log=Path("logs/stderr.log"),
            command=(str(self.root / "definitely-missing-executable"),),
            timeout=1.0,
        )

        self.assertEqual(result, release_cell.START_FAILURE_EXIT_CODE)
        self.assertTrue(self.execution()["start_failure"])
        self.assertIsNone(self.execution()["returncode"])
        self.assertTrue((self.root / "logs/stdout.log").is_file())
        self.assertIn(b"could not run", (self.root / "logs/stderr.log").read_bytes())


class MetadataTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "repo"
        self.cell = Path(self.temporary.name) / "cell"
        (self.root / "dear-imgui-sys/src").mkdir(parents=True)
        (self.root / "extensions/example-sys/src").mkdir(parents=True)
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["dear-imgui-sys", "extensions/example-sys"]\n',
            encoding="utf-8",
        )
        for relative in (
            "dear-imgui-sys/Cargo.toml",
            "extensions/example-sys/Cargo.toml",
        ):
            path = self.root / relative
            path.write_text(
                f'[package]\nname = "{path.parent.name}"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
        (self.root / "dear-imgui-sys/src/wasm_bindings_pregenerated.rs").write_text(
            "pub const CORE: u32 = 1;\n", encoding="utf-8"
        )
        (
            self.root
            / "extensions/example-sys/src/wasm_bindings_pregenerated.rs"
        ).write_text("pub const EXT: u32 = 2;\n", encoding="utf-8")

    def tearDown(self):
        self.temporary.cleanup()

    def test_wasm_metadata_is_deterministic_and_hashes_authoritative_inputs(self):
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ) as resolve:
            first = release_cell.materialize_metadata(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
            )
            first_bytes = {path.name: path.read_bytes() for path in first}
            second = release_cell.materialize_metadata(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
            )

        self.assertEqual(
            {path.name for path in second},
            {"target.json", "binding-hashes.json", "manifests.json"},
        )
        self.assertEqual(
            {path.name: path.read_bytes() for path in second}, first_bytes
        )
        self.assertEqual(resolve.call_count, 2)
        resolve.assert_called_with(self.root.resolve(), SHA)
        hashes = json.loads(
            (self.cell / "metadata/binding-hashes.json").read_text(encoding="utf-8")
        )
        self.assertEqual(
            [entry["path"] for entry in hashes["files"]],
            [
                "dear-imgui-sys/src/wasm_bindings_pregenerated.rs",
                "extensions/example-sys/src/wasm_bindings_pregenerated.rs",
            ],
        )

    def test_candidate_mismatch_stops_before_writing_metadata(self):
        with (
            patch.object(
                release_cell.release_evidence,
                "resolve_candidate_sha",
                side_effect=release_evidence.EvidenceError("candidate HEAD mismatch"),
            ),
            self.assertRaisesRegex(
                release_evidence.EvidenceError, "candidate HEAD mismatch"
            ),
        ):
            release_cell.materialize_metadata(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
            )

        self.assertFalse(self.cell.exists())

    def test_mingw_import_evidence_is_preserved_with_lf_and_validated(self):
        source = Path(self.temporary.name) / "imports.txt"
        source.write_bytes(b"Checking imports\r\nDLL Name: KERNEL32.dll\r\n")
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ):
            release_cell.materialize_metadata(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="windows-gnu",
                cell_root=self.cell,
                mingw_imports=source,
            )

        self.assertEqual(
            (self.cell / "metadata/mingw-imports.txt").read_bytes(),
            b"Checking imports\nDLL Name: KERNEL32.dll\n",
        )

        source.write_text("DLL Name: libstdc++-6.dll\n", encoding="utf-8")
        with (
            patch.object(
                release_cell.release_evidence,
                "resolve_candidate_sha",
                return_value=SHA,
            ),
            self.assertRaisesRegex(release_cell.ReleaseCellError, "libstdc"),
        ):
            release_cell.materialize_metadata(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="windows-gnu",
                cell_root=Path(self.temporary.name) / "rejected-cell",
                mingw_imports=source,
            )

    def test_prebuilt_metadata_reads_tar_manifests_and_copies_packages(self):
        package_dir = Path(self.temporary.name) / "source-packages"
        package_dir.mkdir()
        core = package_dir / "dear-imgui-core.tar.gz"
        extension = package_dir / "dear-implot-prebuilt-test.tar.gz"
        self._write_archive(
            core,
            "dear-imgui-sys prebuilt\n"
            f"candidate_sha={SHA}\n"
            "binding_spec_hash=fnv1a64:1111111111111111\n"
            f"cimgui_revision={'b' * 40}\n"
            f"imgui_revision={'c' * 40}\n",
        )
        self._write_archive(
            extension,
            "dear-implot-sys prebuilt\n"
            f"candidate_sha={SHA}\n"
            "extension_binding_identity=fnv1a64:2222222222222222\n"
            "core_artifact_identity=fnv1a64:3333333333333333\n",
        )
        with (
            patch.object(
                release_cell.release_evidence,
                "resolve_candidate_sha",
                return_value=SHA,
            ),
            patch.object(
                release_cell._prebuilt,
                "select_core_prebuilt_archives",
                return_value={"normal": core},
            ),
            patch.object(
                release_cell._prebuilt,
                "select_extension_prebuilt_archives",
                return_value={("implot", "normal"): extension},
            ),
        ):
            release_cell.materialize_metadata(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="prebuilt-x86_64-unknown-linux-gnu",
                cell_root=self.cell,
                package_dir=package_dir,
            )

        self.assertTrue((self.cell / "packages" / core.name).is_file())
        self.assertTrue((self.cell / "packages" / extension.name).is_file())
        manifests = json.loads(
            (self.cell / "metadata/prebuilt-manifests.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            [item["archive"] for item in manifests["archives"]],
            [core.name, extension.name],
        )
        for name in (
            "build.stdout.log",
            "build.stderr.log",
            "consume.stdout.log",
            "consume.stderr.log",
        ):
            path = self.cell / "logs" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"{name}\n", encoding="utf-8")
        executions = []
        for name in ("build", "consume"):
            path = self.cell / "executions" / f"{name}.json"
            write_json(
                path,
                {
                    "schema_version": 1,
                    "command": ["cargo", name],
                    "returncode": 0,
                    "timed_out": False,
                    "start_failure": False,
                    "evidence_errors": [],
                },
            )
            executions.append(path)
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ):
            record = release_cell.finalize_cell(
                repo_root=self.root,
                candidate_sha=SHA,
                cell_id="prebuilt-x86_64-unknown-linux-gnu",
                cell_root=self.cell,
                execution_paths=executions,
            )

        self.assertEqual(record["conclusion"], "success")
        self.assertEqual(len(record["logs"]), 4)

    @staticmethod
    def _write_archive(path: Path, manifest: str) -> None:
        payload = manifest.encode("utf-8")
        info = tarfile.TarInfo("manifest.txt")
        info.size = len(payload)
        with tarfile.open(path, "w:gz") as archive:
            archive.addfile(info, io.BytesIO(payload))


class FinalizeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name) / "repo"
        self.repo.mkdir()
        self.cell = Path(self.temporary.name) / "cell"
        for relative in (
            "metadata/target.json",
            "metadata/binding-hashes.json",
            "metadata/manifests.json",
        ):
            write_json(
                self.cell / relative,
                {
                    "schema_version": 1,
                    "cell_id": "linux-wasm",
                    "candidate_sha": SHA,
                    "target": "wasm32-unknown-unknown",
                },
            )
        (self.cell / "logs").mkdir()
        (self.cell / "logs/stdout.log").write_text("ok\n", encoding="utf-8")
        (self.cell / "logs/stderr.log").write_text("", encoding="utf-8")
        self.execution = self.cell / "executions/check.json"
        write_json(
            self.execution,
            {
                "schema_version": 1,
                "command": ["cargo", "check"],
                "returncode": 0,
                "timed_out": False,
                "start_failure": False,
                "evidence_errors": [],
            },
        )

    def tearDown(self):
        self.temporary.cleanup()

    def test_finalize_derives_success_and_classifies_evidence(self):
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ) as resolve:
            record = release_cell.finalize_cell(
                repo_root=self.repo,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
                execution_paths=(self.execution,),
            )

        resolve.assert_called_once_with(self.repo.resolve(), SHA)
        self.assertEqual(record["conclusion"], "success")
        artifact_paths = {item["path"] for item in record["artifacts"]}
        log_paths = {item["path"] for item in record["logs"]}
        self.assertIn("executions/check.json", artifact_paths)
        self.assertIn("metadata/target.json", artifact_paths)
        self.assertEqual(log_paths, {"logs/stdout.log", "logs/stderr.log"})

    def test_finalize_never_accepts_a_caller_conclusion(self):
        value = json.loads(self.execution.read_text(encoding="utf-8"))
        value["returncode"] = 9
        write_json(self.execution, value)
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ):
            record = release_cell.finalize_cell(
                repo_root=self.repo,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
                execution_paths=(self.execution,),
            )

        self.assertEqual(record["conclusion"], "failure")

    def test_finalize_requires_every_discovered_execution_record(self):
        unlisted = self.cell / "executions/unlisted-failure.json"
        write_json(
            unlisted,
            {
                "schema_version": 1,
                "command": ["cargo", "test"],
                "returncode": 1,
                "timed_out": False,
                "start_failure": False,
                "evidence_errors": [],
            },
        )
        with (
            patch.object(
                release_cell.release_evidence,
                "resolve_candidate_sha",
                return_value=SHA,
            ),
            self.assertRaisesRegex(release_cell.ReleaseCellError, "enumerate every"),
        ):
            release_cell.finalize_cell(
                repo_root=self.repo,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
                execution_paths=(self.execution,),
            )

    def test_finalize_rejects_metadata_target_drift(self):
        target_path = self.cell / "metadata/target.json"
        value = json.loads(target_path.read_text(encoding="utf-8"))
        value["target"] = "wrong-target"
        write_json(target_path, value)
        with (
            patch.object(
                release_cell.release_evidence,
                "resolve_candidate_sha",
                return_value=SHA,
            ),
            self.assertRaisesRegex(release_cell.ReleaseCellError, "target mismatch"),
        ):
            release_cell.finalize_cell(
                repo_root=self.repo,
                candidate_sha=SHA,
                cell_id="linux-wasm",
                cell_root=self.cell,
                execution_paths=(self.execution,),
            )


class RuntimeFinalizeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name) / "repo"
        self.repo.mkdir()
        self.cell = Path(self.temporary.name) / "cell"

    def tearDown(self):
        self.temporary.cleanup()

    def write_attempt(
        self,
        name: str,
        *,
        gate: str = "test-engine-runtime",
        attempt: int,
        success: bool,
        category: str,
        retry: bool,
        extra_files: tuple[str, ...] = (
            "pass.stdout.log",
            "pass.stderr.log",
        ),
    ) -> Path:
        root = Path(self.temporary.name) / name
        write_json(
            root / "gate-invocation.json",
            {
                "schema_version": 1,
                "status": "Complete",
                "gate": gate,
                "attempt": attempt,
                "process_id": 123,
            },
        )
        for relative in extra_files:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if path.suffix == ".json":
                write_json(path, {"result": "ok"})
            else:
                path.write_text(f"{relative}\n", encoding="utf-8")
        evidence = sorted(
            path.relative_to(root).as_posix()
            for path in root.rglob("*")
            if path.is_file()
        )
        write_json(
            root / "gate-result.json",
            {
                "schema_version": 1,
                "status": "Complete",
                "gate": gate,
                "success": success,
                "category": category,
                "attempt": attempt,
                "summary": "fixture result",
                "retry": {
                    "eligible": retry,
                    "max_fresh_runner_attempts": 2,
                },
                "evidence": evidence,
                "details": {},
            },
        )
        return root

    def finalize(self, attempt1: Path, attempt2: Path | None = None) -> dict[str, object]:
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ):
            return release_cell.finalize_runtime_cell(
                repo_root=self.repo,
                candidate_sha=SHA,
                cell_id="linux-test-engine-runtime",
                cell_root=self.cell,
                attempt1_dir=attempt1,
                attempt2_dir=attempt2,
            )

    def test_retry_selects_attempt_two_and_retains_both_attempts(self):
        first = self.write_attempt(
            "attempt-one",
            attempt=1,
            success=False,
            category="InfrastructureUnavailable",
            retry=True,
        )
        second = self.write_attempt(
            "attempt-two",
            attempt=2,
            success=True,
            category="Passed",
            retry=False,
        )

        record = self.finalize(first, second)

        self.assertEqual(record["conclusion"], "success")
        stable = json.loads(
            (self.cell / "runtime/gate-result.json").read_text(encoding="utf-8")
        )
        self.assertEqual(stable["attempt"], 2)
        self.assertTrue((self.cell / "runtime/attempt1/gate-result.json").is_file())
        self.assertTrue((self.cell / "runtime/attempt2/gate-result.json").is_file())
        self.assertEqual(list(self.cell.rglob("cell.json")), [self.cell / "cell.json"])

    def test_no_retry_uses_the_first_terminal_success(self):
        first = self.write_attempt(
            "attempt-one",
            attempt=1,
            success=True,
            category="Passed",
            retry=False,
        )

        record = self.finalize(first)

        self.assertEqual(record["conclusion"], "success")
        self.assertFalse((self.cell / "runtime/attempt2").exists())

    def test_product_failure_creates_a_failure_record_without_yaml_outcome(self):
        first = self.write_attempt(
            "attempt-one",
            attempt=1,
            success=False,
            category="ProductFailure",
            retry=False,
        )

        record = self.finalize(first)

        self.assertEqual(record["conclusion"], "failure")

    def test_retry_is_rejected_after_a_noneligible_product_failure(self):
        first = self.write_attempt(
            "attempt-one",
            attempt=1,
            success=False,
            category="ProductFailure",
            retry=False,
        )
        second = self.write_attempt(
            "attempt-two",
            attempt=2,
            success=True,
            category="Passed",
            retry=False,
        )

        with self.assertRaisesRegex(release_cell.ReleaseCellError, "not retry-eligible"):
            self.finalize(first, second)

    def test_retry_eligible_attempt_requires_a_second_terminal_attempt(self):
        first = self.write_attempt(
            "attempt-one",
            attempt=1,
            success=False,
            category="InfrastructureUnavailable",
            retry=True,
        )

        with self.assertRaisesRegex(release_cell.ReleaseCellError, "requires attempt 2"):
            self.finalize(first)

    def test_runtime_attempt_must_not_embed_another_cell_record(self):
        first = self.write_attempt(
            "attempt-one",
            attempt=1,
            success=True,
            category="Passed",
            retry=False,
            extra_files=("pass.stdout.log", "pass.stderr.log", "cell.json"),
        )

        with self.assertRaisesRegex(release_cell.ReleaseCellError, "contain cell.json"):
            self.finalize(first)

    def test_wrong_gate_and_attempt_are_rejected(self):
        wrong_gate = self.write_attempt(
            "wrong-gate",
            gate="multi-viewport-smoke",
            attempt=1,
            success=True,
            category="Passed",
            retry=False,
        )
        wrong_attempt = self.write_attempt(
            "wrong-attempt",
            attempt=2,
            success=True,
            category="Passed",
            retry=False,
        )
        for source, message in (
            (wrong_gate, "gate mismatch"),
            (wrong_attempt, "attempt mismatch"),
        ):
            with self.subTest(message=message), self.assertRaisesRegex(
                release_cell.ReleaseCellError, message
            ):
                self.finalize(source)

    def test_viewport_cell_uses_its_exact_stable_evidence_contract(self):
        files = (
            "runtime-environment.json",
            "viewport-result.json",
            "display.stdout.log",
            "display.stderr.log",
            "adapter.stdout.log",
            "adapter.stderr.log",
            "viewport.stdout.log",
            "viewport.stderr.log",
        )
        first = self.write_attempt(
            "viewport-attempt",
            gate="multi-viewport-smoke",
            attempt=1,
            success=True,
            category="Passed",
            retry=False,
            extra_files=files,
        )
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ):
            record = release_cell.finalize_runtime_cell(
                repo_root=self.repo,
                candidate_sha=SHA,
                cell_id="linux-multi-viewport-smoke",
                cell_root=self.cell,
                attempt1_dir=first,
            )

        self.assertEqual(record["cell_id"], "linux-multi-viewport-smoke")
        self.assertEqual(record["conclusion"], "success")
        self.assertTrue((self.cell / "runtime/viewport-result.json").is_file())


class AggregateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name) / "repo"
        self.repo.mkdir()
        self.evidence = Path(self.temporary.name) / "evidence"
        self.evidence.mkdir()
        self.output = self.evidence / "gate-result.json"

    def tearDown(self):
        self.temporary.cleanup()

    def test_zero_discovery_still_writes_a_complete_no_go(self):
        with patch.object(
            release_cell.release_evidence,
            "resolve_candidate_sha",
            return_value=SHA,
        ):
            result = release_cell.aggregate_cells(
                repo_root=self.repo,
                candidate_sha=SHA,
                evidence_root=self.evidence,
                output_path=self.output,
            )

        self.assertEqual(result["decision"], "No-Go")
        self.assertEqual(
            result["summary"]["expected_cells"],
            len(release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY),
        )
        self.assertEqual(len(result["checks"]), 13)
        self.assertTrue(self.output.is_file())

    def test_discovery_is_stable_and_excludes_the_output_itself(self):
        first = self.evidence / "b/cell.json"
        second = self.evidence / "a/cell.json"
        output = self.evidence / "cell.json"
        for path in (first, second, output):
            write_json(path, {})
        with (
            patch.object(
                release_cell.release_evidence,
                "resolve_candidate_sha",
                return_value=SHA,
            ),
            patch.object(
                release_cell.release_evidence,
                "aggregate_release_evidence",
                return_value={"decision": "Go"},
            ) as aggregate,
        ):
            result = release_cell.aggregate_cells(
                repo_root=self.repo,
                candidate_sha=SHA,
                evidence_root=self.evidence,
                output_path=output,
            )

        self.assertEqual(result["decision"], "Go")
        self.assertEqual(aggregate.call_args.args[0], [second, first])
        self.assertEqual(
            aggregate.call_args.kwargs["expected_cells"],
            release_evidence.DEFAULT_EXPECTED_CELL_INVENTORY,
        )


if __name__ == "__main__":
    unittest.main()
