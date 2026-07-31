from pathlib import Path
import shlex
import unittest

from tools.tests.workflow_semantics import (
    job_dependencies,
    load_workflow,
    named_step,
    require_mapping,
    workflow_call_inputs,
    workflow_jobs,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = REPO_ROOT / ".github" / "workflows"


def workflow(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def parsed_workflow(name: str):
    return load_workflow(WORKFLOWS / name)


class ReleaseGateWorkflowTests(unittest.TestCase):
    def test_every_native_runtime_call_satisfies_the_required_input_contract(self):
        runtime = parsed_workflow("native-runtime.yml")
        declared_inputs = workflow_call_inputs(runtime)
        self.assertIs(declared_inputs["candidate_sha"].get("required"), True)
        required_inputs = {
            name
            for name, specification in declared_inputs.items()
            if specification.get("required") is True
        }

        callsites = []
        workflow_paths = sorted(
            path
            for path in WORKFLOWS.iterdir()
            if path.suffix.casefold() in {".yml", ".yaml"}
        )
        for path in workflow_paths:
            for job_id, job in workflow_jobs(load_workflow(path)).items():
                if job.get("uses") != "./.github/workflows/native-runtime.yml":
                    continue
                callsites.append((path.name, job_id))
                provided = require_mapping(
                    job.get("with"), f"{path.name}:{job_id}.with"
                )
                with self.subTest(workflow=path.name, job=job_id):
                    self.assertEqual(required_inputs - provided.keys(), set())

        self.assertGreater(len(callsites), 0)

    def test_ci_native_runtime_calls_bind_the_trigger_commit(self):
        jobs = workflow_jobs(parsed_workflow("ci.yml"))
        callsites = {
            job_id: job
            for job_id, job in jobs.items()
            if job.get("uses") == "./.github/workflows/native-runtime.yml"
        }

        self.assertEqual(len(callsites), 8)
        for job_id, job in callsites.items():
            inputs = require_mapping(job.get("with"), f"ci.yml:{job_id}.with")
            with self.subTest(job=job_id):
                self.assertEqual(inputs.get("candidate_sha"), "${{ github.sha }}")

    def test_candidate_identity_guard_fails_closed_and_protects_release_graph(self):
        jobs = workflow_jobs(parsed_workflow("release-gate.yml"))
        guard_id = "validate-candidate-identity"
        self.assertEqual(next(iter(jobs)), guard_id)
        guard = jobs[guard_id]

        checkout = named_step(guard, "Checkout workflow revision")
        self.assertEqual(checkout.get("uses"), "actions/checkout@v6")
        checkout_inputs = require_mapping(
            checkout.get("with"), f"jobs.{guard_id}.steps.checkout.with"
        )
        self.assertEqual(checkout_inputs.get("ref"), "${{ github.sha }}")

        validation = named_step(guard, "Validate candidate identity")
        self.assertEqual(
            tuple(shlex.split(validation.get("run", ""))),
            (
                "python3",
                "tools/ci/release_evidence.py",
                "verify-candidate",
                "--repo-root",
                "${{ github.workspace }}",
                "--candidate-sha",
                "${{ inputs.candidate_sha }}",
            ),
        )

        protected_jobs = (
            "test-engine-attempt-1",
            "test-engine-attempt-2",
            "test-engine-cell",
            "viewport-attempt-1",
            "viewport-attempt-2",
            "viewport-cell",
            "ash-vulkan-validation-attempt-1",
            "ash-vulkan-validation-attempt-2",
            "ash-vulkan-validation-cell",
            "sdl3-glow-viewport-attempt-1",
            "sdl3-glow-viewport-attempt-2",
            "sdl3-glow-viewport-cell",
            "standard-cells",
            "prebuilt",
            "aggregate",
        )
        for job_id in protected_jobs:
            with self.subTest(job=job_id):
                self.assertIn(guard_id, job_dependencies(jobs[job_id]))

        guard_success = f"needs.{guard_id}.result == 'success'"
        always_jobs = (
            "test-engine-attempt-2",
            "test-engine-cell",
            "viewport-attempt-2",
            "viewport-cell",
            "ash-vulkan-validation-attempt-2",
            "ash-vulkan-validation-cell",
            "sdl3-glow-viewport-attempt-2",
            "sdl3-glow-viewport-cell",
            "aggregate",
        )
        for job_id in always_jobs:
            condition = jobs[job_id].get("if", "")
            terms = {
                term.strip()
                for term in condition.removeprefix("${{").removesuffix("}}").split("&&")
            }
            with self.subTest(always_job=job_id):
                self.assertIn("always()", terms)
                self.assertIn(guard_success, terms)

    def test_release_gate_owns_the_authoritative_fifteen_cell_inventory(self):
        gate = workflow("release-gate.yml")
        prebuilt = workflow("prebuilt-binaries.yml")
        source = f"{gate}\n{prebuilt}"
        required_cells = (
            "linux-test-engine-runtime",
            "linux-multi-viewport-smoke",
            "linux-ash-vulkan-validation-smoke",
            "linux-sdl3-glow-multi-viewport-smoke",
            "linux-wasm",
            "windows-vcpkg",
            "windows-platform-md",
            "windows-platform-mt",
            "windows-gnu",
            "macos-build",
            "prebuilt-x86_64-unknown-linux-gnu",
            "prebuilt-x86_64-apple-darwin",
            "prebuilt-aarch64-apple-darwin",
            "prebuilt-x86_64-pc-windows-msvc-md",
            "prebuilt-x86_64-pc-windows-msvc-mt",
        )

        for cell_id in required_cells:
            with self.subTest(cell_id=cell_id):
                self.assertIn(cell_id, source)
        self.assertIn("uses: ./.github/workflows/prebuilt-binaries.yml", gate)
        self.assertIn("release_cell.py aggregate", gate)
        self.assertIn("if: always()", gate)
        self.assertIn("retention-days: 30", gate)

    def test_linux_wasm_cell_builds_and_verifies_the_actual_provider(self):
        jobs = workflow_jobs(parsed_workflow("release-gate.yml"))
        job = jobs["standard-cells"]
        setup = named_step(job, "Set up pinned Emscripten provider toolchain")
        setup_inputs = require_mapping(
            setup.get("with"), "jobs.standard-cells.emsdk.with"
        )
        self.assertEqual(setup.get("if"), "matrix.cell_id == 'linux-wasm'")
        self.assertEqual(setup.get("uses"), "emscripten-core/setup-emsdk@v16")
        self.assertEqual(str(setup_inputs.get("version")), "5.0.1")
        self.assertEqual(str(setup_inputs.get("emsdk-version")), "5.0.5")

        capture = named_step(job, "Capture WASM route and provider contract")
        self.assertEqual(capture.get("if"), "matrix.cell_id == 'linux-wasm'")
        command = tuple(shlex.split(capture.get("run", "")))
        self.assertIn("tools/ci/verify_wasm_provider.py", command)
        self.assertIn("--check-rust-route", command)

    def test_aggregate_output_is_relative_to_the_evidence_root(self):
        jobs = workflow_jobs(parsed_workflow("release-gate.yml"))
        aggregate = jobs["aggregate"]
        command = shlex.split(
            named_step(aggregate, "Aggregate exact-SHA release evidence").get(
                "run", ""
            )
        )
        self.assertEqual(
            command[command.index("--evidence-root") + 1],
            "target/release-evidence/cells",
        )
        self.assertEqual(command[command.index("--output") + 1], "gate-result.json")

        retain = named_step(aggregate, "Retain authoritative release decision")
        retain_inputs = require_mapping(retain.get("with"), "aggregate.retain.with")
        self.assertEqual(
            retain_inputs.get("path"),
            "target/release-evidence/cells/gate-result.json",
        )

    def test_candidate_sha_is_required_and_checked_out_exactly(self):
        gate = workflow("release-gate.yml")
        native = workflow("native-runtime.yml")
        prebuilt = workflow("prebuilt-binaries.yml")

        self.assertIn("candidate_sha:", gate)
        self.assertIn("required: true", gate)
        self.assertIn("candidate_sha:", native)
        self.assertIn("ref: ${{ inputs.candidate_sha }}", native)
        self.assertIn("workflow_call:", prebuilt)
        self.assertIn("candidate_sha:", prebuilt)
        self.assertIn("ref: ${{ inputs.candidate_sha }}", prebuilt)
        self.assertNotIn("inputs.tag || inputs.branch || github.ref", prebuilt)

    def test_runtime_and_prebuilt_evidence_is_bounded_and_retained(self):
        native = workflow("native-runtime.yml")
        prebuilt = workflow("prebuilt-binaries.yml")

        self.assertIn("timeout-minutes: 30", native)
        self.assertIn("--defer-infrastructure-retry", native)
        self.assertIn("retention-days: 30", native)
        self.assertIn("timeout-minutes: 90", prebuilt)
        self.assertIn("release_cell.py capture", prebuilt)
        self.assertIn("release_cell.py finalize", prebuilt)
        self.assertIn("retention-days: 30", prebuilt)
        self.assertNotIn("softprops/action-gh-release", prebuilt)

    def test_release_requires_the_same_run_gate_before_uploading(self):
        release = workflow("release.yml")

        self.assertNotIn("push:\n    tags:", release)
        self.assertIn("candidate_sha:", release)
        self.assertIn("gate_run_id:", release)
        self.assertIn("ref: ${{ inputs.tag }}", release)
        self.assertIn("actions/download-artifact@v8", release)
        self.assertIn("run-id: ${{ inputs.gate_run_id }}", release)
        self.assertIn("release_evidence.py verify", release)
        self.assertIn("pattern: release-cell-*-${{ inputs.candidate_sha }}", release)
        self.assertIn("files: target/release-cells/prebuilt-*/packages/*.tar.gz", release)
        self.assertIn("prerelease: ${{ contains(steps.tag.outputs.version, '-') }}", release)
        self.assertIn(
            "make_latest: ${{ contains(steps.tag.outputs.version, '-') && 'false' || 'true' }}",
            release,
        )
        self.assertLess(
            release.index("release_evidence.py verify"),
            release.index("softprops/action-gh-release"),
        )


if __name__ == "__main__":
    unittest.main()
