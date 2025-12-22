#!/usr/bin/env python3
"""
Update version numbers in README files across the workspace.

This script updates version numbers in README.md files, particularly in
compatibility tables and installation examples.

Usage:
  # Update to a specific version
  python3 tools/update_readme_versions.py 0.5.0

  # Dry run (show what would be changed)
  python3 tools/update_readme_versions.py 0.5.0 --dry-run

  # Specify old version manually
  python3 tools/update_readme_versions.py 0.5.0 --old-version 0.4.0

Requirements:
  - Python 3.7+
"""

import argparse
import re
import sys
from pathlib import Path
from typing import List, Tuple, Optional


# README files to update
README_FILES = [
    "README.md",
    "backends/dear-imgui-wgpu/README.md",
    "backends/dear-imgui-glow/README.md",
    "backends/dear-imgui-winit/README.md",
    "dear-app/README.md",
    "extensions/dear-implot/README.md",
    "extensions/dear-imnodes/README.md",
    "extensions/dear-imguizmo/README.md",
    "extensions/dear-implot3d/README.md",
    "extensions/dear-imguizmo-quat/README.md",
    "extensions/dear-file-browser/README.md",
    "extensions/dear-imgui-reflect/README.md",
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


def update_readme(
    path: Path,
    old_version: str,
    new_version: str,
    dry_run: bool = False
) -> Tuple[bool, List[str]]:
    """
    Update version in a README file.
    
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
    
    # Extract major.minor for dependency versions
    old_minor = '.'.join(old_version.split('.')[:2])
    new_minor = '.'.join(new_version.split('.')[:2])
    
    # Pattern 1: Compatibility table entries like "| Crate | 0.3.x |"
    pattern1 = re.compile(
        r'(\|\s*(?:Crate|dear-[a-z0-9-]+)\s*\|\s*)' + re.escape(old_minor) + r'(\.x\s*\|)',
        re.MULTILINE
    )
    new_content, count1 = pattern1.subn(r'\g<1>' + new_minor + r'\g<2>', content)
    if count1 > 0:
        changes.append(f"Updated {count1} compatibility table entr{'y' if count1 == 1 else 'ies'}: {old_minor}.x -> {new_minor}.x")
        content = new_content
    
    # Pattern 2: Cargo.toml dependency examples like 'dear-imgui-rs = "0.4"'
    pattern2 = re.compile(
        r'(dear-[a-z0-9-]+\s*=\s*["\'])' + re.escape(old_minor) + r'(["\'])',
        re.MULTILINE
    )
    new_content, count2 = pattern2.subn(r'\g<1>' + new_minor + r'\g<2>', content)
    if count2 > 0:
        changes.append(f"Updated {count2} dependency example(s): {old_minor} -> {new_minor}")
        content = new_content
    
    # Pattern 3: Cargo.toml dependency with version key like 'version = "0.4"'
    pattern3 = re.compile(
        r'(version\s*=\s*["\'])' + re.escape(old_minor) + r'(["\'])',
        re.MULTILINE
    )
    new_content, count3 = pattern3.subn(r'\g<1>' + new_minor + r'\g<2>', content)
    if count3 > 0:
        changes.append(f"Updated {count3} version specification(s): {old_minor} -> {new_minor}")
        content = new_content
    
    if content == original_content:
        return True, ["No changes needed"]
    
    if not dry_run:
        try:
            with open(path, 'w', encoding='utf-8') as f:
                f.write(content)
        except Exception as e:
            return False, [f"Failed to write {path}: {e}"]
    
    return True, changes


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Update version numbers in README files",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        "new_version",
        help="New version number (e.g., 0.5.0)"
    )
    parser.add_argument(
        "--old-version",
        help="Old version to replace (default: auto-detect from dear-imgui-sys)"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be changed without making changes"
    )
    
    args = parser.parse_args()
    
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
    
    print_info(f"Updating {len(README_FILES)} README file(s)\n")
    
    # Update each README
    success_count = 0
    error_count = 0
    no_change_count = 0
    
    for readme_path in README_FILES:
        full_path = repo_root / readme_path
        
        print(f"\n{Colors.BOLD}Updating: {readme_path}{Colors.ENDC}")
        
        success, changes = update_readme(
            full_path,
            old_version,
            args.new_version,
            dry_run=args.dry_run
        )
        
        if success:
            has_changes = False
            for change in changes:
                if "No changes needed" in change:
                    print_warning(change)
                    no_change_count += 1
                else:
                    print_success(change)
                    has_changes = True
            if has_changes:
                success_count += 1
        else:
            for change in changes:
                print_error(change)
            error_count += 1
    
    # Print summary
    print(f"\n{Colors.BOLD}{'=' * 80}{Colors.ENDC}")
    print(f"{Colors.BOLD}Summary{Colors.ENDC}")
    print(f"{Colors.BOLD}{'=' * 80}{Colors.ENDC}\n")
    
    print_success(f"Successfully updated: {success_count} file(s)")
    if no_change_count > 0:
        print_warning(f"No changes needed: {no_change_count} file(s)")
    if error_count > 0:
        print_error(f"Failed to update: {error_count} file(s)")
    
    if args.dry_run:
        print_warning("\nDRY RUN: No files were actually modified")
        print_info("Run without --dry-run to apply changes")
    else:
        print_info("\nNext steps:")
        print("  1. Review the changes: git diff")
        print("  2. Commit: git add -A && git commit -m 'docs: update README versions to " + args.new_version + "'")
    
    return 1 if error_count > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
