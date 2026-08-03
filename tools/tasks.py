#!/usr/bin/env python3
"""
Task runner for dear-imgui-rs workspace.

This script provides convenient shortcuts for common development and release tasks.

Usage:
  python3 tools/tasks.py <task> [options]

Available tasks:
  check                     - Run configurable pre-publish checks
  bump <version>            - Update the unified workspace release version
  bindings                  - Update all core and extension bindings
  release-prepare <version> - Generate a release diff from a clean tree
  release-check             - Validate a committed clean release candidate
  publish                   - Publish all crates to crates.io
  test                      - Run all tests
  doc                       - Build documentation
  clean                     - Clean build artifacts

Examples:
  python3 tools/tasks.py check
  python3 tools/tasks.py bump 0.16.0-alpha.1 --allow-prerelease-relabel --dry-run
  python3 tools/tasks.py bindings
  python3 tools/tasks.py release-prepare 0.16.0-alpha.1 --allow-prerelease-relabel
  python3 tools/tasks.py release-check
  python3 tools/tasks.py publish --dry-run
"""

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Callable, List


STANDALONE_LOCKED_MANIFESTS = (
    Path("examples-android/dear-imgui-android-smoke/Cargo.toml"),
    Path("examples-ios/dear-imgui-ios-smoke/Cargo.toml"),
    Path("examples-ios/dear-imgui-ios-sdl3-smoke/Cargo.toml"),
)


def run_command(
    cmd: List[str], cwd=None, quiet: bool = False, capture: bool = False
) -> int:
    """
    Run a command and return its exit code.

    Args:
        cmd: Command to run
        cwd: Working directory
        quiet: If True, suppress the command echo
        capture: If True, suppress successful output and show it only on failure
    """
    if not quiet:
        print(f"$ {' '.join(str(c) for c in cmd)}")
    try:
        result = subprocess.run(
            cmd,
            cwd=cwd,
            check=False,
            capture_output=capture,
            text=capture,
        )
        if capture and result.returncode != 0:
            if result.stdout:
                print(result.stdout, file=sys.stderr)
            if result.stderr:
                print(result.stderr, file=sys.stderr)
        return result.returncode
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cargo_nextest_available(cwd=None) -> bool:
    """Return True if cargo-nextest is installed."""
    try:
        result = subprocess.run(
            ["cargo", "nextest", "--version"],
            cwd=cwd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        return result.returncode == 0
    except Exception:
        return False


def task_check(args, repo_root: Path) -> int:
    """Run configurable pre-publish validation checks."""
    cmd = [sys.executable, "tools/pre_publish_check.py"]

    if getattr(args, "skip_git", False) and not getattr(
        args, "skip_package", False
    ):
        print(
            "Error: --skip-git requires --skip-package because package checks "
            "validate a clean clone of HEAD",
            file=sys.stderr,
        )
        return 1
    if getattr(args, "skip_git", False):
        cmd.append("--skip-git-check")
    if getattr(args, "skip_doc", False):
        cmd.append("--skip-doc-check")
    if getattr(args, "skip_test", False):
        cmd.append("--skip-test-check")
    if getattr(args, "skip_package", False):
        cmd.append("--skip-package-check")

    return run_command(cmd, cwd=repo_root)


def task_bump(args, repo_root: Path) -> int:
    """Update the unified workspace release version."""
    cmd = [
        "cargo",
        "run",
        "--locked",
        "-p",
        "xtask",
        "--",
        "release-version",
        args.version,
    ]

    if getattr(args, "dry_run", False):
        cmd.append("--dry-run")
    if getattr(args, "allow_prerelease_relabel", False):
        cmd.append("--allow-prerelease-relabel")
    return run_command(cmd, cwd=repo_root)


def task_bindings(args, repo_root: Path) -> int:
    """Update every maintained binding profile through the canonical xtask."""
    crates = getattr(args, "crates", None) or "all"
    selected = {crate.strip() for crate in crates.split(",") if crate.strip()}
    includes_core = crates.strip().lower() == "all" or "dear-imgui-sys" in selected

    cmd = [sys.executable, "tools/update_submodule_and_bindings.py"]
    cmd.extend(["--crates", crates])
    cmd.extend(["--profile", "release"])
    if getattr(args, "update_submodules", False):
        cmd.extend(["--submodules", "update"])
    else:
        cmd.extend(["--submodules", "skip"])
    if includes_core:
        cmd.append("--skip-core-bindings")
    if getattr(args, "dry_run", False):
        cmd.append("--dry-run")

    rc = run_command(cmd, cwd=repo_root)
    if rc != 0 or not includes_core:
        return rc

    core_command = [
        "cargo",
        "run",
        "-p",
        "xtask",
        "--",
        "verify-bindings",
        "--update",
        "--allow-dirty",
    ]
    if getattr(args, "dry_run", False):
        print(f"$ {' '.join(core_command)}")
        return 0

    return run_command(core_command, cwd=repo_root)


def task_publish(args, repo_root: Path) -> int:
    """Publish crates to crates.io with authoritative release evidence."""
    cmd = [sys.executable, "tools/publish.py"]
    
    if getattr(args, "dry_run", False):
        cmd.append("--dry-run")
    if getattr(args, "cargo_dry_run", False):
        cmd.append("--cargo-dry-run")
    if getattr(args, "no_verify", False):
        cmd.append("--no-verify")
    if getattr(args, "crates", None):
        cmd.extend(["--crates", args.crates])
    if getattr(args, "release_gate_result", None):
        cmd.extend(["--release-gate-result", str(args.release_gate_result)])
    if getattr(args, "yes", False):
        cmd.append("--yes")
    if getattr(args, "verify_published", False):
        cmd.append("--verify-published")
    if getattr(args, "index_timeout", None) is not None:
        cmd.extend(["--index-timeout", str(args.index_timeout)])
    if getattr(args, "publish_timeout", None) is not None:
        cmd.extend(["--publish-timeout", str(args.publish_timeout)])
    if getattr(args, "poll_interval", None) is not None:
        cmd.extend(["--poll-interval", str(args.poll_interval)])
    if getattr(args, "journal", None):
        cmd.extend(["--journal", str(args.journal)])
    
    return run_command(cmd, cwd=repo_root)


def task_test(args, repo_root: Path) -> int:
    """Run tests."""
    # See notes in tools/pre_publish_check.py: with test-engine present, feature unification can enable
    # ImGui test engine hooks in `dear-imgui-sys`, and a full workspace test run may try to link crates
    # that don't depend on the test engine library.
    #
    # Prefer nextest when available. It better isolates tests that create Dear ImGui contexts; the
    # cargo-test fallback uses one test thread to avoid racing the process-global C++ context.
    #
    # Default behaviour: run tests in two passes.
    # - Pass 1: all workspace crates excluding test-engine crates.
    # - Pass 2: test-engine crate itself.
    # - Pass 3: dear-imgui-rs with multi-viewport enabled for PlatformIO callbacks.
    #
    # If `--package` is provided, run a normal single-package test.
    use_nextest = cargo_nextest_available(repo_root)
    runner = ["cargo", "nextest", "run", "--no-tests", "pass"] if use_nextest else ["cargo", "test"]
    serial_args = [] if use_nextest else ["--", "--test-threads=1"]

    if getattr(args, "package", None):
        cmd = runner + ["--workspace", "-p", args.package]
        if getattr(args, "lib_only", False):
            cmd.append("--lib")
        cmd += serial_args
        return run_command(cmd, cwd=repo_root)

    cmd = runner + ["--workspace"]
    
    if getattr(args, "lib_only", False):
        cmd.append("--lib")

    pass1 = cmd + ["--exclude", "dear-imgui-test-engine", "--exclude", "dear-imgui-test-engine-sys"]
    pass1 += serial_args
    rc = run_command(pass1, cwd=repo_root)
    if rc != 0:
        return rc

    pass2 = runner + ["-p", "dear-imgui-test-engine"]
    if getattr(args, "lib_only", False):
        pass2.append("--lib")
    pass2 += serial_args
    rc = run_command(pass2, cwd=repo_root)
    if rc != 0:
        return rc

    pass3 = runner + ["-p", "dear-imgui-rs", "--features", "multi-viewport"]
    if getattr(args, "lib_only", False):
        pass3.append("--lib")
    pass3 += serial_args
    return run_command(pass3, cwd=repo_root)


def task_doc(args, repo_root: Path) -> int:
    """Build documentation."""
    cmd = ["cargo", "doc", "--workspace"]
    
    if getattr(args, "no_deps", False):
        cmd.append("--no-deps")
    if getattr(args, "open", False):
        cmd.append("--open")
    if getattr(args, "package", None):
        cmd.extend(["-p", args.package])
    
    return run_command(cmd, cwd=repo_root)


def task_clean(args, repo_root: Path) -> int:
    """Clean build artifacts."""
    cmd = ["cargo", "clean"]
    
    if getattr(args, "package", None):
        cmd.extend(["-p", args.package])
    
    return run_command(cmd, cwd=repo_root)


def require_clean_worktree(repo_root: Path) -> int:
    """Require all tracked, untracked, and submodule state to be clean."""
    command = [
        "git",
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "--ignore-submodules=none",
    ]
    print(f"$ {' '.join(command)}")
    try:
        result = subprocess.run(
            command,
            cwd=repo_root,
            check=False,
            capture_output=True,
            text=True,
        )
    except Exception as error:
        print(f"Error: could not inspect worktree: {error}", file=sys.stderr)
        return 1

    if result.returncode != 0:
        detail = result.stderr.strip() or "git status failed"
        print(f"Error: {detail}", file=sys.stderr)
        return result.returncode
    if result.stdout.strip():
        print(
            "Error: release-prepare requires a completely clean worktree:",
            file=sys.stderr,
        )
        print(result.stdout.rstrip(), file=sys.stderr)
        return 1
    return 0


def refresh_cargo_lock(repo_root: Path, dry_run: bool) -> int:
    """Refresh every maintained lockfile, then prove each graph resolves locked."""
    routes = ((None, True),) + tuple(
        (manifest, False) for manifest in STANDALONE_LOCKED_MANIFESTS
    )
    for manifest, no_deps in routes:
        manifest_args = (
            [] if manifest is None else ["--manifest-path", manifest.as_posix()]
        )
        dependency_args = ["--no-deps"] if no_deps else []
        refresh = [
            "cargo",
            "metadata",
            *manifest_args,
            *dependency_args,
            "--format-version",
            "1",
        ]
        verify = [
            "cargo",
            "metadata",
            *manifest_args,
            "--locked",
            *dependency_args,
            "--format-version",
            "1",
        ]

        if dry_run:
            print(f"$ {' '.join(refresh)}  # skipped by --dry-run")
        else:
            result = run_command(refresh, cwd=repo_root, capture=True)
            if result != 0:
                return result
        result = run_command(verify, cwd=repo_root, capture=True)
        if result != 0:
            return result
    return 0


def run_release_step(label: str, operation: Callable[[], int]) -> int:
    """Run one release preparation step and stop on its first failure."""
    print(f"\n{'=' * 80}")
    print(f"Step: {label}")
    print("=" * 80 + "\n")
    result = operation()
    if result != 0:
        print(f"\nError: {label} failed", file=sys.stderr)
    return result


def task_release_prepare(args, repo_root: Path) -> int:
    """Generate a release diff without invoking strict committed-tree checks."""
    print("\n" + "=" * 80)
    print("RELEASE PREPARATION WORKFLOW")
    print("=" * 80 + "\n")

    if require_clean_worktree(repo_root) != 0:
        return 1

    steps = [
        ("1. Update unified release version", lambda: task_bump(args, repo_root)),
        (
            "2. Refresh and verify Cargo.lock",
            lambda: refresh_cargo_lock(repo_root, args.dry_run),
        ),
        (
            "3. Regenerate bindings without updating submodules",
            lambda: task_bindings(args, repo_root),
        ),
    ]
    if not args.skip_tool_tests:
        steps.append(
            (
                "4. Run release-tool unit tests",
                lambda: run_command(
                    [
                        sys.executable,
                        "-B",
                        "-m",
                        "unittest",
                        "discover",
                        "-s",
                        "tools/tests",
                        "-p",
                        "test_*.py",
                    ],
                    cwd=repo_root,
                ),
            )
        )

    for label, operation in steps:
        result = run_release_step(label, operation)
        if result != 0:
            return result

    print("\n" + "=" * 80)
    print("RELEASE PREPARATION COMPLETE")
    print("=" * 80 + "\n")
    if args.dry_run:
        print("Dry run complete; no release files were intentionally modified.")
    print("Next steps:")
    print("  1. Review the generated diff: git diff")
    print("  2. Update CHANGELOG.md, README.md, and release documentation")
    print(
        "  3. Commit the release candidate: "
        f"git commit -m 'chore: prepare release v{args.version}'"
    )
    print("  4. From the committed clean tree: python3 tools/tasks.py release-check")
    print()
    return 0


def task_release_check(_args, repo_root: Path) -> int:
    """Run the strict release gate against a committed clean tree."""
    return run_command(
        [sys.executable, "tools/pre_publish_check.py"],
        cwd=repo_root,
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Task runner for dear-imgui-rs workspace",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    
    subparsers = parser.add_subparsers(dest="task", help="Task to run")
    
    # check task
    check_parser = subparsers.add_parser(
        "check", help="Run configurable pre-publish validation"
    )
    check_parser.add_argument(
        "--skip-git",
        action="store_true",
        help="Skip git checks (requires --skip-package)",
    )
    check_parser.add_argument("--skip-doc", action="store_true", help="Skip doc checks")
    check_parser.add_argument("--skip-test", action="store_true", help="Skip test checks")
    check_parser.add_argument(
        "--skip-package",
        action="store_true",
        help="Skip clean-clone package and offline-consumer checks",
    )
    
    # bump task
    bump_parser = subparsers.add_parser(
        "bump", help="Update the unified workspace release version"
    )
    bump_parser.add_argument(
        "version", help="New complete version (e.g., 0.16.0-alpha.1)"
    )
    bump_parser.add_argument(
        "--dry-run", action="store_true", help="Validate and preview without writing"
    )
    bump_parser.add_argument(
        "--allow-prerelease-relabel",
        action="store_true",
        help="Allow only a stable version to be relabeled as a same-version prerelease",
    )
    
    # bindings task
    bindings_parser = subparsers.add_parser(
        "bindings",
        help="Update Windows, non-Windows, WASM, and extension bindings",
    )
    bindings_parser.add_argument("--crates", help="Comma-separated list of crates")
    bindings_parser.add_argument("--update-submodules", action="store_true", help="Update submodules")
    bindings_parser.add_argument("--dry-run", action="store_true", help="Dry run")
    
    # publish task
    publish_parser = subparsers.add_parser(
        "publish", help="Publish crates with authoritative release evidence"
    )
    publish_parser.add_argument("--crates", help="Comma-separated list of crates")
    publish_parser.add_argument("--dry-run", action="store_true", help="Dry run")
    publish_parser.add_argument(
        "--cargo-dry-run",
        action="store_true",
        help="Run cargo publish --dry-run",
    )
    publish_parser.add_argument("--no-verify", action="store_true", help="Skip verification")
    publish_parser.add_argument(
        "--release-gate-result",
        type=Path,
        help="Authoritative same-SHA gate-result.json required for uploads",
    )
    publish_parser.add_argument("--yes", action="store_true", help="Confirm upload")
    publish_parser.add_argument(
        "--verify-published",
        action="store_true",
        help="Verify the complete release train without uploading",
    )
    publish_parser.add_argument(
        "--index-timeout",
        type=float,
        help="Maximum seconds to wait for each exact version",
    )
    publish_parser.add_argument(
        "--publish-timeout",
        type=float,
        help="Maximum seconds for one cargo publish process",
    )
    publish_parser.add_argument(
        "--poll-interval",
        type=float,
        help="Initial registry polling interval in seconds",
    )
    publish_parser.add_argument(
        "--journal",
        type=Path,
        help="Write an atomic publication journal",
    )
    
    # test task
    test_parser = subparsers.add_parser("test", help="Run tests")
    test_parser.add_argument("--lib-only", action="store_true", help="Test only libraries")
    test_parser.add_argument("-p", "--package", help="Test specific package")
    
    # doc task
    doc_parser = subparsers.add_parser("doc", help="Build documentation")
    doc_parser.add_argument("--no-deps", action="store_true", help="Don't build dependencies")
    doc_parser.add_argument("--open", action="store_true", help="Open in browser")
    doc_parser.add_argument("-p", "--package", help="Document specific package")
    
    # clean task
    clean_parser = subparsers.add_parser("clean", help="Clean build artifacts")
    clean_parser.add_argument("-p", "--package", help="Clean specific package")
    
    release_prepare_parser = subparsers.add_parser(
        "release-prepare",
        help="Generate a release diff from a completely clean worktree",
    )
    release_prepare_parser.add_argument(
        "version", help="New complete release version (e.g., 0.16.0-alpha.1)"
    )
    release_prepare_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate and display mutating steps without writing release files",
    )
    release_prepare_parser.add_argument(
        "--allow-prerelease-relabel",
        action="store_true",
        help="Allow only a stable version to be relabeled as a same-version prerelease",
    )
    release_prepare_parser.add_argument(
        "--skip-tool-tests",
        action="store_true",
        help="Skip the focused Python release-tool unit tests",
    )

    subparsers.add_parser(
        "release-check",
        help="Run every strict release gate against a committed clean tree",
    )
    
    args = parser.parse_args()
    
    if not args.task:
        parser.print_help()
        return 1
    
    repo_root = Path(__file__).resolve().parents[1]
    
    tasks = {
        "check": task_check,
        "bump": task_bump,
        "bindings": task_bindings,
        "publish": task_publish,
        "test": task_test,
        "doc": task_doc,
        "clean": task_clean,
        "release-prepare": task_release_prepare,
        "release-check": task_release_check,
    }
    
    task_func = tasks.get(args.task)
    if not task_func:
        print(f"Unknown task: {args.task}", file=sys.stderr)
        return 1
    
    return task_func(args, repo_root)


if __name__ == "__main__":
    sys.exit(main())
