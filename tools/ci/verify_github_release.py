#!/usr/bin/env python3
"""Verify that a GitHub Release contains only the prepared release assets."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from http.client import IncompleteRead
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen


MAX_RESPONSE_BYTES = 8 * 1024 * 1024
FULL_GIT_SHA = re.compile(r"[0-9a-f]{40}")
SHA256 = re.compile(r"[0-9a-f]{64}")


class ReleaseAssetError(RuntimeError):
    """The existing GitHub Release does not match the prepared asset contract."""


def _retry_after_seconds(error: HTTPError) -> float | None:
    raw = error.headers.get("Retry-After") if error.headers is not None else None
    if raw is None:
        return None
    try:
        return max(float(raw), 0.0)
    except ValueError:
        try:
            retry_at = parsedate_to_datetime(raw)
        except (TypeError, ValueError, OverflowError):
            return None
        if retry_at.tzinfo is None:
            retry_at = retry_at.replace(tzinfo=timezone.utc)
        return max((retry_at - datetime.now(timezone.utc)).total_seconds(), 0.0)


def _rate_limit_delay(error: HTTPError) -> float | None:
    retry_after = _retry_after_seconds(error)
    if retry_after is not None:
        return retry_after
    if error.headers is None or error.headers.get("X-RateLimit-Remaining") != "0":
        return None
    try:
        reset_at = float(error.headers["X-RateLimit-Reset"])
    except (KeyError, TypeError, ValueError):
        return None
    return max(reset_at - time.time(), 0.0)


def _request_json(
    url: str,
    token: str,
    *,
    allow_not_found: bool = False,
    timeout: float = 15.0,
    attempts: int = 4,
    max_elapsed: float = 60.0,
) -> Any | None:
    request = Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "dear-imgui-rs-release/1",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    deadline = time.monotonic() + max(max_elapsed, 0.1)
    delay = 1.0
    last_error: BaseException | None = None
    for attempt in range(max(attempts, 1)):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            with urlopen(
                request,
                timeout=min(max(timeout, 0.1), remaining),
            ) as response:
                payload = response.read(MAX_RESPONSE_BYTES + 1)
            last_error = None
            break
        except HTTPError as error:
            if allow_not_found and error.code == 404:
                return None
            retry_after = _rate_limit_delay(error)
            retryable = (
                error.code == 429
                or 500 <= error.code < 600
                or (error.code == 403 and retry_after is not None)
            )
            if not retryable:
                raise ReleaseAssetError(
                    f"GitHub API request failed with HTTP {error.code}: {url}"
                ) from error
            last_error = error
        except (IncompleteRead, OSError, URLError) as error:
            last_error = error
            retry_after = None

        if attempt + 1 >= max(attempts, 1):
            break
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        wait = min(retry_after if retry_after is not None else delay, remaining)
        if wait > 0:
            time.sleep(wait)
        delay = min(delay * 2, 8.0)
    else:  # pragma: no cover - the bounded loop always breaks or returns.
        last_error = RuntimeError("GitHub API retry loop exhausted")

    if last_error is not None:
        if isinstance(last_error, HTTPError):
            message = f"HTTP {last_error.code}"
        else:
            message = str(last_error)
        raise ReleaseAssetError(
            f"GitHub API request failed after bounded retries: {message}"
        ) from last_error

    if len(payload) > MAX_RESPONSE_BYTES:
        raise ReleaseAssetError("GitHub API response exceeds the safety limit")
    try:
        return json.loads(payload)
    except json.JSONDecodeError as error:
        raise ReleaseAssetError("GitHub API returned invalid JSON") from error


def _repository_parts(repository: str) -> tuple[str, str]:
    parts = repository.split("/")
    if len(parts) != 2 or any(
        re.fullmatch(r"[A-Za-z0-9_.-]+", part) is None for part in parts
    ):
        raise ReleaseAssetError(
            f"repository must use the owner/name form: {repository!r}"
        )
    return parts[0], parts[1]


def expected_asset_names(
    manifest_path: Path,
    *,
    tag: str,
    candidate_sha: str,
) -> set[str]:
    """Load and validate the exact user-facing asset names from a bundle."""
    try:
        manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseAssetError(f"invalid release manifest: {error}") from error
    if not isinstance(manifest, dict):
        raise ReleaseAssetError("release manifest must be a JSON object")
    if manifest.get("tag") != tag:
        raise ReleaseAssetError("release manifest tag does not match the workflow tag")
    if FULL_GIT_SHA.fullmatch(candidate_sha) is None:
        raise ReleaseAssetError(f"invalid candidate SHA: {candidate_sha!r}")
    if manifest.get("candidate_sha") != candidate_sha:
        raise ReleaseAssetError(
            "release manifest candidate does not match the workflow candidate"
        )
    assets = manifest.get("assets")
    if not isinstance(assets, list) or not assets:
        raise ReleaseAssetError("release manifest has no assets")

    names = {"SHA256SUMS"}
    for entry in assets:
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            raise ReleaseAssetError("release manifest contains an invalid asset")
        name = entry["name"]
        if (
            not name
            or name in (".", "..")
            or Path(name).name != name
            or "/" in name
            or "\\" in name
        ):
            raise ReleaseAssetError(f"invalid release asset name: {name!r}")
        digest = entry.get("sha256")
        if not isinstance(digest, str) or SHA256.fullmatch(digest) is None:
            raise ReleaseAssetError(f"invalid release asset checksum: {name}")
        if name in names:
            raise ReleaseAssetError(f"duplicate release asset name: {name}")
        names.add(name)
    return names


def existing_asset_names(
    *,
    api_url: str,
    repository: str,
    tag: str,
    token: str,
) -> set[str] | None:
    """Return every explicit asset on an existing release, or None if absent."""
    owner, name = _repository_parts(repository)
    base = api_url.rstrip("/")
    release_url = (
        f"{base}/repos/{quote(owner, safe='')}/{quote(name, safe='')}"
        f"/releases/tags/{quote(tag, safe='')}"
    )
    release = _request_json(release_url, token, allow_not_found=True)
    if release is None:
        return None
    if not isinstance(release, dict) or type(release.get("id")) is not int:
        raise ReleaseAssetError("GitHub release response has no numeric id")

    release_id = release["id"]
    names: set[str] = set()
    for page in range(1, 101):
        query = urlencode({"per_page": 100, "page": page})
        assets = _request_json(
            f"{base}/repos/{quote(owner, safe='')}/{quote(name, safe='')}"
            f"/releases/{release_id}/assets?{query}",
            token,
        )
        if not isinstance(assets, list):
            raise ReleaseAssetError("GitHub release assets response is not a list")
        for asset in assets:
            if not isinstance(asset, dict) or not isinstance(asset.get("name"), str):
                raise ReleaseAssetError("GitHub returned an invalid release asset")
            asset_name = asset["name"]
            if not asset_name:
                raise ReleaseAssetError("GitHub returned an empty release asset name")
            if asset_name in names:
                raise ReleaseAssetError(
                    f"GitHub release contains duplicate asset name: {asset_name}"
                )
            names.add(asset_name)
        if len(assets) < 100:
            return names
    raise ReleaseAssetError("GitHub release asset pagination exceeded 100 pages")


def verify_asset_inventory(
    expected: set[str],
    existing: set[str] | None,
    *,
    require_exact: bool,
) -> None:
    """Allow resumable subsets before upload and require equality afterwards."""
    if existing is None:
        if require_exact:
            raise ReleaseAssetError("GitHub Release does not exist after upload")
        return
    unexpected = sorted(existing - expected)
    if unexpected:
        raise ReleaseAssetError(
            "GitHub Release contains unexpected assets: " + ", ".join(unexpected)
        )
    if require_exact:
        missing = sorted(expected - existing)
        if missing:
            raise ReleaseAssetError(
                "GitHub Release is missing expected assets: " + ", ".join(missing)
            )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument(
        "--require-exact",
        action="store_true",
        help="Require the release to exist and contain every prepared asset",
    )
    return parser


def main() -> int:
    args = build_parser().parse_args()
    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print("error: GITHUB_TOKEN is required", file=sys.stderr)
        return 1
    try:
        expected = expected_asset_names(
            args.manifest,
            tag=args.tag,
            candidate_sha=args.candidate_sha,
        )
        existing = existing_asset_names(
            api_url=os.environ.get("GITHUB_API_URL", "https://api.github.com"),
            repository=args.repository,
            tag=args.tag,
            token=token,
        )
        verify_asset_inventory(
            expected,
            existing,
            require_exact=args.require_exact,
        )
    except ReleaseAssetError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    state = "exact" if args.require_exact else "compatible"
    count = 0 if existing is None else len(existing)
    print(f"GitHub Release asset inventory is {state} ({count} existing assets).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
