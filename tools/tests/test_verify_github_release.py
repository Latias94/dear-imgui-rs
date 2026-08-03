import importlib.util
import json
import unittest
from http.client import IncompleteRead
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "verify_github_release",
    REPO_ROOT / "tools/ci/verify_github_release.py",
)
VERIFY = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(VERIFY)


class JsonResponse:
    def __init__(self, payload):
        self.payload = json.dumps(payload).encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self, _limit):
        return self.payload


class ReleaseAssetInventoryTests(unittest.TestCase):
    candidate_sha = "a" * 40

    def write_manifest(self, directory: str) -> Path:
        path = Path(directory) / "release-manifest.json"
        path.write_text(
            json.dumps(
                {
                    "version": 1,
                    "tag": "v0.16.0-alpha.1",
                    "candidate_sha": self.candidate_sha,
                    "assets": [
                        {"name": "linux.tar.gz", "sha256": "b" * 64},
                        {"name": "windows.tar.gz", "sha256": "c" * 64},
                    ],
                }
            ),
            encoding="utf-8",
        )
        return path

    def test_manifest_adds_checksum_asset_to_exact_inventory(self):
        with TemporaryDirectory() as directory:
            expected = VERIFY.expected_asset_names(
                self.write_manifest(directory),
                tag="v0.16.0-alpha.1",
                candidate_sha=self.candidate_sha,
            )

        self.assertEqual(
            expected,
            {"SHA256SUMS", "linux.tar.gz", "windows.tar.gz"},
        )

    def test_manifest_is_bound_to_tag_and_candidate(self):
        with TemporaryDirectory() as directory:
            manifest = self.write_manifest(directory)
            with self.assertRaisesRegex(VERIFY.ReleaseAssetError, "tag"):
                VERIFY.expected_asset_names(
                    manifest,
                    tag="v0.16.0-alpha.2",
                    candidate_sha=self.candidate_sha,
                )
            with self.assertRaisesRegex(VERIFY.ReleaseAssetError, "candidate"):
                VERIFY.expected_asset_names(
                    manifest,
                    tag="v0.16.0-alpha.1",
                    candidate_sha="d" * 40,
                )

    def test_compatible_mode_allows_absent_or_partial_release(self):
        expected = {"SHA256SUMS", "linux.tar.gz"}

        VERIFY.verify_asset_inventory(expected, None, require_exact=False)
        VERIFY.verify_asset_inventory(
            expected,
            {"linux.tar.gz"},
            require_exact=False,
        )

    def test_compatible_mode_rejects_unexpected_assets(self):
        with self.assertRaisesRegex(VERIFY.ReleaseAssetError, "manual.zip"):
            VERIFY.verify_asset_inventory(
                {"SHA256SUMS", "linux.tar.gz"},
                {"linux.tar.gz", "manual.zip"},
                require_exact=False,
            )

    def test_exact_mode_requires_complete_release(self):
        expected = {"SHA256SUMS", "linux.tar.gz"}
        with self.assertRaisesRegex(VERIFY.ReleaseAssetError, "does not exist"):
            VERIFY.verify_asset_inventory(expected, None, require_exact=True)
        with self.assertRaisesRegex(VERIFY.ReleaseAssetError, "SHA256SUMS"):
            VERIFY.verify_asset_inventory(
                expected,
                {"linux.tar.gz"},
                require_exact=True,
            )

    def test_existing_assets_are_read_from_paginated_release_endpoint(self):
        responses = [
            {"id": 41},
            [{"name": f"asset-{index}"} for index in range(100)],
            [{"name": "final"}],
        ]

        with patch.object(
            VERIFY,
            "_request_json",
            side_effect=responses,
        ) as request:
            names = VERIFY.existing_asset_names(
                api_url="https://api.github.test",
                repository="owner/repo",
                tag="v0.16.0-alpha.1",
                token="token",
            )

        self.assertEqual(len(names), 101)
        self.assertIn("final", names)
        self.assertIn("page=2", request.call_args_list[-1].args[0])

    def test_github_api_retries_transient_http_failure(self):
        unavailable = VERIFY.HTTPError(
            "https://api.github.test/release",
            503,
            "unavailable",
            {"Retry-After": "0"},
            None,
        )
        with (
            patch.object(
                VERIFY,
                "urlopen",
                side_effect=[unavailable, JsonResponse({"id": 41})],
            ) as request,
            patch.object(VERIFY.time, "sleep") as sleep,
        ):
            payload = VERIFY._request_json(
                "https://api.github.test/release",
                "token",
                max_elapsed=1,
            )

        self.assertEqual(payload, {"id": 41})
        self.assertEqual(request.call_count, 2)
        sleep.assert_not_called()

    def test_github_api_does_not_retry_nonrecoverable_http_failure(self):
        forbidden = VERIFY.HTTPError(
            "https://api.github.test/release",
            403,
            "forbidden",
            {},
            None,
        )
        with (
            patch.object(VERIFY, "urlopen", side_effect=forbidden) as request,
            self.assertRaisesRegex(VERIFY.ReleaseAssetError, "HTTP 403"),
        ):
            VERIFY._request_json("https://api.github.test/release", "token")

        self.assertEqual(request.call_count, 1)

    def test_github_api_retries_secondary_rate_limit_403(self):
        limited = VERIFY.HTTPError(
            "https://api.github.test/release",
            403,
            "secondary rate limit",
            {"Retry-After": "0"},
            None,
        )
        with (
            patch.object(
                VERIFY,
                "urlopen",
                side_effect=[limited, JsonResponse({"id": 41})],
            ) as request,
            patch.object(VERIFY.time, "sleep") as sleep,
        ):
            payload = VERIFY._request_json(
                "https://api.github.test/release",
                "token",
                max_elapsed=1,
            )

        self.assertEqual(payload, {"id": 41})
        self.assertEqual(request.call_count, 2)
        sleep.assert_not_called()

    def test_github_api_retries_primary_rate_limit_403_until_reset(self):
        limited = VERIFY.HTTPError(
            "https://api.github.test/release",
            403,
            "rate limit",
            {"X-RateLimit-Remaining": "0", "X-RateLimit-Reset": "102"},
            None,
        )
        with (
            patch.object(
                VERIFY,
                "urlopen",
                side_effect=[limited, JsonResponse({"id": 41})],
            ) as request,
            patch.object(VERIFY.time, "time", return_value=100),
            patch.object(VERIFY.time, "sleep") as sleep,
        ):
            payload = VERIFY._request_json(
                "https://api.github.test/release",
                "token",
                max_elapsed=10,
            )

        self.assertEqual(payload, {"id": 41})
        self.assertEqual(request.call_count, 2)
        sleep.assert_called_once_with(2.0)

    def test_github_api_retries_incomplete_response(self):
        with (
            patch.object(
                VERIFY,
                "urlopen",
                side_effect=[IncompleteRead(b"{", 1), JsonResponse([])],
            ) as request,
            patch.object(VERIFY.time, "sleep"),
        ):
            payload = VERIFY._request_json(
                "https://api.github.test/release",
                "token",
                attempts=2,
                max_elapsed=1,
            )

        self.assertEqual(payload, [])
        self.assertEqual(request.call_count, 2)


if __name__ == "__main__":
    unittest.main()
