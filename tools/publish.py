#!/usr/bin/env python3
"""
Automated publishing script for dear-imgui-rs workspace.

This script publishes all crates in the correct dependency order, ensuring that
dependencies are published before their dependents.

Publishing Order:
1. Tooling: dear-imgui-build-support
2. Core: dear-imgui-sys -> dear-imgui-rs
3. Backends: dear-imgui-winit, dear-imgui-wgpu, dear-imgui-glow, dear-imgui-ash, dear-imgui-sdl3
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

  # Upload all crates after verifying the authoritative same-SHA release gate
  python3 tools/publish.py --release-gate-result artifacts/gate-result.json

  # Publish specific crates
  python3 tools/publish.py --release-gate-result gate-result.json --crates dear-imgui-sys,dear-imgui-rs

  # Skip verification (faster but not recommended)
  python3 tools/publish.py --release-gate-result gate-result.json --no-verify

  # Emergency-only upload without release evidence or the local release gate
  python3 tools/publish.py --dangerously-skip-release-check

  # Wait longer between publishes (for crates.io to index)
  python3 tools/publish.py --release-gate-result gate-result.json --wait 60

Requirements:
  - cargo in PATH
  - Logged in to crates.io (cargo login)
  - All crates must have correct versions in Cargo.toml
  - Pregenerated bindings must be up-to-date for -sys crates
"""

import argparse
import subprocess
import sys
import time
from pathlib import Path
from typing import Callable, List, Optional

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


def run_command(cmd: List[str], cwd: Optional[Path] = None, dry_run: bool = False, capture: bool = False) -> int:
    """
    Run a command and return its exit code.

    Args:
        cmd: Command to run
        cwd: Working directory
        dry_run: If True, only print the command without executing
        capture: If True, capture output; if False, stream output in real-time
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
            )
            if result.stdout:
                print(result.stdout)
            return 0
        else:
            # Stream output in real-time
            result = subprocess.run(cmd, cwd=cwd, check=True)
            return 0
    except subprocess.CalledProcessError as e:
        print_error(f"Command failed with exit code {e.returncode}")
        return e.returncode


def check_crate_published(crate_name: str, version: str) -> bool:
    """Check if a crate version is already published on crates.io."""
    try:
        result = subprocess.run(
            [
                "cargo",
                "search",
                crate_name,
                "--limit",
                "1",
                "--registry",
                "crates-io",
            ],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=True
        )
        # Output format: "crate_name = \"version\" # description"
        if result.stdout and f'{crate_name} = "{version}"' in result.stdout:
            return True
    except subprocess.CalledProcessError:
        pass
     
    return False


def publish_crate(
    crate_name: str,
    crate_path: Path,
    version: str,
    repo_root: Path,
    dry_run: bool = False,
    cargo_dry_run: bool = False,
    no_verify: bool = False,
    wait_time: int = 30,
    source_guard: Optional[Callable[[], bool]] = None,
) -> bool:
    """Publish a single crate."""
    print_header(f"Publishing {crate_name}")

    full_path = repo_root / crate_path
    if not full_path.exists():
        print_error(f"Crate path does not exist: {full_path}")
        return False

    print_info(f"Crate: {crate_name}")
    print_info(f"Version: {version}")
    print_info(f"Path: {crate_path}")

    # Check if already published
    if (
        not dry_run
        and not cargo_dry_run
        and check_crate_published(crate_name, version)
    ):
        print_warning(f"{crate_name} v{version} is already published on crates.io")
        response = input("Skip this crate? [Y/n]: ").strip().lower()
        if response in ('', 'y', 'yes'):
            print_info(f"Skipping {crate_name}")
            return True

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
        return False

    # Execute publish (stream output in real-time, don't capture)
    result = run_command(cmd, cwd=repo_root, dry_run=dry_run, capture=False)

    if result != 0:
        action = "cargo dry-run publish" if cargo_dry_run else "publish"
        print_error(f"Failed to {action} {crate_name}")
        return False

    if dry_run:
        print_success(f"Dry run: would publish {crate_name} v{version}")
    elif cargo_dry_run:
        print_success(f"Cargo dry-run publish succeeded for {crate_name} v{version}")
    else:
        print_success(f"Successfully published {crate_name} v{version}")

    # Wait for crates.io to index the crate
    if not dry_run and not cargo_dry_run and wait_time > 0:
        print_info(f"Waiting {wait_time} seconds for crates.io to index...")
        time.sleep(wait_time)

    return True


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


def release_gate_verification_command(
    repo_root: Path,
    candidate_sha: str,
    gate_result: Path,
) -> List[str]:
    """Build the authoritative same-SHA release evidence verification command."""
    return [
        sys.executable,
        "tools/ci/release_evidence.py",
        "verify",
        "--repo-root",
        str(repo_root),
        "--candidate-sha",
        candidate_sha,
        "--gate-result",
        str(gate_result),
    ]


def verify_release_gate_result(
    repo_root: Path,
    candidate_sha: str,
    gate_result: Path,
) -> int:
    """Verify an authoritative remote Go result for the clean release HEAD."""
    print_header("Authoritative Release Gate Evidence")
    return run_command(
        release_gate_verification_command(repo_root, candidate_sha, gate_result),
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


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Publish dear-imgui-rs workspace crates in dependency order",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        "--crates",
        help="Comma-separated list of crates to publish (default: all)"
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
        "--release-gate-result",
        type=Path,
        help=(
            "Authoritative same-SHA gate-result.json; required for crates.io uploads"
        ),
    )
    parser.add_argument(
        "--dangerously-skip-release-check",
        action="store_true",
        help=(
            "Emergency-only upload without authoritative release evidence or the "
            "strict local release gate"
        ),
    )
    parser.add_argument(
        "--wait",
        type=int,
        default=30,
        help="Seconds to wait between publishes for crates.io indexing (default: 30)"
    )
    parser.add_argument(
        "--start-from",
        help="Start publishing from this crate (useful for resuming)"
    )
    
    args = parser.parse_args()

    if args.dry_run and args.cargo_dry_run:
        print_error("--dry-run and --cargo-dry-run are mutually exclusive")
        return 1

    actual_upload = not args.dry_run and not args.cargo_dry_run
    if (
        actual_upload
        and not args.dangerously_skip_release_check
        and args.release_gate_result is None
    ):
        print_error(
            "Actual crates.io uploads require --release-gate-result PATH from the "
            "authoritative remote release gate. Use "
            "--dangerously-skip-release-check only for an explicit emergency bypass."
        )
        return 1
    
    # Get repository root
    repo_root = Path(__file__).resolve().parents[1]

    # Determine which crates to publish
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
    
    # Handle start-from
    if args.start_from:
        found = False
        filtered = []
        for name, path in crates_to_publish:
            if name == args.start_from:
                found = True
            if found:
                filtered.append((name, path))
        if not found:
            print_error(f"Start crate not found: {args.start_from}")
            return 1
        crates_to_publish = filtered

    release_head: Optional[str] = None
    if not args.dry_run:
        release_head = capture_release_fingerprint(repo_root)
        if release_head is None:
            return 1
        if args.dangerously_skip_release_check:
            print_warning(
                "Authoritative release evidence and strict local preflight were "
                "explicitly bypassed"
            )
        else:
            if actual_upload:
                gate_result = args.release_gate_result
                assert gate_result is not None
                gate_verification = verify_release_gate_result(
                    repo_root,
                    release_head,
                    gate_result,
                )
                if gate_verification != 0:
                    print_error(
                        "Authoritative release gate verification failed; no crates "
                        "were uploaded"
                    )
                    return gate_verification
                if not verify_release_fingerprint(repo_root, release_head):
                    print_error(
                        "Release source changed during gate verification; no crates "
                        "were uploaded"
                    )
                    return 1
            preflight = run_release_preflight(repo_root)
            if preflight != 0:
                print_error("Strict release preflight failed; no crates were uploaded")
                return preflight
            if not verify_release_fingerprint(repo_root, release_head):
                print_error("Release source changed during preflight; no crates were uploaded")
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

    if not actual_upload:
        gate_status = "not required for preview"
    elif args.dangerously_skip_release_check:
        gate_status = "DANGEROUSLY SKIPPED"
    else:
        gate_status = f"verified from {args.release_gate_result}"

    if args.dry_run:
        preflight_status = "not run for print-only dry-run"
    elif args.dangerously_skip_release_check:
        preflight_status = "DANGEROUSLY SKIPPED"
    else:
        preflight_status = "passed for this clean HEAD"
    
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
    print_info(f"No verify: {args.no_verify}")
    print_info(f"Authoritative release gate: {gate_status}")
    print_info(f"Strict local release preflight: {preflight_status}")
    print_info(f"Wait time: {args.wait}s")
    print()
    print("Publishing order:")
    for i, (name, path) in enumerate(crates_to_publish, 1):
        print(f"  {i}. {name} ({path})")
    print()
    
    if not args.dry_run and not args.cargo_dry_run:
        response = input("Continue with publishing? [y/N]: ").strip().lower()
        if response not in ('y', 'yes'):
            print_info("Publishing cancelled")
            return 0
    # Publish each crate
    failed_crates = []
    for name, path in crates_to_publish:
        if release_head is not None and not verify_release_fingerprint(
            repo_root, release_head
        ):
            print_error("Release source changed before publish; stopping immediately")
            return 1
        success = publish_crate(
            name,
            Path(path),
            package_versions[name],
            repo_root,
            dry_run=args.dry_run,
            cargo_dry_run=args.cargo_dry_run,
            no_verify=args.no_verify,
            wait_time=args.wait,
            source_guard=(
                (lambda: verify_release_fingerprint(repo_root, release_head))
                if release_head is not None
                else None
            ),
        )
        
        if not success:
            if release_head is not None and not verify_release_fingerprint(
                repo_root, release_head
            ):
                print_error("Release source changed; refusing to continue publishing")
                return 1
            failed_crates.append(name)
            print_error(f"Failed to publish {name}")
            if args.dry_run or args.cargo_dry_run:
                break
            response = input("Continue with remaining crates? [y/N]: ").strip().lower()
            if response not in ('y', 'yes'):
                break
    
    # Print final summary
    print_header("Publishing Complete")
    
    if failed_crates:
        print_error(f"Failed to publish {len(failed_crates)} crate(s):")
        for name in failed_crates:
            print(f"  - {name}")
        return 1
    else:
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
                f"Successfully uploaded all {len(crates_to_publish)} crate(s)."
            )
        return 0


if __name__ == "__main__":
    sys.exit(main())
