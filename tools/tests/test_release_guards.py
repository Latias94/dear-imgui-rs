import hashlib
import importlib.util
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOL_PATH = REPO_ROOT / "tools" / "ci" / "release_workflow.py"
SPEC = importlib.util.spec_from_file_location("release_workflow", TOOL_PATH)
RELEASE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = RELEASE
SPEC.loader.exec_module(RELEASE)


class ReleaseGuardTests(unittest.TestCase):
    CANDIDATE_SHA = "a" * 40

    def test_latest_exact_sha_failure_cannot_be_hidden_by_older_success(self):
        runs = [
            {
                "id": 1,
                "created_at": "2026-08-05T01:00:00Z",
                "head_sha": self.CANDIDATE_SHA,
                "head_branch": "main",
                "event": "push",
                "status": "completed",
                "conclusion": "success",
            },
            {
                "id": 2,
                "created_at": "2026-08-05T02:00:00Z",
                "head_sha": self.CANDIDATE_SHA,
                "head_branch": "main",
                "event": "push",
                "status": "completed",
                "conclusion": "failure",
            },
        ]

        selected = RELEASE.select_latest_exact_sha_ci_run(
            runs, self.CANDIDATE_SHA
        )

        self.assertEqual(selected["id"], 2)
        self.assertEqual(selected["conclusion"], "failure")

    def test_remote_tag_resolution_prefers_annotated_tag_commit(self):
        tag = "v0.16.0-alpha.2"
        tag_object = "b" * 40
        commit = "c" * 40
        output = "\n".join(
            (
                f"{tag_object}\trefs/tags/{tag}",
                f"{commit}\trefs/tags/{tag}^{{}}",
            )
        )

        self.assertEqual(RELEASE.resolve_remote_tag(output, tag), commit)
        self.assertEqual(
            RELEASE.resolve_remote_tag(f"{commit}\trefs/tags/{tag}\n", tag),
            commit,
        )

    def test_existing_identical_asset_subset_is_resumable(self):
        expected = {
            "a.tar.gz": hashlib.sha256(b"archive-a").hexdigest(),
            "b.tar.gz": hashlib.sha256(b"archive-b").hexdigest(),
        }
        release = {
            "draft": True,
            "assets": [
                {
                    "name": "a.tar.gz",
                    "state": "uploaded",
                    "digest": f"sha256:{expected['a.tar.gz']}",
                }
            ],
        }

        inspection = RELEASE.inspect_release_assets(
            release, expected, require_complete=False
        )

        self.assertEqual(inspection.release_state, "draft")
        self.assertEqual(inspection.missing, ("b.tar.gz",))

    def test_existing_asset_conflicts_fail_closed(self):
        expected = {"a.tar.gz": hashlib.sha256(b"expected").hexdigest()}
        mismatch = {
            "draft": False,
            "assets": [
                {
                    "name": "a.tar.gz",
                    "state": "uploaded",
                    "digest": f"sha256:{hashlib.sha256(b'different').hexdigest()}",
                }
            ],
        }
        unexpected = {
            "draft": False,
            "assets": [
                {
                    "name": "stale.tar.gz",
                    "state": "uploaded",
                    "digest": f"sha256:{hashlib.sha256(b'stale').hexdigest()}",
                }
            ],
        }

        with self.assertRaisesRegex(RELEASE.ReleaseWorkflowError, "digest mismatch"):
            RELEASE.inspect_release_assets(
                mismatch, expected, require_complete=False
            )
        with self.assertRaisesRegex(RELEASE.ReleaseWorkflowError, "unexpected"):
            RELEASE.inspect_release_assets(
                unexpected, expected, require_complete=False
            )

    def test_complete_verification_rejects_missing_assets(self):
        expected = {"a.tar.gz": hashlib.sha256(b"archive").hexdigest()}
        with self.assertRaisesRegex(RELEASE.ReleaseWorkflowError, "missing expected"):
            RELEASE.inspect_release_assets(
                {"draft": False, "assets": []},
                expected,
                require_complete=True,
            )

    def test_checksum_output_is_sorted_and_stable(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.joinpath("z.tar.gz").write_bytes(b"z")
            root.joinpath("a.tar.gz").write_bytes(b"a")

            first = RELEASE.write_checksums(root)
            second = RELEASE.write_checksums(root)

            self.assertEqual(first, second)
            self.assertEqual(
                first.splitlines(),
                [
                    f"{hashlib.sha256(b'a').hexdigest()}  a.tar.gz",
                    f"{hashlib.sha256(b'z').hexdigest()}  z.tar.gz",
                ],
            )


if __name__ == "__main__":
    unittest.main()
