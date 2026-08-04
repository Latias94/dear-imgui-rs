#!/usr/bin/env python3
"""
Automated publishing script for dear-imgui-rs workspace.

This script publishes all crates in the correct dependency order, ensuring that
dependencies are published before their dependents.

Publishing Order:
1. Tooling: dear-imgui-build-support
2. Core: dear-imgui-sys -> dear-imgui-rs
3. Backends: dear-imgui-winit, dear-imgui-sdl3, dear-imgui-wgpu, dear-imgui-glow, dear-imgui-ash
4. Extensions (sys): dear-implot-sys, dear-imnodes-sys, dear-node-editor-sys,
                     dear-imguizmo-sys, dear-implot3d-sys, dear-imguizmo-quat-sys,
                     dear-imgui-test-engine-sys
5. Extensions (high-level): dear-implot, dear-imnodes, dear-node-editor,
                            dear-imguizmo, dear-implot3d, dear-imguizmo-quat,
                            dear-imgui-test-engine, dear-file-browser,
                            dear-imgui-reflect-derive, dear-imgui-reflect
6. Bevy backend: dear-imgui-bevy
7. Application: dear-app

Usage:
  # Dry run (show what would be published)
  python3 tools/publish.py --dry-run

  # Cargo dry run (run cargo publish --dry-run for selected crates)
  python3 tools/publish.py --cargo-dry-run --crates dear-imgui-build-support

  # Upload the complete train from an authorized release environment
  python3 tools/publish.py --yes

  # Verify that every exact workspace version is available
  python3 tools/publish.py --verify-published

Requirements:
  - cargo in PATH
  - CARGO_REGISTRY_TOKEN for uploads (CI obtains a short-lived OIDC token)
  - All crates must have correct versions in Cargo.toml
  - Pregenerated bindings must be up-to-date for -sys crates
"""

import argparse
import gzip
import io
import json
import re
import subprocess
import sys
import tarfile
import time
from enum import Enum
from http.client import IncompleteRead
from pathlib import Path, PurePosixPath
from typing import Callable, List, Optional
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

from release_metadata import (
    MetadataError,
    PUBLISH_ORDER,
    WorkspaceMetadata,
    load_workspace_metadata,
    validate_publish_order,
    validate_release_workspace,
)


class Colors:
    """ANSI color codes for terminal output."""
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKCYAN = '\033[96m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'


class RegistryState(Enum):
    """Exact crates.io version state."""

    PRESENT = "present"
    ABSENT = "absent"
    UNAVAILABLE = "unavailable"


class PublicationStatus(Enum):
    """Terminal state for one package in a release run."""

    PREVIEWED = "previewed"
    VERIFIED = "verified"
    ALREADY_PUBLISHED = "already-published"
    PUBLISHED = "published"


class RegistryProvenanceError(RuntimeError):
    """A published crate cannot be safely bound to the release candidate."""


MAX_CRATE_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_CRATE_UNPACKED_BYTES = 128 * 1024 * 1024
MAX_CRATE_MEMBERS = 20_000
MAX_VCS_INFO_BYTES = 64 * 1024
FULL_GIT_SHA = re.compile(r"[0-9a-f]{40}")


def print_header(msg: str):
    """Print a header message."""
    print(f"\n{Colors.HEADER}{Colors.BOLD}{'=' * 80}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{msg}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{'=' * 80}{Colors.ENDC}\n")


def print_info(msg: str):
    """Print an info message."""
    print(f"{Colors.OKBLUE}INFO: {msg}{Colors.ENDC}")


def print_success(msg: str):
    """Print a success message."""
    print(f"{Colors.OKGREEN}OK: {msg}{Colors.ENDC}")


def print_warning(msg: str):
    """Print a warning message."""
    print(f"{Colors.WARNING}WARN: {msg}{Colors.ENDC}")


def print_error(msg: str):
    """Print an error message."""
    print(f"{Colors.FAIL}ERR: {msg}{Colors.ENDC}", file=sys.stderr)


def run_command(
    cmd: List[str],
    cwd: Optional[Path] = None,
    dry_run: bool = False,
    capture: bool = False,
    timeout: float | None = None,
) -> int:
    """
    Run a command and return its exit code.

    Args:
        cmd: Command to run
        cwd: Working directory
        dry_run: If True, only print the command without executing
        capture: If True, capture output; if False, stream output in real-time
        timeout: Optional process timeout in seconds
    """
    cmd_str = " ".join(str(c) for c in cmd)
    print_info(f"Running: {cmd_str}")

    if dry_run:
        print_warning("DRY RUN: Command not executed")
        return 0

    try:
        if capture:
            # Capture output for processing
            result = subprocess.run(
                cmd,
                cwd=cwd,
                check=True,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=timeout,
            )
            if result.stdout:
                print(result.stdout)
            return 0
        else:
            # Stream output in real-time
            result = subprocess.run(cmd, cwd=cwd, check=True, timeout=timeout)
            return 0
    except subprocess.TimeoutExpired:
        print_error("Command timed out")
        return 124
    except subprocess.CalledProcessError as e:
        print_error(f"Command failed with exit code {e.returncode}")
        return e.returncode


def query_crate_version(
    crate_name: str, version: str, *, timeout: float = 15.0
) -> RegistryState:
    """Query one exact crate version without confusing absence with network failure."""
    crate = quote(crate_name, safe="")
    crate_version = quote(version, safe="")
    request = Request(
        f"https://crates.io/api/v1/crates/{crate}/{crate_version}",
        headers={"User-Agent": "dear-imgui-rs-release/1"},
    )
    try:
        with urlopen(request, timeout=max(timeout, 0.1)) as response:
            return (
                RegistryState.PRESENT
                if getattr(response, "status", None) == 200
                else RegistryState.UNAVAILABLE
            )
    except HTTPError as error:
        if error.code == 404:
            return RegistryState.ABSENT
        return RegistryState.UNAVAILABLE
    except (OSError, URLError):
        return RegistryState.UNAVAILABLE


def resolve_registry_state(
    crate_name: str,
    version: str,
    *,
    attempts: int = 4,
    retry_delay: float = 2.0,
) -> RegistryState:
    """Retry transient registry failures without treating them as unpublished."""
    delay = max(retry_delay, 0.0)
    for attempt in range(max(attempts, 1)):
        state = query_crate_version(crate_name, version)
        if state is not RegistryState.UNAVAILABLE:
            return state
        if attempt + 1 < max(attempts, 1) and delay > 0:
            time.sleep(delay)
            delay = min(delay * 2, 10.0)
    return RegistryState.UNAVAILABLE


def crate_version_is_indexed(
    crate_name: str, version: str, *, timeout: float = 30.0
) -> bool:
    """Require Cargo's registry view to resolve the exact published version."""
    try:
        result = subprocess.run(
            [
                "cargo",
                "info",
                f"{crate_name}@{version}",
                "--registry",
                "crates-io",
            ],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
            timeout=max(timeout, 0.1),
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return result.returncode == 0


def query_crate_candidate_sha(
    crate_name: str,
    version: str,
    *,
    timeout: float = 15.0,
) -> str | None:
    """Return the clean Git SHA recorded in a published crate archive.

    Network and registry availability failures return ``None`` so callers can
    retry. A malformed archive fails closed because it cannot safely represent
    the requested release candidate.
    """
    crate = quote(crate_name, safe="")
    crate_version = quote(version, safe="")
    request = Request(
        f"https://crates.io/api/v1/crates/{crate}/{crate_version}/download",
        headers={"User-Agent": "dear-imgui-rs-release/1"},
    )
    try:
        with urlopen(request, timeout=max(timeout, 0.1)) as response:
            payload = response.read(MAX_CRATE_ARCHIVE_BYTES + 1)
    except (HTTPError, IncompleteRead, OSError, URLError):
        return None

    if len(payload) > MAX_CRATE_ARCHIVE_BYTES:
        raise RegistryProvenanceError(
            f"published archive for {crate_name} v{version} exceeds the "
            f"{MAX_CRATE_ARCHIVE_BYTES}-byte safety limit"
        )

    try:
        with gzip.GzipFile(fileobj=io.BytesIO(payload)) as compressed:
            unpacked = compressed.read(MAX_CRATE_UNPACKED_BYTES + 1)
        if len(unpacked) > MAX_CRATE_UNPACKED_BYTES:
            raise RegistryProvenanceError(
                f"published archive for {crate_name} v{version} exceeds the "
                f"{MAX_CRATE_UNPACKED_BYTES}-byte unpacked safety limit"
            )
    except RegistryProvenanceError:
        raise
    except (EOFError, gzip.BadGzipFile, OSError) as error:
        raise RegistryProvenanceError(
            f"invalid published archive for {crate_name} v{version}: {error}"
        ) from error

    expected_member = PurePosixPath(
        f"{crate_name}-{version}/.cargo_vcs_info.json"
    )
    vcs_payload: bytes | None = None
    try:
        with tarfile.open(fileobj=io.BytesIO(unpacked), mode="r:") as archive:
            for member_count, member in enumerate(archive, 1):
                if member_count > MAX_CRATE_MEMBERS:
                    raise RegistryProvenanceError(
                        f"published archive for {crate_name} v{version} exceeds the "
                        f"{MAX_CRATE_MEMBERS}-member safety limit"
                    )
                if PurePosixPath(member.name) != expected_member or not member.isfile():
                    continue
                if vcs_payload is not None:
                    raise RegistryProvenanceError(
                        f"published archive for {crate_name} v{version} contains "
                        f"duplicate {expected_member.as_posix()} entries"
                    )
                if member.size > MAX_VCS_INFO_BYTES:
                    raise RegistryProvenanceError(
                        f"Cargo VCS metadata for {crate_name} v{version} exceeds the "
                        f"{MAX_VCS_INFO_BYTES}-byte safety limit"
                    )
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise RegistryProvenanceError(
                        f"could not read Cargo VCS metadata for {crate_name} v{version}"
                    )
                vcs_payload = extracted.read(MAX_VCS_INFO_BYTES + 1)
    except RegistryProvenanceError:
        raise
    except (OSError, tarfile.TarError) as error:
        raise RegistryProvenanceError(
            f"invalid published archive for {crate_name} v{version}: {error}"
        ) from error

    if vcs_payload is None:
        raise RegistryProvenanceError(
            f"published archive for {crate_name} v{version} has no "
            f"{expected_member.as_posix()}"
        )
    try:
        vcs_info = json.loads(vcs_payload)
    except json.JSONDecodeError as error:
        raise RegistryProvenanceError(
            f"invalid Cargo VCS metadata for {crate_name} v{version}: {error}"
        ) from error

    if not isinstance(vcs_info, dict) or not isinstance(vcs_info.get("git"), dict):
        raise RegistryProvenanceError(
            f"published archive for {crate_name} v{version} has no Git provenance"
        )
    git = vcs_info["git"]
    candidate_sha = git.get("sha1")
    if (
        not isinstance(candidate_sha, str)
        or FULL_GIT_SHA.fullmatch(candidate_sha) is None
    ):
        raise RegistryProvenanceError(
            f"published archive for {crate_name} v{version} has an invalid Git SHA"
        )
    dirty = git.get("dirty", False)
    if not isinstance(dirty, bool):
        raise RegistryProvenanceError(
            f"published archive for {crate_name} v{version} has invalid "
            "dirty-state metadata"
        )
    if dirty:
        raise RegistryProvenanceError(
            f"published archive for {crate_name} v{version} was built from dirty sources"
        )
    return candidate_sha


def wait_for_crate_available(
    crate_name: str,
    version: str,
    *,
    expected_candidate_sha: str,
    timeout: float = 180.0,
    poll_interval: float = 2.0,
) -> bool:
    """Wait until registry, Cargo, and archive provenance agree on a version."""
    if FULL_GIT_SHA.fullmatch(expected_candidate_sha) is None:
        raise RegistryProvenanceError(
            f"invalid expected release candidate SHA: {expected_candidate_sha!r}"
        )
    deadline = time.monotonic() + max(timeout, 0.0)
    delay = max(poll_interval, 0.1)
    while True:
        remaining = max(deadline - time.monotonic(), 0.1)
        if query_crate_version(
            crate_name,
            version,
            timeout=min(15.0, remaining),
        ) is RegistryState.PRESENT and crate_version_is_indexed(
            crate_name,
            version,
            timeout=min(30.0, max(deadline - time.monotonic(), 0.1)),
        ):
            candidate_sha = query_crate_candidate_sha(
                crate_name,
                version,
                timeout=min(15.0, max(deadline - time.monotonic(), 0.1)),
            )
            if candidate_sha is not None:
                if candidate_sha != expected_candidate_sha:
                    raise RegistryProvenanceError(
                        f"published {crate_name} v{version} came from {candidate_sha}, "
                        f"not release candidate {expected_candidate_sha}"
                    )
                return True
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return False
        time.sleep(min(delay, remaining))
        delay = min(delay * 1.5, 10.0)


def write_publication_journal(
    path: Path | None,
    *,
    candidate_sha: str | None,
    release_version: str,
    packages: list[dict[str, str]],
    complete: bool,
) -> None:
    """Atomically persist resumable, non-secret release state."""
    if path is None:
        return
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    payload = {
        "version": 1,
        "candidate_sha": candidate_sha,
        "release_version": release_version,
        "registry": "crates-io",
        "complete": complete,
        "packages": packages,
    }
    temporary.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    temporary.replace(path)


def publish_crate(
    crate_name: str,
    crate_path: Path,
    version: str,
    repo_root: Path,
    dry_run: bool = False,
    cargo_dry_run: bool = False,
    no_verify: bool = False,
    candidate_sha: str | None = None,
    publish_timeout: float = 300.0,
    availability_timeout: float = 180.0,
    poll_interval: float = 2.0,
    source_guard: Optional[Callable[[], bool]] = None,
) -> PublicationStatus | None:
    """Publish a single crate."""
    print_header(f"Publishing {crate_name}")

    full_path = repo_root / crate_path
    if not full_path.exists():
        print_error(f"Crate path does not exist: {full_path}")
        return None

    print_info(f"Crate: {crate_name}")
    print_info(f"Version: {version}")
    print_info(f"Path: {crate_path}")

    if not dry_run and not cargo_dry_run:
        if candidate_sha is None:
            print_error("Actual publication requires a release candidate SHA")
            return None
        state = resolve_registry_state(crate_name, version)
        if state is RegistryState.UNAVAILABLE:
            print_error(
                f"Could not determine whether {crate_name} v{version} is published"
            )
            return None
        if state is RegistryState.PRESENT:
            try:
                available = wait_for_crate_available(
                    crate_name,
                    version,
                    expected_candidate_sha=candidate_sha,
                    timeout=availability_timeout,
                    poll_interval=poll_interval,
                )
            except RegistryProvenanceError as error:
                print_error(str(error))
                return None
            if not available:
                print_error(
                    f"{crate_name} v{version} exists but is not available to Cargo"
                )
                return None
            print_info(f"Skipping already published {crate_name} v{version}")
            return PublicationStatus.ALREADY_PUBLISHED

    # Build publish command
    cmd = [
        "cargo",
        "publish",
        "-p",
        crate_name,
        "--locked",
        "--registry",
        "crates-io",
    ]
    if cargo_dry_run:
        cmd.append("--dry-run")
    if no_verify:
        cmd.append("--no-verify")

    if source_guard is not None and not source_guard():
        print_error(f"Release source changed before invoking cargo publish for {crate_name}")
        return None

    # Execute publish (stream output in real-time, don't capture)
    result = run_command(
        cmd,
        cwd=repo_root,
        dry_run=dry_run,
        capture=False,
        timeout=(publish_timeout if not dry_run and not cargo_dry_run else None),
    )

    if result != 0 and (dry_run or cargo_dry_run):
        action = "cargo dry-run publish" if cargo_dry_run else "publish"
        print_error(f"Failed to {action} {crate_name}")
        return None

    if dry_run:
        print_success(f"Dry run: would publish {crate_name} v{version}")
        return PublicationStatus.PREVIEWED
    elif cargo_dry_run:
        print_success(f"Cargo dry-run publish succeeded for {crate_name} v{version}")
        return PublicationStatus.VERIFIED

    if result != 0:
        print_warning(
            f"cargo publish returned {result}; reconciling exact registry state"
        )
    try:
        available = wait_for_crate_available(
            crate_name,
            version,
            expected_candidate_sha=candidate_sha,
            timeout=availability_timeout,
            poll_interval=poll_interval,
        )
    except RegistryProvenanceError as error:
        print_error(str(error))
        return None
    if not available:
        print_error(
            f"{crate_name} v{version} did not become available before timeout"
        )
        return None
    print_success(f"Successfully published {crate_name} v{version}")
    return PublicationStatus.PUBLISHED


def validate_release_configuration(
    metadata: WorkspaceMetadata, repo_root: Path
) -> list[str]:
    """Validate release versions and the hand-authored dependency order."""
    return [
        *validate_release_workspace(metadata),
        *validate_publish_order(metadata, PUBLISH_ORDER, repo_root),
    ]


def run_release_preflight(repo_root: Path) -> int:
    """Run the strict committed-tree gate immediately before an upload."""
    print_header("Strict Release Preflight")
    return run_command(
        [sys.executable, "tools/pre_publish_check.py"],
        cwd=repo_root,
    )


def capture_release_fingerprint(repo_root: Path) -> Optional[str]:
    """Return the clean HEAD that subsequent publish commands must retain."""
    commands = [
        ["git", "rev-parse", "--verify", "HEAD"],
        [
            "git",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
    ]
    results = []
    for command in commands:
        try:
            result = subprocess.run(
                command,
                cwd=repo_root,
                check=False,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
            )
        except OSError as error:
            print_error(f"Could not inspect release source: {error}")
            return None
        if result.returncode != 0:
            detail = result.stderr.strip() or "Git command failed"
            print_error(detail)
            return None
        results.append(result.stdout.strip())

    head, status = results
    if status:
        print_error("Publishing requires a completely clean worktree:")
        print(status, file=sys.stderr)
        return None
    if FULL_GIT_SHA.fullmatch(head) is None:
        print_error(f"Git returned an invalid release HEAD: {head!r}")
        return None
    return head


def verify_release_fingerprint(repo_root: Path, expected_head: str) -> bool:
    """Reject source edits or commits after the strict release preflight."""
    current_head = capture_release_fingerprint(repo_root)
    if current_head is None:
        return False
    if current_head != expected_head:
        print_error(
            f"Release HEAD changed after validation: {expected_head} -> {current_head}"
        )
        return False
    return True


def build_parser() -> argparse.ArgumentParser:
    """Build the canonical publish CLI parser for wrapper contract tests."""
    parser = argparse.ArgumentParser(
        description="Publish dear-imgui-rs workspace crates in dependency order",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        "--crates",
        help="Comma-separated preview/dry-run subset; uploads always use the full train"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show publish order and cargo commands without executing them"
    )
    parser.add_argument(
        "--cargo-dry-run",
        action="store_true",
        help="Run cargo publish --dry-run for every selected crate without uploading"
    )
    parser.add_argument(
        "--no-verify",
        action="store_true",
        help="Skip verification (pass --no-verify to cargo publish)"
    )
    parser.add_argument(
        "--yes",
        action="store_true",
        help="Confirm a full release upload without reading stdin",
    )
    parser.add_argument(
        "--verify-published",
        action="store_true",
        help="Verify that every exact workspace version is available without uploading",
    )
    parser.add_argument(
        "--index-timeout",
        type=float,
        default=180.0,
        help="Maximum seconds to wait for each exact version (default: 180)",
    )
    parser.add_argument(
        "--publish-timeout",
        type=float,
        default=300.0,
        help="Maximum seconds for one cargo publish process (default: 300)",
    )
    parser.add_argument(
        "--poll-interval",
        type=float,
        default=2.0,
        help="Initial registry polling interval in seconds (default: 2)",
    )
    parser.add_argument(
        "--journal",
        type=Path,
        help="Write an atomic machine-readable publication journal",
    )
    return parser


def main() -> int:
    parser = build_parser()

    args = parser.parse_args()

    selected_modes = sum(
        (args.dry_run, args.cargo_dry_run, args.verify_published)
    )
    if selected_modes > 1:
        print_error(
            "--dry-run, --cargo-dry-run, and --verify-published are mutually exclusive"
        )
        return 1
    if (
        args.index_timeout < 0
        or args.publish_timeout <= 0
        or args.poll_interval <= 0
    ):
        print_error(
            "--index-timeout must be non-negative; --publish-timeout and "
            "--poll-interval must be positive"
        )
        return 1

    actual_upload = not (
        args.dry_run or args.cargo_dry_run or args.verify_published
    )
    if actual_upload and args.crates:
        print_error("Actual uploads always publish the complete release train")
        return 1
    if args.verify_published and args.crates:
        print_error("Published-release verification always checks the complete train")
        return 1
    
    repo_root = Path(__file__).resolve().parents[1]

    if args.crates:
        requested_crates = set(c.strip() for c in args.crates.split(","))
        crates_to_publish = [
            (name, path) for name, path in PUBLISH_ORDER
            if name in requested_crates
        ]
        # Check for unknown crates
        known_crates = {name for name, _ in PUBLISH_ORDER}
        unknown = requested_crates - known_crates
        if unknown:
            print_error(f"Unknown crates: {', '.join(unknown)}")
            print_info(f"Known crates: {', '.join(known_crates)}")
            return 1
    else:
        crates_to_publish = PUBLISH_ORDER

    release_head: Optional[str] = None
    if actual_upload or args.cargo_dry_run or args.verify_published:
        release_head = capture_release_fingerprint(repo_root)
        if release_head is None:
            return 1
        if args.cargo_dry_run:
            preflight = run_release_preflight(repo_root)
            if preflight != 0:
                print_error("Strict release preflight failed")
                return preflight
            if not verify_release_fingerprint(repo_root, release_head):
                print_error("Release source changed during preflight")
                return 1

    try:
        metadata = load_workspace_metadata(repo_root)
    except MetadataError as error:
        print_error(str(error))
        return 1

    configuration_errors = validate_release_configuration(metadata, repo_root)
    if configuration_errors:
        print_error("Release metadata or PUBLISH_ORDER is inconsistent:")
        for error in configuration_errors:
            print_error(f"  {error}")
        return 1
    package_versions = {
        package.name: package.version for package in metadata.publishable_packages
    }
    if release_head is not None and not verify_release_fingerprint(
        repo_root, release_head
    ):
        print_error("Release source changed while loading metadata")
        return 1

    if args.dry_run:
        preflight_status = "not run for print-only dry-run"
    elif args.cargo_dry_run:
        preflight_status = "passed for this clean HEAD"
    elif actual_upload:
        preflight_status = "covered by the authoritative source-package cell"
    else:
        preflight_status = "not required for registry verification"

    journal_packages = [
        {
            "name": name,
            "version": package_versions[name],
            "status": "pending",
        }
        for name, _path in crates_to_publish
    ]
    journal_by_name = {entry["name"]: entry for entry in journal_packages}
    write_publication_journal(
        args.journal,
        candidate_sha=release_head,
        release_version=metadata.release_version,
        packages=journal_packages,
        complete=False,
    )
    
    # Print summary
    print_header("Publishing Summary")
    print_info(f"Repository: {repo_root}")
    print_info(f"Release version: {metadata.release_version}")
    print_info(
        f"Workspace packages: {len(metadata.publishable_packages)} publishable, "
        f"{len(metadata.private_packages)} private"
    )
    print_info(f"Crates to publish: {len(crates_to_publish)}")
    print_info(f"Dry run: {args.dry_run}")
    print_info(f"Cargo dry run: {args.cargo_dry_run}")
    print_info(f"Verify published: {args.verify_published}")
    print_info(f"No verify: {args.no_verify}")
    print_info(f"Strict local release preflight: {preflight_status}")
    print_info(f"Per-crate index timeout: {args.index_timeout:g}s")
    print_info(f"Per-crate publish timeout: {args.publish_timeout:g}s")
    print()
    print("Publishing order:")
    for i, (name, path) in enumerate(crates_to_publish, 1):
        print(f"  {i}. {name} ({path})")
    print()
    
    if actual_upload and not args.yes:
        try:
            response = input("Continue with publishing? [y/N]: ").strip().lower()
        except EOFError:
            print_error("Confirmation requires an interactive terminal or --yes")
            return 1
        if response not in ('y', 'yes'):
            print_info("Publishing cancelled")
            return 0

    if args.verify_published:
        assert release_head is not None
        for name, _path in crates_to_publish:
            version = package_versions[name]
            try:
                available = wait_for_crate_available(
                    name,
                    version,
                    expected_candidate_sha=release_head,
                    timeout=args.index_timeout,
                    poll_interval=args.poll_interval,
                )
            except RegistryProvenanceError as error:
                print_error(str(error))
                available = False
            if not available:
                journal_by_name[name]["status"] = "failed"
                write_publication_journal(
                    args.journal,
                    candidate_sha=release_head,
                    release_version=metadata.release_version,
                    packages=journal_packages,
                    complete=False,
                )
                print_error(f"{name} v{version} is not available from crates.io")
                return 1
            journal_by_name[name]["status"] = "already-published"
        write_publication_journal(
            args.journal,
            candidate_sha=release_head,
            release_version=metadata.release_version,
            packages=journal_packages,
            complete=True,
        )
        print_success(
            f"Verified all {len(crates_to_publish)} exact crate versions on crates.io."
        )
        return 0

    for name, path in crates_to_publish:
        if release_head is not None and not verify_release_fingerprint(
            repo_root, release_head
        ):
            print_error("Release source changed before publish; stopping immediately")
            return 1
        status = publish_crate(
            name,
            Path(path),
            package_versions[name],
            repo_root,
            dry_run=args.dry_run,
            cargo_dry_run=args.cargo_dry_run,
            no_verify=args.no_verify,
            candidate_sha=release_head,
            publish_timeout=args.publish_timeout,
            availability_timeout=args.index_timeout,
            poll_interval=args.poll_interval,
            source_guard=(
                (lambda: verify_release_fingerprint(repo_root, release_head))
                if release_head is not None
                else None
            ),
        )
        if status is None:
            journal_by_name[name]["status"] = "failed"
            write_publication_journal(
                args.journal,
                candidate_sha=release_head,
                release_version=metadata.release_version,
                packages=journal_packages,
                complete=False,
            )
            if release_head is not None and not verify_release_fingerprint(
                repo_root, release_head
            ):
                print_error("Release source changed; refusing to continue publishing")
                return 1
            print_error(f"Failed to publish {name}")
            return 1
        journal_by_name[name]["status"] = status.value
        write_publication_journal(
            args.journal,
            candidate_sha=release_head,
            release_version=metadata.release_version,
            packages=journal_packages,
            complete=False,
        )

    write_publication_journal(
        args.journal,
        candidate_sha=release_head,
        release_version=metadata.release_version,
        packages=journal_packages,
        complete=True,
    )
    print_header("Publishing Complete")
    if args.dry_run:
        print_success(
            f"Preview complete for all {len(crates_to_publish)} selected crate(s)."
        )
    elif args.cargo_dry_run:
        print_success(
            f"Cargo dry-run succeeded for all {len(crates_to_publish)} crate(s)."
        )
    else:
        print_success(
            f"Successfully reconciled all {len(crates_to_publish)} crate(s)."
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
