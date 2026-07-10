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
  python3 tools/pre_publish_check.py

  # Skip specific checks
  python3 tools/pre_publish_check.py --skip-git-check --skip-doc-check

Requirements:
  - Python 3.11+
  - cargo, git in PATH
"""

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import List, Tuple, Optional, Dict


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

# All publishable crates
ALL_CRATES = [
    ("dear-imgui-build-support", "tools/build-support"),
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-imgui-rs", "dear-imgui"),
    ("dear-imgui-winit", "backends/dear-imgui-winit"),
    ("dear-imgui-wgpu", "backends/dear-imgui-wgpu"),
    ("dear-imgui-glow", "backends/dear-imgui-glow"),
    ("dear-imgui-ash", "backends/dear-imgui-ash"),
    ("dear-imgui-sdl3", "backends/dear-imgui-sdl3"),
    ("dear-imgui-bevy", "backends/dear-imgui-bevy"),
    ("dear-app", "dear-app"),
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-implot", "extensions/dear-implot"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-imnodes", "extensions/dear-imnodes"),
    ("dear-node-editor-sys", "extensions/dear-node-editor-sys"),
    ("dear-node-editor", "extensions/dear-node-editor"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-imguizmo", "extensions/dear-imguizmo"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-implot3d", "extensions/dear-implot3d"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
    ("dear-imguizmo-quat", "extensions/dear-imguizmo-quat"),
    ("dear-imgui-test-engine-sys", "extensions/dear-imgui-test-engine-sys"),
    ("dear-imgui-test-engine", "extensions/dear-imgui-test-engine"),
    ("dear-file-browser", "extensions/dear-file-browser"),
    ("dear-imgui-reflect-derive", "extensions/dear-imgui-reflect-derive"),
    ("dear-imgui-reflect", "extensions/dear-imgui-reflect"),
]

SOURCE_METADATA_SECTION = "package.metadata.dear-imgui-sources"
SOURCE_METADATA_KEYS = {"cimgui-revision", "imgui-revision"}
GIT_REVISION_RE = re.compile(r"^[0-9a-fA-F]{40}$")
SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$")


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
    """Check release crate versions and the independently patched build helper."""
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
    
    build_support_version = versions.pop("dear-imgui-build-support")
    release_versions = set(versions.values())
    if len(release_versions) != 1:
        errors.append("Release crate version mismatch detected:")
        for name, version in sorted(versions.items()):
            errors.append(f"  {name}: {version}")
    else:
        release_version = next(iter(release_versions))
        release_match = SEMVER_RE.fullmatch(release_version)
        build_support_match = SEMVER_RE.fullmatch(build_support_version)
        if release_match is None or build_support_match is None:
            errors.append(
                "Could not compare dear-imgui-build-support and release crate semantic versions"
            )
        else:
            release_parts = tuple(int(part) for part in release_match.groups())
            build_support_parts = tuple(int(part) for part in build_support_match.groups())
            if build_support_parts[:2] != release_parts[:2]:
                errors.append(
                    "dear-imgui-build-support must share the release major/minor version"
                )
            elif build_support_parts[2] < release_parts[2]:
                errors.append(
                    "dear-imgui-build-support patch version cannot trail the release crates"
                )

    if errors:
        for error in errors:
            print_error(error)
        return False, errors

    print_success(
        f"Release crates use {release_version}; build support uses {build_support_version}"
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
        print_error("Run: python3 tools/update_submodule_and_bindings.py --crates all --profile release")
        return False, errors


def read_core_source_metadata(manifest_path: Path) -> Dict[str, str]:
    """Read the exact source provenance schema from the packaged manifest."""
    with manifest_path.open("rb") as manifest_file:
        data = tomllib.load(manifest_file)
    try:
        metadata = data["package"]["metadata"]["dear-imgui-sources"]
    except (KeyError, TypeError) as error:
        raise ValueError(f"missing [{SOURCE_METADATA_SECTION}]") from error
    if not isinstance(metadata, dict) or set(metadata) != SOURCE_METADATA_KEYS:
        found = sorted(metadata) if isinstance(metadata, dict) else type(metadata).__name__
        raise ValueError(
            f"[{SOURCE_METADATA_SECTION}] must contain exactly "
            f"{sorted(SOURCE_METADATA_KEYS)}, found {found}"
        )
    for key, value in metadata.items():
        if not isinstance(value, str) or not GIT_REVISION_RE.fullmatch(value):
            raise ValueError(f"{key} must be a 40-character hexadecimal git revision")
    return metadata


def check_core_source_contract(repo_root: Path) -> Tuple[bool, List[str]]:
    """Require clean vendored sources whose HEADs match packaged metadata."""
    print_check("Dear ImGui source provenance")
    errors = []
    try:
        metadata = read_core_source_metadata(repo_root / "dear-imgui-sys" / "Cargo.toml")
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print_error(str(error))
        return False, [str(error)]

    sources = (
        (
            "cimgui",
            repo_root / "dear-imgui-sys" / "third-party" / "cimgui",
            "cimgui-revision",
        ),
        (
            "Dear ImGui",
            repo_root / "dear-imgui-sys" / "third-party" / "cimgui" / "imgui",
            "imgui-revision",
        ),
    )
    for label, source_path, metadata_key in sources:
        status_code, status, status_error = run_command(
            ["git", "status", "--porcelain=v1", "--untracked-files=all"],
            cwd=source_path,
        )
        if status_code != 0:
            errors.append(f"Could not inspect {label}: {status_error.strip()}")
            continue
        if status:
            errors.append(f"{label} source tree is dirty: {source_path}\n{status.rstrip()}")

        revision_code, revision, revision_error = run_command(
            ["git", "rev-parse", "HEAD"], cwd=source_path
        )
        revision = revision.strip()
        if revision_code != 0 or not GIT_REVISION_RE.fullmatch(revision):
            errors.append(f"Could not read {label} revision: {revision_error.strip()}")
        elif revision != metadata[metadata_key]:
            errors.append(
                f"{label} metadata mismatch: expected {revision}, "
                f"found {metadata[metadata_key]}"
            )
        else:
            print_success(f"{label}: {revision}")

    if errors:
        for error in errors:
            print_error(error)
        return False, errors
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

    code, stdout, stderr = run_command(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=repo_root
    )
    if code != 0:
        detail = stderr.strip() or stdout.strip() or "cargo metadata --locked failed"
        print_error(detail)
        return False, [detail]

    print_success("Cargo.lock satisfies cargo metadata --locked")
    return True, []


def check_changelog_release_notes(repo_root: Path) -> Tuple[bool, List[str]]:
    """Check that the current release has extractable, soft-wrapped release notes."""
    print_check("Changelog release notes")

    version = get_crate_version(repo_root / "dear-imgui")
    if version is None:
        print_error("Could not determine current dear-imgui-rs version")
        return False, ["Could not determine current dear-imgui-rs version"]

    errors = []
    changelog_tool = repo_root / "tools" / "changelog.py"
    for command in (
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

    # NOTE: The workspace contains `dear-imgui-test-engine(-sys)`, which enables
    # `IMGUI_ENABLE_TEST_ENGINE` in `dear-imgui-sys` via Cargo feature unification.
    # Running `cargo test --workspace` would then try to link test binaries that do
    # not depend on the test engine library, causing unresolved hook symbols on
    # some platforms (notably MSVC).
    #
    # We run tests in two passes:
    # 1) All crates except the test-engine crates (no test-engine hooks enabled).
    # 2) The safe test-engine crate itself (ensures the feature-gated path builds/links).
    # 3) dear-imgui-rs with multi-viewport enabled, covering feature-gated PlatformIO callbacks.
    #
    # Prefer nextest when available. Several core tests create Dear ImGui contexts,
    # and the C++ context is a process-global resource. nextest isolates tests more
    # effectively than a single cargo test binary. The cargo-test fallback runs
    # with one test thread for the same reason.

    use_nextest = cargo_nextest_available(repo_root)
    if use_nextest:
        print("  Using cargo nextest")
        base_cmd = ["cargo", "nextest", "run", "--no-tests", "pass", "--workspace", "--lib"]
        test_engine_cmd = [
            "cargo", "nextest", "run", "--no-tests", "pass",
            "-p", "dear-imgui-test-engine", "--lib",
        ]
        multi_viewport_cmd = [
            "cargo", "nextest", "run", "--no-tests", "pass",
            "-p", "dear-imgui-rs", "--features", "multi-viewport",
        ]
        cargo_test_serial_args: List[str] = []
    else:
        print_warning("cargo-nextest not found; falling back to serial cargo test")
        base_cmd = ["cargo", "test", "--workspace", "--lib"]
        test_engine_cmd = ["cargo", "test", "-p", "dear-imgui-test-engine", "--lib"]
        multi_viewport_cmd = ["cargo", "test", "-p", "dear-imgui-rs", "--features", "multi-viewport"]
        cargo_test_serial_args = ["--", "--test-threads=1"]

    base_cmd += ["--exclude", "dear-imgui-test-engine", "--exclude", "dear-imgui-test-engine-sys"]
    base_cmd += cargo_test_serial_args
    test_engine_cmd += cargo_test_serial_args
    multi_viewport_cmd += cargo_test_serial_args

    # Pass 1: core/backends/extensions (excluding test-engine crates)
    code, _stdout, _stderr = run_command(
        base_cmd,
        cwd=repo_root,
        capture=False,  # Stream output in real-time
    )
    if code != 0:
        print_error("Tests failed (workspace without test-engine)")
        return False, ["Tests failed (workspace without test-engine)"]

    # Pass 2: test-engine crate
    code, _stdout, _stderr = run_command(
        test_engine_cmd,
        cwd=repo_root,
        capture=False,  # Stream output in real-time
    )

    if code != 0:
        print_error("Tests failed (dear-imgui-test-engine)")
        return False, ["Tests failed (dear-imgui-test-engine)"]

    # Pass 3: core multi-viewport feature path
    code, _stdout, _stderr = run_command(
        multi_viewport_cmd,
        cwd=repo_root,
        capture=False,
    )

    if code != 0:
        print_error("Tests failed (dear-imgui-rs multi-viewport)")
        return False, ["Tests failed (dear-imgui-rs multi-viewport)"]

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
        "--core-contract-only",
        action="store_true",
        help="Only verify core source provenance and reproducible binding profiles"
    )
    
    args = parser.parse_args()
    
    repo_root = Path(__file__).resolve().parents[1]
    
    print_header("Pre-Publish Validation")
    print(f"Repository: {repo_root}\n")
    
    checks = []
    
    checks.append(("Core Source Provenance", check_core_source_contract(repo_root)))
    checks.append(
        (
            "Core Binding Contract",
            check_core_binding_contract(repo_root, allow_dirty=args.skip_git_check),
        )
    )

    if not args.core_contract_only:
        checks.append(("Version Consistency", check_version_consistency(repo_root)))
        checks.append(("Pregenerated Bindings", check_pregenerated_bindings(repo_root)))

        if not args.skip_git_check:
            checks.append(("Git Status", check_git_status(repo_root)))

        checks.append(("Cargo.lock", check_cargo_lock(repo_root)))
        checks.append(("Changelog", check_changelog_release_notes(repo_root)))

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
        if args.core_contract_only:
            print_success("Core source and binding contracts passed.")
            return 0
        print_success("All checks passed! Ready to publish.")
        print()
        print("Next steps:")
        print("  1. Review changes one more time")
        print("  2. Run: python3 tools/publish.py --dry-run")
        print("  3. Run: python3 tools/publish.py")
        return 0
    else:
        print()
        print_error("Some checks failed. Please fix the issues before publishing.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
