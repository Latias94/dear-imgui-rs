#!/usr/bin/env python3
"""
Pre-publish validation script for dear-imgui-rs workspace.

This script performs various checks to ensure the workspace is ready for publishing:
- Version consistency across all crates
- Pregenerated bindings exist for -sys crates
- Git working tree is clean
- Cargo.lock is up-to-date
- Python release-tool contracts and release policy are valid
- Core and supported high-level extensions build for wasm32
- Documentation builds successfully
- Packaged core crates build from an offline consumer

Usage:
  python3 tools/pre_publish_check.py

  # Skip specific checks
  python3 tools/pre_publish_check.py \
    --skip-git-check --skip-doc-check --skip-package-check

Requirements:
  - Python 3.11+
  - cargo, git in PATH
"""

import argparse
import subprocess
import sys
from pathlib import Path
from typing import List, Optional, Tuple

import source_metadata
from release_metadata import (
    MetadataError,
    PUBLISH_ORDER,
    WorkspaceMetadata,
    load_workspace_metadata,
    validate_publish_order,
    validate_release_workspace,
)


# Crates that should have pregenerated bindings
SYS_CRATES = [
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-node-editor-sys", "extensions/dear-node-editor-sys"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
    ("dear-imgui-test-engine-sys", "extensions/dear-imgui-test-engine-sys"),
]

# Crates that should have rustdoc coverage in the pre-publish documentation gate.
DOC_CRATES = [
    (
        "dear-imgui-bevy",
        "backends/dear-imgui-bevy",
        ["--features", "render,multi-viewport"],
    ),
]

# Run release tests one package at a time. A workspace-wide nextest invocation asks
# nextest to enumerate every binary concurrently and also unifies unrelated Cargo
# features. Besides being less deterministic, that has deadlocked the macOS dynamic
# loader during the `--list` phase. Keeping this list tied to PUBLISH_ORDER makes the
# release gate fail closed when the publish topology changes.
RELEASE_TEST_PACKAGES = tuple(name for name, _path in PUBLISH_ORDER)
PRIVATE_RELEASE_TEST_PACKAGES = ("xtask",)
PACKAGE_TEST_FEATURES = {
    "dear-imgui-build-support": ("binding-spec",),
}
WASM_RELEASE_PACKAGES = (
    "dear-imgui-rs",
    "dear-imgui-glow",
    "dear-implot",
    "dear-implot3d",
    "dear-imnodes",
    "dear-imguizmo",
    "dear-imguizmo-quat",
)


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
    print(f"{Colors.OKBLUE}CHECK: {msg}{Colors.ENDC}")


def print_success(msg: str):
    print(f"{Colors.OKGREEN}OK: {msg}{Colors.ENDC}")


def print_warning(msg: str):
    print(f"{Colors.WARNING}WARN: {msg}{Colors.ENDC}")


def print_error(msg: str):
    print(f"{Colors.FAIL}ERR: {msg}{Colors.ENDC}")


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


def cargo_nextest_available(repo_root: Path) -> bool:
    """Return True if cargo-nextest is installed."""
    code, _stdout, _stderr = run_command(
        ["cargo", "nextest", "--version"],
        cwd=repo_root,
        capture=True,
    )
    return code == 0


def release_contract_commands() -> list[tuple[str, list[str]]]:
    """Build the deterministic local release-contract command sequence."""
    wasm_features = ",".join(
        f"{package_name}/wasm" for package_name in WASM_RELEASE_PACKAGES
    )
    wasm_command = [
        "cargo",
        "check",
        "--target",
        "wasm32-unknown-unknown",
        "--no-default-features",
    ]
    for package_name in WASM_RELEASE_PACKAGES:
        wasm_command.extend(["-p", package_name])
    wasm_command.extend(["--features", wasm_features])

    return [
        (
            "Python contract suite",
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
        ),
        (
            "Workflow policy",
            [sys.executable, "tools/ci/workflow_policy.py", "--check"],
        ),
        (
            "WASM core and high-level extensions",
            wasm_command,
        ),
    ]


def check_release_contracts(repo_root: Path) -> Tuple[bool, List[str]]:
    """Run local release contracts in order and stop at the first failure."""
    print_check("Python, workflow, and WASM release contracts")
    for label, command in release_contract_commands():
        print(f"\n  Checking {label}...")
        code, stdout, stderr = run_command(
            command,
            cwd=repo_root,
            capture=False,
        )
        if code != 0:
            detail = stderr.strip() or stdout.strip() or f"exit code {code}"
            error = f"{label} failed: {detail}"
            print_error(error)
            return False, [error]
        print_success(label)

    print_success("All local release contracts passed")
    return True, []


def release_test_commands(use_nextest: bool) -> list[tuple[str, list[str]]]:
    """Build deterministic per-package and feature-profile test commands."""
    commands: list[tuple[str, list[str]]] = []
    for package_name in RELEASE_TEST_PACKAGES:
        if use_nextest:
            command = [
                "cargo",
                "nextest",
                "run",
                "--no-tests",
                "pass",
                "-p",
                package_name,
            ]
        else:
            command = ["cargo", "test", "-p", package_name]

        features = PACKAGE_TEST_FEATURES.get(package_name, ())
        if features:
            command += ["--features", ",".join(features)]
        if not use_nextest:
            command += ["--", "--test-threads=1"]
        commands.append((package_name, command))

    for package_name in PRIVATE_RELEASE_TEST_PACKAGES:
        if use_nextest:
            command = [
                "cargo",
                "nextest",
                "run",
                "--no-tests",
                "pass",
                "-p",
                package_name,
            ]
        else:
            command = [
                "cargo",
                "test",
                "-p",
                package_name,
                "--",
                "--test-threads=1",
            ]
        commands.append((package_name, command))

    feature_profiles = (
        (
            "dear-imgui-rs multi-viewport",
            ["-p", "dear-imgui-rs", "--features", "multi-viewport"],
        ),
        (
            "dear-imgui-rs stack-layout integration",
            [
                "-p",
                "dear-imgui-rs",
                "--no-default-features",
                "--features",
                "stack-layout",
                "--test",
                "stack_layout_context",
            ],
        ),
        (
            "dear-imgui-wgpu tracing",
            [
                "-p",
                "dear-imgui-wgpu",
                "--no-default-features",
                "--features",
                "wgpu-30,tracing",
            ],
        ),
    )
    for label, cargo_args in feature_profiles:
        if use_nextest:
            command = [
                "cargo",
                "nextest",
                "run",
                "--no-tests",
                "pass",
                *cargo_args,
            ]
        else:
            command = [
                "cargo",
                "test",
                *cargo_args,
                "--",
                "--test-threads=1",
            ]
        commands.append((label, command))

    return commands


def read_locked_workspace_metadata(
    repo_root: Path,
) -> tuple[Optional[WorkspaceMetadata], Tuple[bool, List[str]]]:
    """Load the one Cargo metadata snapshot used by every release check."""
    print_check("Locked Cargo workspace metadata")
    try:
        metadata = load_workspace_metadata(repo_root)
    except MetadataError as error:
        print_error(str(error))
        return None, (False, [str(error)])

    print_success(
        f"Cargo.lock resolves {len(metadata.publishable_packages)} publishable and "
        f"{len(metadata.private_packages)} private workspace packages"
    )
    return metadata, (True, [])


def check_version_consistency(
    repo_root: Path, metadata: WorkspaceMetadata
) -> Tuple[bool, List[str]]:
    """Check release versions and every internal workspace dependency edge."""
    print_check("Release versions and internal path requirements")
    errors = [
        *validate_release_workspace(metadata),
        *validate_publish_order(metadata, PUBLISH_ORDER, repo_root),
    ]
    if errors:
        for error in errors:
            print_error(error)
        return False, errors

    print_success(
        f"All {len(metadata.publishable_packages)} publishable packages use "
        f"{metadata.release_version}; internal path requirements match their targets"
    )
    return True, []


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

    for label, relative_path in (
        (
            "dear-imgui-sys Windows ABI profile",
            "dear-imgui-sys/src/bindings_pregenerated_windows.rs",
        ),
        (
            "dear-imgui-sys WASM import profile",
            "dear-imgui-sys/src/wasm_bindings_pregenerated.rs",
        ),
    ):
        path = repo_root / relative_path
        if not path.exists():
            errors.append(f"Missing pregenerated bindings: {label}")
            print_error(f"Missing: {path}")
        elif path.stat().st_size < 1000:
            errors.append(f"Pregenerated bindings too small: {label}")
            print_error(f"Too small: {path} ({path.stat().st_size} bytes)")
        else:
            print_success(f"{label}: {path.stat().st_size:,} bytes")
    
    if not errors:
        print_success("All -sys crates have pregenerated bindings")
        return True, []
    else:
        print_error("Run: python3 tools/update_submodule_and_bindings.py --crates all")
        return False, errors


def check_core_source_contract(repo_root: Path) -> Tuple[bool, List[str]]:
    """Require clean vendored sources whose HEADs match packaged metadata."""
    print_check("Dear ImGui source provenance")
    try:
        revisions = source_metadata.verify_core_source_metadata(repo_root)
    except source_metadata.SourceMetadataError as error:
        for message in error.errors:
            print_error(message)
        return False, list(error.errors)

    for source in source_metadata.CORE_SOURCE_SPECS:
        print_success(f"{source.label}: {revisions[source.metadata_key]}")
    return True, []


def check_core_binding_contract(
    repo_root: Path, allow_dirty: bool
) -> Tuple[bool, List[str]]:
    """Regenerate every supported core ABI profile and require exact parity."""
    print_check("Core binding specification and ABI profiles")
    command = ["cargo", "run", "-p", "xtask", "--", "verify-bindings"]
    if allow_dirty:
        command.append("--allow-dirty")
    code, stdout, stderr = run_command(command, cwd=repo_root, capture=True)
    if code != 0:
        detail = stderr.strip() or stdout.strip() or "core binding verification failed"
        print_error(detail)
        return False, [detail]
    if stderr.strip():
        print(stderr.strip())
    print_success("Core native/WASM bindings match the shared binding specification")
    return True, []


def check_git_status(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that git working tree is clean."""
    print_check("Git working tree status")
    
    code, stdout, stderr = run_command(
        ["git", "status", "--porcelain", "--ignore-submodules=none"],
        cwd=repo_root,
    )
    
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


def check_changelog_release_notes(
    repo_root: Path, metadata: WorkspaceMetadata
) -> Tuple[bool, List[str]]:
    """Check changelog structure and the current release notes."""
    print_check("Changelog release notes")

    try:
        version = metadata.release_version
    except MetadataError as error:
        print_error(str(error))
        return False, [str(error)]

    errors = []
    changelog_tool = repo_root / "tools" / "changelog.py"
    for command in (
        ["check-unreleased"],
        ["extract", "--version", version],
        ["check-soft-wrap", "--version", version],
    ):
        code, stdout, stderr = run_command(
            [sys.executable, str(changelog_tool), *command],
            cwd=repo_root,
            capture=True,
        )
        if code != 0:
            errors.append(f"CHANGELOG.md failed {' '.join(command)}")
            if stdout:
                print(stdout)
            if stderr:
                print_error(stderr.strip())

    if errors:
        return False, errors

    print_success(f"CHANGELOG.md has release notes for {version}")
    return True, []


def check_packaged_core(repo_root: Path) -> Tuple[bool, List[str]]:
    """Package and consume the core crates from a clean isolated checkout."""
    print_check("Packaged core crates and offline consumption")
    command = [
        sys.executable,
        str(repo_root / "tools" / "ci" / "verify_packaged_core.py"),
    ]
    code, stdout, stderr = run_command(
        command,
        cwd=repo_root,
        capture=True,
        show_output=True,
    )
    if code != 0:
        detail = stderr.strip() or stdout.strip() or "packaged core verification failed"
        print_error(detail)
        return False, [detail]

    print_success("Packaged dear-imgui-sys builds offline with its packaged helper")
    return True, []


def check_docs_build(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that documentation-related publish gates build."""
    print_check("Documentation builds (-sys offline mode plus selected rustdoc crates)")

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

    for name, _path, extra_args in DOC_CRATES:
        print(f"\n  Documenting {name}...")

        env = os.environ.copy()
        env["DOCS_RS"] = "1"
        rustdocflags = env.get("RUSTDOCFLAGS", "")
        env["RUSTDOCFLAGS"] = f"{rustdocflags} -D warnings".strip()

        try:
            result = subprocess.run(
                ["cargo", "doc", "-p", name, "--no-deps", *extra_args],
                cwd=repo_root,
                env=env,
                check=False
            )
            code = result.returncode
        except Exception as e:
            print_error(f"Failed to run cargo doc: {e}")
            code = 1

        if code != 0:
            errors.append(f"Doc build failed for {name}")
            print_error(f"Failed: {name}")
        else:
            print_success(f"OK: {name}")

    if not errors:
        print_success("\nAll documentation checks passed")
        return True, []
    else:
        return False, errors


def check_tests(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that tests pass."""
    print_check("Running tests")

    use_nextest = cargo_nextest_available(repo_root)
    if use_nextest:
        print("  Using cargo nextest with one package per invocation")
    else:
        print_warning("cargo-nextest not found; falling back to serial cargo test")

    for label, command in release_test_commands(use_nextest):
        print(f"\n  Testing {label}...")
        code, _stdout, _stderr = run_command(
            command,
            cwd=repo_root,
            capture=False,
        )
        if code != 0:
            error = f"Tests failed ({label})"
            print_error(error)
            return False, [error]

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
    parser.add_argument(
        "--skip-package-check",
        action="store_true",
        help="Skip the clean-clone package and offline-consumer release gate"
    )
    parser.add_argument(
        "--core-contract-only",
        action="store_true",
        help="Only verify core source provenance and reproducible binding profiles"
    )
    
    args = parser.parse_args()

    if (
        args.skip_git_check
        and not args.skip_package_check
        and not args.core_contract_only
    ):
        parser.error(
            "--skip-git-check must be paired with --skip-package-check: "
            "the package gate verifies a clean clone of HEAD, not dirty worktree changes"
        )
    
    repo_root = Path(__file__).resolve().parents[1]
    
    print_header("Pre-Publish Validation")
    print(f"Repository: {repo_root}\n")
    
    checks = []

    metadata: Optional[WorkspaceMetadata] = None
    if not args.core_contract_only:
        metadata, metadata_check = read_locked_workspace_metadata(repo_root)
        checks.append(("Locked Workspace Metadata", metadata_check))
    
    checks.append(("Core Source Provenance", check_core_source_contract(repo_root)))
    checks.append(
        (
            "Core Binding Contract",
            check_core_binding_contract(repo_root, allow_dirty=args.skip_git_check),
        )
    )

    if not args.core_contract_only:
        if metadata is not None:
            checks.append(
                (
                    "Version Consistency",
                    check_version_consistency(repo_root, metadata),
                )
            )
        checks.append(("Pregenerated Bindings", check_pregenerated_bindings(repo_root)))

        if not args.skip_git_check:
            checks.append(("Git Status", check_git_status(repo_root)))

        if metadata is not None:
            checks.append(
                ("Changelog", check_changelog_release_notes(repo_root, metadata))
            )

        if not args.skip_doc_check:
            checks.append(("Documentation", check_docs_build(repo_root)))

        if not args.skip_test_check:
            checks.append(
                ("Release Contracts", check_release_contracts(repo_root))
            )
            checks.append(("Tests", check_tests(repo_root)))

        if not args.skip_package_check:
            checks.append(("Packaged Core", check_packaged_core(repo_root)))
    
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
        if args.core_contract_only:
            print_success("Core source and binding contracts passed.")
            return 0
        print_success("All checks passed! Ready to publish.")
        print()
        print("Next steps:")
        print("  1. Review changes one more time")
        print("  2. Run: python3 tools/publish.py --dry-run")
        print("  3. Prefer the protected release workflow for publication")
        print("  4. For manual recovery only: python3 tools/publish.py --yes")
        return 0
    else:
        print()
        print_error("Some checks failed. Please fix the issues before publishing.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
