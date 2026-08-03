import re
import shlex
import unittest
from pathlib import Path

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
CHECKOUT_ACTION = "actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803"
EMSDK_ACTION = (
    "emscripten-core/setup-emsdk@4528d102f7230f0e7b276855c01ea1159be0e984"
)
UPLOAD_ARTIFACT_ACTION = (
    "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
)


def workflow(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def parsed_workflow(name: str):
    return load_workflow(WORKFLOWS / name)


class ReleaseGateWorkflowTests(unittest.TestCase):
    def test_ci_and_reusable_runtime_default_to_read_only_contents(self):
        for name in ("ci.yml", "native-runtime.yml", "release-gate.yml"):
            document = parsed_workflow(name)
            with self.subTest(workflow=name):
                self.assertEqual(
                    require_mapping(document.get("permissions"), f"{name}.permissions"),
                    {"contents": "read"},
                )

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
        self.assertEqual(checkout.get("uses"), CHECKOUT_ACTION)
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
                "$GITHUB_WORKSPACE",
                "--candidate-sha",
                "$RELEASE_CANDIDATE_SHA",
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
            "source-packages",
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

    def test_release_gate_owns_the_authoritative_sixteen_cell_inventory(self):
        gate = workflow("release-gate.yml")
        prebuilt = workflow("prebuilt-binaries.yml")
        source = f"{gate}\n{prebuilt}"
        required_cells = (
            "linux-test-engine-runtime",
            "linux-multi-viewport-smoke",
            "linux-ash-vulkan-validation-smoke",
            "linux-sdl3-glow-multi-viewport-smoke",
            "linux-wasm",
            "linux-source-packages",
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
        self.assertEqual(setup.get("uses"), EMSDK_ACTION)
        self.assertEqual(str(setup_inputs.get("version")), "5.0.1")
        self.assertEqual(str(setup_inputs.get("emsdk-version")), "5.0.5")
        self.assertNotIn("actions-cache-folder", setup_inputs)

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
        prebuilt_job = workflow_jobs(parsed_workflow("prebuilt-binaries.yml"))[
            "build-prebuilt"
        ]

        self.assertIn("timeout-minutes: 30", native)
        self.assertIn("--defer-infrastructure-retry", native)
        self.assertIn("retention-days: 30", native)
        self.assertIn("timeout-minutes: 160", prebuilt)
        command_timeouts = []
        for step_name in (
            "Build complete core and extension profile matrix",
            "Consume every safe prebuilt profile",
        ):
            command = str(named_step(prebuilt_job, step_name).get("run", ""))
            match = re.search(r"--timeout\s+(\d+)", command)
            self.assertIsNotNone(match, command)
            command_timeouts.append(int(match.group(1)))
        self.assertGreaterEqual(
            int(prebuilt_job["timeout-minutes"]) * 60,
            sum(command_timeouts) + 1200,
        )
        self.assertIn("release_cell.py capture", prebuilt)
        self.assertIn("release_cell.py finalize", prebuilt)
        self.assertIn("retention-days: 30", prebuilt)
        self.assertNotIn("softprops/action-gh-release", prebuilt)

    def test_runtime_preparation_failure_drives_the_retry_output(self):
        runtime_job = workflow_jobs(parsed_workflow("native-runtime.yml"))["runtime"]
        preparation = named_step(runtime_job, "Classify runtime preparation failure")

        self.assertEqual(preparation.get("id"), "preparation")
        self.assertEqual(
            preparation.get("if"),
            "always() && steps.gate.outcome == 'skipped' && "
            "hashFiles('tools/ci/run_contract.py') != ''",
        )
        self.assertIn("runtime-preparation-failure", preparation.get("run", ""))
        outputs = require_mapping(runtime_job.get("outputs"), "runtime.outputs")
        self.assertIn(
            "steps.preparation.outputs.retry_eligible",
            outputs["retry_eligible"],
        )
        self.assertIn(
            "inputs.gate_attempt == 1",
            outputs["retry_eligible"],
        )
        self.assertIn(
            "steps.preparation.outputs.gate_success",
            outputs["gate_success"],
        )

    def test_untrusted_workflow_inputs_never_enter_shell_source(self):
        unsafe_expression = re.compile(
            r"\$\{\{\s*(?:inputs\.|github\.event\.)"
        )
        for name in (
            "release.yml",
            "release-gate.yml",
            "native-runtime.yml",
            "prebuilt-binaries.yml",
        ):
            jobs = workflow_jobs(parsed_workflow(name))
            for job_id, job in jobs.items():
                steps = job.get("steps", [])
                if not isinstance(steps, list):
                    continue
                for index, step in enumerate(steps):
                    run = step.get("run") if isinstance(step, dict) else None
                    if not isinstance(run, str):
                        continue
                    with self.subTest(workflow=name, job=job_id, step=index):
                        self.assertIsNone(unsafe_expression.search(run), run)

    def test_release_is_one_same_run_resumable_transaction(self):
        document = parsed_workflow("release.yml")
        source = workflow("release.yml")
        triggers = require_mapping(document.get("on"), "release.on")
        dispatch = require_mapping(
            triggers.get("workflow_dispatch"), "release.on.workflow_dispatch"
        )
        inputs = require_mapping(dispatch.get("inputs"), "release dispatch inputs")
        self.assertEqual(set(inputs), {"tag"})
        self.assertNotIn("push:\n    tags:", source)
        self.assertEqual(document.get("permissions"), {})

        jobs = workflow_jobs(document)
        self.assertEqual(
            tuple(jobs),
            ("validate", "gate", "prepare", "publish-crates", "github-release"),
        )
        self.assertEqual(job_dependencies(jobs["gate"]), ("validate",))
        self.assertEqual(jobs["gate"].get("uses"), "./.github/workflows/release-gate.yml")
        self.assertEqual(
            require_mapping(jobs["gate"].get("with"), "release.gate.with").get(
                "candidate_sha"
            ),
            "${{ github.sha }}",
        )
        self.assertEqual(job_dependencies(jobs["prepare"]), ("gate",))
        self.assertEqual(job_dependencies(jobs["publish-crates"]), ("prepare",))
        self.assertEqual(
            job_dependencies(jobs["github-release"]), ("publish-crates",)
        )

        publish = jobs["publish-crates"]
        self.assertEqual(publish.get("environment"), "release")
        self.assertEqual(
            require_mapping(publish.get("permissions"), "publish permissions"),
            {"actions": "read", "contents": "read", "id-token": "write"},
        )
        auth = named_step(publish, "Acquire short-lived crates.io token")
        self.assertEqual(
            auth.get("uses"),
            "rust-lang/crates-io-auth-action@c6f97d42243bad5fab37ca0427f495c86d5b1a18",
        )
        upload = named_step(publish, "Publish complete crates.io release train")
        self.assertIn("--release-gate-result", upload.get("run", ""))
        self.assertIn("--yes", upload.get("run", ""))
        self.assertIn("--no-verify", upload.get("run", ""))

        prepare = named_step(jobs["prepare"], "Prepare exact release bundle")
        self.assertIn("release_cell.py prepare-release", prepare.get("run", ""))
        self.assertNotIn("run-id:", source)
        github_release = named_step(jobs["github-release"], "Create GitHub release")
        self.assertEqual(
            github_release.get("uses"),
            "softprops/action-gh-release@3d0d9888cb7fd7b750713d6e236d1fcb99157228",
        )
        release_inputs = require_mapping(
            github_release.get("with"), "github release inputs"
        )
        self.assertEqual(
            release_inputs.get("files"),
            "${{ runner.temp }}/release-ready/assets/*",
        )
        self.assertTrue(release_inputs.get("overwrite_files"))
        self.assertLess(
            source.index("--verify-published"),
            source.index("action-gh-release"),
        )
        compatible_assets = named_step(
            jobs["github-release"],
            "Reject unexpected assets on an existing release",
        )
        exact_assets = named_step(
            jobs["github-release"],
            "Verify exact GitHub release asset inventory",
        )
        self.assertIn("verify_github_release.py", compatible_assets.get("run", ""))
        self.assertNotIn("--require-exact", compatible_assets.get("run", ""))
        self.assertIn("--require-exact", exact_assets.get("run", ""))
        self.assertLess(
            source.index("Reject unexpected assets on an existing release"),
            source.index("action-gh-release"),
        )
        self.assertLess(
            source.index("action-gh-release"),
            source.index("Verify exact GitHub release asset inventory"),
        )

    def test_release_gate_artifacts_survive_failed_job_reruns(self):
        gate = workflow("release-gate.yml")
        runtime = workflow("native-runtime.yml")
        prebuilt = workflow("prebuilt-binaries.yml")
        source = f"{gate}\n{runtime}\n{prebuilt}"

        self.assertNotIn("github.run_attempt", source)
        upload_count = source.count(f"uses: {UPLOAD_ARTIFACT_ACTION}")
        self.assertGreater(upload_count, 0)
        self.assertEqual(source.count("overwrite: true"), upload_count)

    def test_release_supply_chain_actions_are_pinned_to_commits(self):
        release_workflows = (
            "release.yml",
            "release-gate.yml",
            "native-runtime.yml",
            "prebuilt-binaries.yml",
        )
        use_pattern = re.compile(r"^\s*uses:\s+([^\s#]+)", re.MULTILINE)
        pinned_action = re.compile(r"^[^@]+@[0-9a-f]{40}$")

        for name in release_workflows:
            for action in use_pattern.findall(workflow(name)):
                if action.startswith("./"):
                    continue
                with self.subTest(workflow=name, action=action):
                    self.assertRegex(action, pinned_action)


if __name__ == "__main__":
    unittest.main()
