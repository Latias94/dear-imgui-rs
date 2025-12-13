#!/usr/bin/env python3
"""
Bump version numbers across the dear-imgui-rs workspace.

This script updates version numbers in all Cargo.toml files, maintaining
the unified release train model where all crates share the same version.

Usage:
  # Bump to a specific version
  python tools/bump_version.py 0.6.0

  # Dry run (show what would be changed)
  python tools/bump_version.py 0.6.0 --dry-run

  # Bump only specific crates
  python tools/bump_version.py 0.6.0 --crates dear-imgui-sys,dear-imgui-rs

Requirements:
  - Python 3.7+
  - Write access to the repository
"""

import argparse
import re
import sys
from pathlib import Path
from typing import List, Tuple, Optional


# All crates in the workspace (excluding examples and tools)
WORKSPACE_CRATES = [
    "dear-imgui-sys",
    "dear-imgui",
    "backends/dear-imgui-winit",
    "backends/dear-imgui-wgpu",
    "backends/dear-imgui-glow",
    "backends/dear-imgui-sdl3",
    "dear-app",
    "extensions/dear-implot-sys",
    "extensions/dear-implot",
    "extensions/dear-imnodes-sys",
    "extensions/dear-imnodes",
    "extensions/dear-imguizmo-sys",
    "extensions/dear-imguizmo",
    "extensions/dear-implot3d-sys",
    "extensions/dear-implot3d",
    "extensions/dear-imguizmo-quat-sys",
    "extensions/dear-imguizmo-quat",
    "extensions/dear-file-browser",
    "extensions/dear-imgui-reflect-derive",
    "extensions/dear-imgui-reflect",
]


class Colors:
    """ANSI color codes."""
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'


def print_info(msg: str):
    print(f"INFO: {msg}")


def print_success(msg: str):
    print(f"{Colors.OKGREEN}OK: {msg}{Colors.ENDC}")


def print_warning(msg: str):
    print(f"{Colors.WARNING}WARN: {msg}{Colors.ENDC}")


def print_error(msg: str):
    print(f"{Colors.FAIL}ERR: {msg}{Colors.ENDC}", file=sys.stderr)


def validate_version(version: str) -> bool:
    """Validate semver format."""
    pattern = r'^\d+\.\d+\.\d+(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$'
    return re.match(pattern, version) is not None


def get_version_pattern(old_version: str) -> str:
    """Get the version pattern for matching (without pre-release/build metadata)."""
    # Extract major.minor.patch
    match = re.match(r'^(\d+\.\d+\.\d+)', old_version)
    if match:
        return match.group(1)
    return old_version


def update_cargo_toml(
    path: Path,
    old_version: str,
    new_version: str,
    dry_run: bool = False
) -> Tuple[bool, List[str]]:
    """
    Update version in a Cargo.toml file.
    
    Returns:
        (success, changes) where changes is a list of change descriptions
    """
    if not path.exists():
        return False, [f"File not found: {path}"]
    
    try:
        with open(path, 'r', encoding='utf-8') as f:
            content = f.read()
            original_content = content
    except Exception as e:
        return False, [f"Failed to read {path}: {e}"]
    
    changes = []
    
    # Pattern 1: Direct version assignment
    # version = "0.4.0"
    pattern1 = re.compile(
        r'^(\s*version\s*=\s*["\'])' + re.escape(old_version) + r'(["\'])',
        re.MULTILINE
    )
    new_content, count1 = pattern1.subn(r'\g<1>' + new_version + r'\g<2>', content)
    if count1 > 0:
        changes.append(f"Updated package version: {old_version} -> {new_version}")
        content = new_content
    
    # Pattern 2: Dependency version with path
    # dear-imgui-sys = { path = "...", version = "0.4" }
    # We need to update to the minor version (e.g., "0.6" for "0.6.0")
    old_minor = '.'.join(old_version.split('.')[:2])
    new_minor = '.'.join(new_version.split('.')[:2])
    
    pattern2 = re.compile(
        r'(dear-[a-z0-9-]+\s*=\s*\{[^}]*version\s*=\s*["\'])' + 
        re.escape(old_minor) + 
        r'(["\'][^}]*\})',
        re.MULTILINE
    )
    new_content, count2 = pattern2.subn(r'\g<1>' + new_minor + r'\g<2>', content)
    if count2 > 0:
        changes.append(f"Updated {count2} dependency version(s): {old_minor} -> {new_minor}")
        content = new_content
    
    # Pattern 3: Workspace dependency version
    # dear-imgui-build-support = { version = "0.1", path = "..." }
    # (We don't update build-support as it has independent versioning)
    
    if content == original_content:
        return True, ["No changes needed"]
    
    if not dry_run:
        try:
            with open(path, 'w', encoding='utf-8') as f:
                f.write(content)
        except Exception as e:
            return False, [f"Failed to write {path}: {e}"]
    
    return True, changes


def get_current_version(repo_root: Path) -> Optional[str]:
    """Get the current version from dear-imgui-sys/Cargo.toml."""
    cargo_toml = repo_root / "dear-imgui-sys" / "Cargo.toml"
    if not cargo_toml.exists():
        return None
    
    try:
        with open(cargo_toml, 'r', encoding='utf-8') as f:
            for line in f:
                if line.strip().startswith('version'):
                    match = re.search(r'version\s*=\s*["\']([^"\']+)["\']', line)
                    if match:
                        return match.group(1)
    except Exception:
        pass
    
    return None


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Bump version numbers across the workspace",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        "new_version",
        help="New version number (e.g., 0.6.0)"
    )
    parser.add_argument(
        "--old-version",
        help="Old version to replace (default: auto-detect from dear-imgui-sys)"
    )
    parser.add_argument(
        "--crates",
        help="Comma-separated list of crate paths to update (default: all)"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be changed without making changes"
    )
    parser.add_argument(
        "--skip-readme",
        action="store_true",
        help="Skip updating README files"
    )

    args = parser.parse_args()
    
    # Validate new version
    if not validate_version(args.new_version):
        print_error(f"Invalid version format: {args.new_version}")
        print_info("Expected format: MAJOR.MINOR.PATCH (e.g., 0.6.0)")
        return 1
    
    # Get repository root
    repo_root = Path(__file__).resolve().parents[1]
    
    # Determine old version
    if args.old_version:
        old_version = args.old_version
    else:
        old_version = get_current_version(repo_root)
        if not old_version:
            print_error("Could not auto-detect current version")
            print_info("Please specify --old-version manually")
            return 1
    
    print_info(f"Repository: {repo_root}")
    print_info(f"Old version: {old_version}")
    print_info(f"New version: {args.new_version}")
    
    if args.dry_run:
        print_warning("DRY RUN MODE - No files will be modified")
    
    # Determine which crates to update
    if args.crates:
        crate_paths = [c.strip() for c in args.crates.split(",")]
    else:
        crate_paths = WORKSPACE_CRATES
    
    print_info(f"Updating {len(crate_paths)} crate(s)\n")
    
    # Update each crate
    success_count = 0
    error_count = 0
    
    for crate_path in crate_paths:
        full_path = repo_root / crate_path / "Cargo.toml"
        
        print(f"\n{Colors.BOLD}Updating: {crate_path}{Colors.ENDC}")
        
        success, changes = update_cargo_toml(
            full_path,
            old_version,
            args.new_version,
            dry_run=args.dry_run
        )
        
        if success:
            for change in changes:
                if "No changes needed" in change:
                    print_warning(change)
                else:
                    print_success(change)
            success_count += 1
        else:
            for change in changes:
                print_error(change)
            error_count += 1
    
    # Print summary
    print(f"\n{Colors.BOLD}{'=' * 80}{Colors.ENDC}")
    print(f"{Colors.BOLD}Summary{Colors.ENDC}")
    print(f"{Colors.BOLD}{'=' * 80}{Colors.ENDC}\n")
    
    print_success(f"Successfully updated: {success_count} crate(s)")
    if error_count > 0:
        print_error(f"Failed to update: {error_count} crate(s)")
    
    # Update README files if not skipped
    if not args.skip_readme:
        print(f"\n{Colors.BOLD}{'=' * 80}{Colors.ENDC}")
        print(f"{Colors.BOLD}Updating README files{Colors.ENDC}")
        print(f"{Colors.BOLD}{'=' * 80}{Colors.ENDC}\n")

        # Import and run the README updater
        import subprocess
        readme_cmd = [
            sys.executable,
            str(repo_root / "tools" / "update_readme_versions.py"),
            args.new_version,
            "--old-version", old_version
        ]
        if args.dry_run:
            readme_cmd.append("--dry-run")

        try:
            result = subprocess.run(readme_cmd, check=False)
            if result.returncode != 0:
                print_warning("README update had some issues, but continuing...")
        except Exception as e:
            print_warning(f"Failed to run README updater: {e}")

    if args.dry_run:
        print_warning("\nDRY RUN: No files were actually modified")
        print_info("Run without --dry-run to apply changes")
    else:
        print_info("\nNext steps:")
        print("  1. Review the changes: git diff")
        print("  2. Update CHANGELOG.md")
        print("  3. Update docs/COMPATIBILITY.md")
        print("  4. Run: cargo update")
        print("  5. Test: cargo test --workspace")
        print("  6. Commit: git add -A && git commit -m 'chore: bump version to " + args.new_version + "'")

    return 1 if error_count > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
