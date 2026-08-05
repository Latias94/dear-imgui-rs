#!/usr/bin/env python3
"""Small release guards around GitHub CLI and local asset digests."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import urllib.parse
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any, NamedTuple


FULL_GIT_SHA = re.compile(r"[0-9a-f]{40}")
RELEASE_TAG = re.compile(r"v[0-9A-Za-z][0-9A-Za-z.+-]*")


class ReleaseWorkflowError(RuntimeError):
    """A release guard rejected the candidate or remote state."""


class AssetInspection(NamedTuple):
    """Validated state of one GitHub Release asset set."""

    release_state: str
    missing: tuple[str, ...]


def _candidate_sha(value: str) -> str:
    if FULL_GIT_SHA.fullmatch(value) is None:
        raise ReleaseWorkflowError(
            "candidate SHA must be exactly 40 lowercase hexadecimal characters"
        )
    return value


def _release_tag(value: str) -> str:
    if RELEASE_TAG.fullmatch(value) is None:
        raise ReleaseWorkflowError(f"invalid release tag: {value!r}")
    return value


def _repository_path(repository: str) -> str:
    parts = repository.split("/")
    if len(parts) != 2 or not all(parts):
        raise ReleaseWorkflowError(
            f"repository must use the owner/name form, found {repository!r}"
        )
    return "/".join(urllib.parse.quote(part, safe="") for part in parts)


def _run(arguments: Sequence[str], *, accepted: Sequence[int] = (0,)):
    result = subprocess.run(
        [str(argument) for argument in arguments],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode not in accepted:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ReleaseWorkflowError(
            f"command failed ({result.returncode}): {' '.join(arguments)}"
            + (f": {detail}" if detail else "")
        )
    return result


def _json_output(arguments: Sequence[str]) -> Any:
    result = _run(arguments)
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ReleaseWorkflowError(
            f"command returned invalid JSON: {' '.join(arguments)}: {error}"
        ) from error


def select_latest_exact_sha_ci_run(
    runs: Sequence[Mapping[str, object]], candidate_sha: str
) -> Mapping[str, object]:
    """Select the latest main push CI run for one exact candidate SHA."""
    candidate_sha = _candidate_sha(candidate_sha)
    matching = [
        run
        for run in runs
        if run.get("head_sha") == candidate_sha
        and run.get("head_branch") == "main"
        and run.get("event") == "push"
    ]
    if not matching:
        raise ReleaseWorkflowError("no exact-SHA main push CI run found")
    return max(
        matching,
        key=lambda run: (str(run.get("created_at", "")), int(run.get("id", 0))),
    )


def require_successful_ci(repository: str, candidate_sha: str) -> Mapping[str, object]:
    """Require the latest exact-SHA main push CI run to have succeeded."""
    candidate_sha = _candidate_sha(candidate_sha)
    repository_path = _repository_path(repository)
    query = urllib.parse.urlencode(
        {
            "branch": "main",
            "head_sha": candidate_sha,
            "event": "push",
            "per_page": 100,
        }
    )
    payload = _json_output(
        (
            "gh",
            "api",
            f"repos/{repository_path}/actions/workflows/ci.yml/runs?{query}",
        )
    )
    if not isinstance(payload, dict) or not isinstance(payload.get("workflow_runs"), list):
        raise ReleaseWorkflowError("workflow runs response has no workflow_runs array")
    latest = select_latest_exact_sha_ci_run(payload["workflow_runs"], candidate_sha)
    if latest.get("status") != "completed" or latest.get("conclusion") != "success":
        raise ReleaseWorkflowError("latest exact-SHA main push CI run is not successful")
    return latest


def resolve_remote_tag(output: str, tag: str) -> str | None:
    """Resolve lightweight or annotated ls-remote output to its commit target."""
    tag = _release_tag(tag)
    direct_ref = f"refs/tags/{tag}"
    peeled_ref = f"{direct_ref}^{{}}"
    refs: dict[str, str] = {}
    for line in output.splitlines():
        fields = line.split()
        if len(fields) != 2 or FULL_GIT_SHA.fullmatch(fields[0]) is None:
            raise ReleaseWorkflowError("git ls-remote returned malformed tag data")
        refs[fields[1]] = fields[0]
    return refs.get(peeled_ref, refs.get(direct_ref))


def _remote_tag_target(tag: str) -> str | None:
    result = _run(
        (
            "git",
            "ls-remote",
            "--tags",
            "origin",
            f"refs/tags/{tag}",
            f"refs/tags/{tag}^{{}}",
        )
    )
    return resolve_remote_tag(result.stdout, tag)


def reserve_release_tag(repository: str, tag: str, candidate_sha: str) -> None:
    """Create the remote lightweight tag once, or verify an existing target."""
    repository_path = _repository_path(repository)
    tag = _release_tag(tag)
    candidate_sha = _candidate_sha(candidate_sha)
    target = _remote_tag_target(tag)
    if target is None:
        created = _run(
            (
                "gh",
                "api",
                "--method",
                "POST",
                f"repos/{repository_path}/git/refs",
                "-f",
                f"ref=refs/tags/{tag}",
                "-f",
                f"sha={candidate_sha}",
            ),
            accepted=(0, 1),
        )
        if created.returncode == 0:
            try:
                response = json.loads(created.stdout)
                target = response["object"]["sha"]
            except (KeyError, TypeError, json.JSONDecodeError) as error:
                raise ReleaseWorkflowError(
                    "GitHub returned invalid tag creation data"
                ) from error
        else:
            target = _remote_tag_target(tag)
            if target is None:
                detail = created.stderr.strip() or created.stdout.strip()
                raise ReleaseWorkflowError(
                    "release tag creation failed and no competing tag appeared"
                    + (f": {detail}" if detail else "")
                )
    if target != candidate_sha:
        raise ReleaseWorkflowError(
            f"release tag points to {target}, expected candidate {candidate_sha}"
        )


def write_checksums(asset_root: Path) -> str:
    """Write a deterministic SHA256SUMS file for all release archives."""
    asset_root = Path(asset_root)
    archives = sorted(asset_root.glob("*.tar.gz"), key=lambda path: path.name)
    if not archives:
        raise ReleaseWorkflowError("no prebuilt packages were downloaded")
    lines = []
    for path in archives:
        with path.open("rb") as archive:
            digest = hashlib.file_digest(archive, "sha256").hexdigest()
        lines.append(f"{digest}  {path.name}")
    contents = "\n".join(lines) + "\n"
    asset_root.joinpath("SHA256SUMS").write_text(
        contents, encoding="utf-8", newline="\n"
    )
    return contents


def expected_asset_digests(asset_root: Path) -> dict[str, str]:
    """Hash the complete local release asset set."""
    files = sorted(
        (path for path in Path(asset_root).iterdir() if path.is_file()),
        key=lambda path: path.name,
    )
    if not files:
        raise ReleaseWorkflowError("the release asset set is empty")
    digests = {}
    for path in files:
        with path.open("rb") as source:
            digests[path.name] = hashlib.file_digest(source, "sha256").hexdigest()
    return digests


def inspect_release_assets(
    release: Mapping[str, object],
    expected: Mapping[str, str],
    *,
    require_complete: bool,
) -> AssetInspection:
    """Validate existing asset names and GitHub-computed SHA-256 digests."""
    assets = release.get("assets")
    if not isinstance(assets, list):
        raise ReleaseWorkflowError("GitHub Release has no assets array")
    by_name: dict[str, Mapping[str, object]] = {}
    for raw_asset in assets:
        if not isinstance(raw_asset, dict):
            raise ReleaseWorkflowError("GitHub Release asset must be an object")
        name = raw_asset.get("name")
        if not isinstance(name, str) or not name:
            raise ReleaseWorkflowError("GitHub Release asset has no valid name")
        if name in by_name:
            raise ReleaseWorkflowError(f"GitHub Release repeats asset {name!r}")
        by_name[name] = raw_asset

    unexpected = sorted(set(by_name) - set(expected))
    if unexpected:
        raise ReleaseWorkflowError(
            f"existing release contains unexpected assets: {unexpected}"
        )
    for name, asset in sorted(by_name.items()):
        if asset.get("state") not in (None, "uploaded"):
            raise ReleaseWorkflowError(f"release asset {name!r} is not uploaded")
        actual = asset.get("digest")
        wanted = f"sha256:{expected[name]}"
        if actual != wanted:
            raise ReleaseWorkflowError(
                f"release asset digest mismatch for {name}: "
                f"expected {wanted}, found {actual!r}"
            )

    missing = tuple(sorted(set(expected) - set(by_name)))
    if require_complete and missing:
        raise ReleaseWorkflowError(
            f"GitHub Release is missing expected assets: {list(missing)}"
        )
    state = "draft" if release.get("draft") is True else "published"
    return AssetInspection(state, missing)


def _find_release_by_tag(repository: str, tag: str) -> Mapping[str, object] | None:
    repository_path = _repository_path(repository)
    tag = _release_tag(tag)
    payload = _json_output(
        (
            "gh",
            "api",
            "--paginate",
            "--slurp",
            f"repos/{repository_path}/releases?per_page=100",
        )
    )
    if not isinstance(payload, list):
        raise ReleaseWorkflowError("GitHub Releases response must be an array")
    releases = []
    for page in payload:
        if not isinstance(page, list):
            raise ReleaseWorkflowError("GitHub Releases page must be an array")
        releases.extend(page)
    matches = [
        release
        for release in releases
        if isinstance(release, dict) and release.get("tag_name") == tag
    ]
    if len(matches) > 1:
        raise ReleaseWorkflowError(f"multiple GitHub Releases use tag {tag!r}")
    return matches[0] if matches else None


def verify_release_assets(
    repository: str,
    tag: str,
    asset_root: Path,
    *,
    allow_absent: bool,
    require_complete: bool,
) -> AssetInspection:
    """Validate the release asset subset before or after the upload action."""
    expected = expected_asset_digests(asset_root)
    release = _find_release_by_tag(repository, tag)
    if release is None:
        if not allow_absent:
            raise ReleaseWorkflowError("GitHub Release was not found after upload")
        return AssetInspection("absent", tuple(sorted(expected)))
    return inspect_release_assets(
        release, expected, require_complete=require_complete
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    require_ci = commands.add_parser("require-ci")
    require_ci.add_argument("--repository", required=True)
    require_ci.add_argument("--candidate-sha", required=True)

    reserve_tag = commands.add_parser("reserve-tag")
    reserve_tag.add_argument("--repository", required=True)
    reserve_tag.add_argument("--tag", required=True)
    reserve_tag.add_argument("--candidate-sha", required=True)

    checksums = commands.add_parser("write-checksums")
    checksums.add_argument("--asset-root", required=True, type=Path)

    assets = commands.add_parser("verify-assets")
    assets.add_argument("--repository", required=True)
    assets.add_argument("--tag", required=True)
    assets.add_argument("--asset-root", required=True, type=Path)
    assets.add_argument("--allow-absent", action="store_true")
    assets.add_argument("--require-complete", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        if args.command == "require-ci":
            run = require_successful_ci(args.repository, args.candidate_sha)
            print(f"Verified exact-SHA CI run {run.get('id')}")
        elif args.command == "reserve-tag":
            reserve_release_tag(args.repository, args.tag, args.candidate_sha)
            print("Reserved or verified the exact release tag")
        elif args.command == "write-checksums":
            write_checksums(args.asset_root)
            print(f"Wrote {args.asset_root / 'SHA256SUMS'}")
        elif args.command == "verify-assets":
            state = verify_release_assets(
                args.repository,
                args.tag,
                args.asset_root,
                allow_absent=args.allow_absent,
                require_complete=args.require_complete,
            )
            print(
                f"Verified {state.release_state} release assets; "
                f"missing={len(state.missing)}"
            )
        else:  # pragma: no cover - argparse enforces the command set.
            raise AssertionError(f"unknown command: {args.command}")
    except ReleaseWorkflowError as error:
        print(f"release-workflow: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
