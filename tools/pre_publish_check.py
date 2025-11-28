#!/usr/bin/env python3
"""
Pre-publish validation script for dear-imgui-rs workspace.

This script performs various checks to ensure the workspace is ready for publishing:
- Version consistency across all crates
- Pregenerated bindings exist for -sys crates
- Git working tree is clean
- Cargo.lock is up-to-date
- Documentation builds successfully

Usage:
  python tools/pre_publish_check.py

  # Skip specific checks
  python tools/pre_publish_check.py --skip-git-check --skip-doc-check

Requirements:
  - Python 3.7+
  - cargo, git in PATH
"""

import argparse
import subprocess
import sys
from pathlib import Path
from typing import List, Tuple, Optional, Dict


# Crates that should have pregenerated bindings
SYS_CRATES = [
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
]

# All publishable crates
ALL_CRATES = [
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-imgui-rs", "dear-imgui"),
    ("dear-imgui-winit", "backends/dear-imgui-winit"),
    ("dear-imgui-wgpu", "backends/dear-imgui-wgpu"),
    ("dear-imgui-glow", "backends/dear-imgui-glow"),
    ("dear-imgui-sdl3", "backends/dear-imgui-sdl3"),
    ("dear-app", "dear-app"),
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-implot", "extensions/dear-implot"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-imnodes", "extensions/dear-imnodes"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-imguizmo", "extensions/dear-imguizmo"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-implot3d", "extensions/dear-implot3d"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
    ("dear-imguizmo-quat", "extensions/dear-imguizmo-quat"),
    ("dear-file-browser", "extensions/dear-file-browser"),
]


class Colors:
    """ANSI color codes."""
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'


def print_header(msg: str):
    print(f"\n{Colors.HEADER}{Colors.BOLD}{'=' * 80}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{msg}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{'=' * 80}{Colors.ENDC}\n")


def print_check(msg: str):
    print(f"{Colors.OKBLUE}🔍 Checking: {msg}{Colors.ENDC}")


def print_success(msg: str):
    print(f"{Colors.OKGREEN}✓ {msg}{Colors.ENDC}")


def print_warning(msg: str):
    print(f"{Colors.WARNING}⚠ {msg}{Colors.ENDC}")


def print_error(msg: str):
    print(f"{Colors.FAIL}✗ {msg}{Colors.ENDC}")


def run_command(cmd: List[str], cwd: Optional[Path] = None, capture: bool = True, show_output: bool = False) -> Tuple[int, str, str]:
    """
    Run a command and return (exit_code, stdout, stderr).

    Args:
        cmd: Command to run
        cwd: Working directory
        capture: If True, capture output; if False, stream to console
        show_output: If True and capture=True, also print captured output
    """
    try:
        if capture:
            result = subprocess.run(
                cmd,
                cwd=cwd,
                capture_output=True,
                text=True,
                check=False
            )
            if show_output:
                if result.stdout:
                    print(result.stdout)
                if result.stderr:
                    print(result.stderr, file=sys.stderr)
            return result.returncode, result.stdout, result.stderr
        else:
            # Stream output in real-time
            result = subprocess.run(cmd, cwd=cwd, check=False)
            return result.returncode, "", ""
    except Exception as e:
        return 1, "", str(e)


def get_crate_version(crate_path: Path) -> Optional[str]:
    """Extract version from Cargo.toml."""
    cargo_toml = crate_path / "Cargo.toml"
    if not cargo_toml.exists():
        return None
    
    try:
        with open(cargo_toml, 'r', encoding='utf-8') as f:
            for line in f:
                if line.strip().startswith('version'):
                    parts = line.split('=')
                    if len(parts) == 2:
                        version = parts[1].strip().strip('"').strip("'")
                        if not version.startswith('{'):
                            return version
    except Exception:
        pass
    
    return None


def check_version_consistency(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that all crates have consistent versions."""
    print_check("Version consistency across crates")
    
    versions: Dict[str, str] = {}
    errors = []
    
    for name, path in ALL_CRATES:
        full_path = repo_root / path
        version = get_crate_version(full_path)
        
        if version is None:
            errors.append(f"Could not read version for {name}")
        else:
            versions[name] = version
    
    if errors:
        for error in errors:
            print_error(error)
        return False, errors
    
    # Check if all versions are the same
    unique_versions = set(versions.values())
    
    if len(unique_versions) == 1:
        version = list(unique_versions)[0]
        print_success(f"All crates use version {version}")
        return True, []
    else:
        errors.append("Version mismatch detected:")
        for name, version in sorted(versions.items()):
            errors.append(f"  {name}: {version}")
        for error in errors:
            print_error(error)
        return False, errors


def check_pregenerated_bindings(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that all -sys crates have pregenerated bindings."""
    print_check("Pregenerated bindings for -sys crates")
    
    errors = []
    
    for name, path in SYS_CRATES:
        full_path = repo_root / path / "src" / "bindings_pregenerated.rs"
        
        if not full_path.exists():
            errors.append(f"Missing pregenerated bindings: {name}")
            print_error(f"Missing: {full_path}")
        else:
            # Check file is not empty
            size = full_path.stat().st_size
            if size < 1000:  # Bindings should be at least 1KB
                errors.append(f"Pregenerated bindings too small: {name} ({size} bytes)")
                print_error(f"Too small: {full_path} ({size} bytes)")
            else:
                print_success(f"{name}: {size:,} bytes")
    
    if not errors:
        print_success("All -sys crates have pregenerated bindings")
        return True, []
    else:
        print_error("Run: python tools/update_submodule_and_bindings.py --crates all --profile release")
        return False, errors


def check_git_status(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that git working tree is clean."""
    print_check("Git working tree status")
    
    code, stdout, stderr = run_command(["git", "status", "--porcelain"], cwd=repo_root)
    
    if code != 0:
        print_error(f"Git command failed: {stderr}")
        return False, ["Git command failed"]
    
    if stdout.strip():
        print_warning("Working tree has uncommitted changes:")
        print(stdout)
        return False, ["Uncommitted changes in working tree"]
    else:
        print_success("Working tree is clean")
        return True, []


def check_cargo_lock(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that Cargo.lock is up-to-date."""
    print_check("Cargo.lock is up-to-date")
    
    # Run cargo update --dry-run to check if lock file needs updating
    code, stdout, stderr = run_command(
        ["cargo", "update", "--workspace", "--dry-run"],
        cwd=repo_root
    )
    
    if code != 0:
        print_error(f"Cargo update check failed: {stderr}")
        return False, ["Cargo update check failed"]
    
    # Check if any updates are available
    if "Updating" in stdout or "Adding" in stdout:
        print_warning("Cargo.lock may need updating:")
        print(stdout)
        print_warning("Run: cargo update")
        return False, ["Cargo.lock may be outdated"]
    else:
        print_success("Cargo.lock is up-to-date")
        return True, []


def check_docs_build(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that documentation builds for -sys crates in offline mode."""
    print_check("Documentation builds (offline mode for -sys crates)")

    errors = []

    import os
    for name, path in SYS_CRATES:
        print(f"\n  Checking {name}...")

        # Set DOCS_RS=1 to simulate docs.rs environment
        env = os.environ.copy()
        env["DOCS_RS"] = "1"

        # Run cargo check with the modified environment
        try:
            result = subprocess.run(
                ["cargo", "check", "-p", name],
                cwd=repo_root,
                env=env,
                check=False
            )
            code = result.returncode
        except Exception as e:
            print_error(f"Failed to run cargo check: {e}")
            code = 1

        if code != 0:
            errors.append(f"Doc build failed for {name}")
            print_error(f"Failed: {name}")
        else:
            print_success(f"OK: {name}")

    if not errors:
        print_success("\nAll -sys crates build in offline mode")
        return True, []
    else:
        return False, errors


def check_tests(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that tests pass."""
    print_check("Running tests")

    # Stream test output in real-time
    code, stdout, stderr = run_command(
        ["cargo", "test", "--workspace", "--lib"],
        cwd=repo_root,
        capture=False  # Stream output in real-time
    )

    if code != 0:
        print_error("Tests failed")
        return False, ["Tests failed"]
    else:
        print_success("All tests passed")
        return True, []


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Pre-publish validation for dear-imgui-rs workspace",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        "--skip-git-check",
        action="store_true",
        help="Skip git working tree check"
    )
    parser.add_argument(
        "--skip-doc-check",
        action="store_true",
        help="Skip documentation build check"
    )
    parser.add_argument(
        "--skip-test-check",
        action="store_true",
        help="Skip test execution check"
    )
    
    args = parser.parse_args()
    
    repo_root = Path(__file__).resolve().parents[1]
    
    print_header("Pre-Publish Validation")
    print(f"Repository: {repo_root}\n")
    
    checks = []
    
    # Run checks
    checks.append(("Version Consistency", check_version_consistency(repo_root)))
    checks.append(("Pregenerated Bindings", check_pregenerated_bindings(repo_root)))
    
    if not args.skip_git_check:
        checks.append(("Git Status", check_git_status(repo_root)))
    
    checks.append(("Cargo.lock", check_cargo_lock(repo_root)))
    
    if not args.skip_doc_check:
        checks.append(("Documentation", check_docs_build(repo_root)))
    
    if not args.skip_test_check:
        checks.append(("Tests", check_tests(repo_root)))
    
    # Print summary
    print_header("Validation Summary")
    
    passed = 0
    failed = 0
    
    for name, (success, errors) in checks:
        if success:
            print_success(f"{name}: PASSED")
            passed += 1
        else:
            print_error(f"{name}: FAILED")
            failed += 1
    
    print()
    print(f"Total checks: {len(checks)}")
    print_success(f"Passed: {passed}")
    if failed > 0:
        print_error(f"Failed: {failed}")
    
    if failed == 0:
        print()
        print_success("All checks passed! Ready to publish.")
        print()
        print("Next steps:")
        print("  1. Review changes one more time")
        print("  2. Run: python tools/publish.py --dry-run")
        print("  3. Run: python tools/publish.py")
        return 0
    else:
        print()
        print_error("Some checks failed. Please fix the issues before publishing.")
        return 1


if __name__ == "__main__":
    sys.exit(main())

