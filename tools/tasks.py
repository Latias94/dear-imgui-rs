#!/usr/bin/env python3
"""
Task runner for dear-imgui-rs workspace.

This script provides convenient shortcuts for common development and release tasks.

Usage:
  python3 tools/tasks.py <task> [options]

Available tasks:
  check           - Run pre-publish validation checks
  bump <version>  - Bump version to specified version
  bindings        - Update pregenerated bindings for all -sys crates
  publish         - Publish all crates to crates.io
  test            - Run all tests
  doc             - Build documentation
  clean           - Clean build artifacts

Examples:
  python3 tools/tasks.py check
  python3 tools/tasks.py bump 0.7.1
  python3 tools/tasks.py bindings
  python3 tools/tasks.py publish --dry-run
"""

import argparse
import subprocess
import sys
from pathlib import Path
from typing import List


def run_command(cmd: List[str], cwd=None, quiet: bool = False) -> int:
    """
    Run a command and return its exit code.

    Args:
        cmd: Command to run
        cwd: Working directory
        quiet: If True, suppress the command echo
    """
    if not quiet:
        print(f"$ {' '.join(str(c) for c in cmd)}")
    try:
        # Always stream output in real-time (don't capture)
        result = subprocess.run(cmd, cwd=cwd, check=False)
        return result.returncode
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def task_check(args, repo_root: Path) -> int:
    """Run pre-publish validation checks."""
    cmd = [sys.executable, "tools/pre_publish_check.py"]
    
    if args.skip_git:
        cmd.append("--skip-git-check")
    if args.skip_doc:
        cmd.append("--skip-doc-check")
    if args.skip_test:
        cmd.append("--skip-test-check")
    
    return run_command(cmd, cwd=repo_root)


def task_bump(args, repo_root: Path) -> int:
    """Bump version across workspace."""
    if not args.version:
        print("Error: version argument required", file=sys.stderr)
        print("Usage: python3 tools/tasks.py bump <version>")
        return 1
    
    cmd = [sys.executable, "tools/bump_version.py", args.version]
    
    if args.dry_run:
        cmd.append("--dry-run")
    if args.old_version:
        cmd.extend(["--old-version", args.old_version])
    
    return run_command(cmd, cwd=repo_root)


def task_bindings(args, repo_root: Path) -> int:
    """Update pregenerated bindings."""
    cmd = [sys.executable, "tools/update_submodule_and_bindings.py"]
    
    if args.crates:
        cmd.extend(["--crates", args.crates])
    else:
        cmd.extend(["--crates", "all"])
    
    cmd.extend(["--profile", "release"])
    
    if args.update_submodules:
        cmd.extend(["--submodules", "update"])
    else:
        cmd.extend(["--submodules", "skip"])
    
    if args.dry_run:
        cmd.append("--dry-run")
    
    return run_command(cmd, cwd=repo_root)


def task_publish(args, repo_root: Path) -> int:
    """Publish crates to crates.io."""
    cmd = [sys.executable, "tools/publish.py"]
    
    if args.dry_run:
        cmd.append("--dry-run")
    if args.no_verify:
        cmd.append("--no-verify")
    if args.crates:
        cmd.extend(["--crates", args.crates])
    if args.start_from:
        cmd.extend(["--start-from", args.start_from])
    if args.wait:
        cmd.extend(["--wait", str(args.wait)])
    
    return run_command(cmd, cwd=repo_root)


def task_test(args, repo_root: Path) -> int:
    """Run tests."""
    cmd = ["cargo", "test", "--workspace"]
    
    if args.lib_only:
        cmd.append("--lib")
    if args.package:
        cmd.extend(["-p", args.package])
    
    return run_command(cmd, cwd=repo_root)


def task_doc(args, repo_root: Path) -> int:
    """Build documentation."""
    cmd = ["cargo", "doc", "--workspace"]
    
    if args.no_deps:
        cmd.append("--no-deps")
    if args.open:
        cmd.append("--open")
    if args.package:
        cmd.extend(["-p", args.package])
    
    return run_command(cmd, cwd=repo_root)


def task_clean(args, repo_root: Path) -> int:
    """Clean build artifacts."""
    cmd = ["cargo", "clean"]
    
    if args.package:
        cmd.extend(["-p", args.package])
    
    return run_command(cmd, cwd=repo_root)


def task_release_prep(args, repo_root: Path) -> int:
    """Prepare for release (all-in-one)."""
    if not args.version:
        print("Error: version argument required", file=sys.stderr)
        print("Usage: python3 tools/tasks.py release-prep <version>")
        return 1
    
    print("\n" + "=" * 80)
    print("RELEASE PREPARATION WORKFLOW")
    print("=" * 80 + "\n")
    
    steps = [
        ("1. Bump version", lambda: task_bump(args, repo_root)),
        ("2. Update bindings", lambda: task_bindings(args, repo_root)),
        ("3. Run tests", lambda: task_test(args, repo_root)),
        ("4. Run checks", lambda: task_check(args, repo_root)),
    ]
    
    for step_name, step_func in steps:
        print(f"\n{'=' * 80}")
        print(f"Step: {step_name}")
        print("=" * 80 + "\n")
        
        result = step_func()
        if result != 0:
            print(f"\nError: {step_name} failed", file=sys.stderr)
            return result
    
    print("\n" + "=" * 80)
    print("RELEASE PREPARATION COMPLETE")
    print("=" * 80 + "\n")
    print("Next steps:")
    print("  1. Review changes: git diff")
    print("  2. Update CHANGELOG.md")
    print("  3. Update README.md and docs/COMPATIBILITY.md")
    print("  4. Commit: git add -A && git commit -m 'chore: prepare release v" + args.version + "'")
    print("  5. Publish: python3 tools/tasks.py publish")
    print()
    
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Task runner for dear-imgui-rs workspace",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    
    subparsers = parser.add_subparsers(dest="task", help="Task to run")
    
    # check task
    check_parser = subparsers.add_parser("check", help="Run pre-publish validation")
    check_parser.add_argument("--skip-git", action="store_true", help="Skip git checks")
    check_parser.add_argument("--skip-doc", action="store_true", help="Skip doc checks")
    check_parser.add_argument("--skip-test", action="store_true", help="Skip test checks")
    
    # bump task
    bump_parser = subparsers.add_parser("bump", help="Bump version")
    bump_parser.add_argument("version", nargs="?", help="New version (e.g., 0.5.0)")
    bump_parser.add_argument("--old-version", help="Old version to replace")
    bump_parser.add_argument("--dry-run", action="store_true", help="Dry run")
    
    # bindings task
    bindings_parser = subparsers.add_parser("bindings", help="Update pregenerated bindings")
    bindings_parser.add_argument("--crates", help="Comma-separated list of crates")
    bindings_parser.add_argument("--update-submodules", action="store_true", help="Update submodules")
    bindings_parser.add_argument("--dry-run", action="store_true", help="Dry run")
    
    # publish task
    publish_parser = subparsers.add_parser("publish", help="Publish crates")
    publish_parser.add_argument("--crates", help="Comma-separated list of crates")
    publish_parser.add_argument("--start-from", help="Start from specific crate")
    publish_parser.add_argument("--wait", type=int, help="Wait time between publishes")
    publish_parser.add_argument("--dry-run", action="store_true", help="Dry run")
    publish_parser.add_argument("--no-verify", action="store_true", help="Skip verification")
    
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
    
    # release-prep task (all-in-one)
    release_prep_parser = subparsers.add_parser("release-prep", help="Prepare for release (all-in-one)")
    release_prep_parser.add_argument("version", nargs="?", help="New version (e.g., 0.5.0)")
    release_prep_parser.add_argument("--old-version", help="Old version to replace")
    
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
        "release-prep": task_release_prep,
    }
    
    task_func = tasks.get(args.task)
    if not task_func:
        print(f"Unknown task: {args.task}", file=sys.stderr)
        return 1
    
    return task_func(args, repo_root)


if __name__ == "__main__":
    sys.exit(main())
