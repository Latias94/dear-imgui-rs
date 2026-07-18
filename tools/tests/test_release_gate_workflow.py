from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = REPO_ROOT / ".github" / "workflows"


def workflow(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


class ReleaseGateWorkflowTests(unittest.TestCase):
    def test_release_gate_owns_the_authoritative_thirteen_cell_inventory(self):
        gate = workflow("release-gate.yml")
        prebuilt = workflow("prebuilt-binaries.yml")
        source = f"{gate}\n{prebuilt}"
        required_cells = (
            "linux-test-engine-runtime",
            "linux-multi-viewport-smoke",
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
        self.assertLess(
            release.index("release_evidence.py verify"),
            release.index("softprops/action-gh-release"),
        )


if __name__ == "__main__":
    unittest.main()
