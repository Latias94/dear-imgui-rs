#!/usr/bin/env python3
"""
Automated publishing script for dear-imgui-rs workspace.

This script publishes all crates in the correct dependency order, ensuring that
dependencies are published before their dependents.

Publishing Order:
1. Core: dear-imgui-sys -> dear-imgui-rs
2. Backends: dear-imgui-winit, dear-imgui-wgpu, dear-imgui-glow, dear-imgui-ash, dear-imgui-sdl3
3. Extensions (sys): dear-implot-sys, dear-imnodes-sys, dear-imguizmo-sys,
                     dear-implot3d-sys, dear-imguizmo-quat-sys, dear-imgui-test-engine-sys
4. Extensions (high-level): dear-implot, dear-imnodes, dear-imguizmo,
                            dear-implot3d, dear-imguizmo-quat, dear-imgui-test-engine, dear-file-browser,
                            dear-imgui-reflect-derive, dear-imgui-reflect
5. Application: dear-app

Usage:
  # Dry run (show what would be published)
  python3 tools/publish.py --dry-run

  # Publish all crates
  python3 tools/publish.py

  # Publish specific crates
  python3 tools/publish.py --crates dear-imgui-sys,dear-imgui-rs

  # Skip verification (faster but not recommended)
  python3 tools/publish.py --no-verify

  # Wait longer between publishes (for crates.io to index)
  python3 tools/publish.py --wait 60

Requirements:
  - cargo in PATH
  - Logged in to crates.io (cargo login)
  - All crates must have correct versions in Cargo.toml
  - Pregenerated bindings must be up-to-date for -sys crates
"""

import argparse
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import List, Optional


# Define all crates in dependency order
PUBLISH_ORDER = [
    # Core (must be first)
    ("dear-imgui-sys", "dear-imgui-sys"),
    ("dear-imgui-rs", "dear-imgui"),
    
    # Backends (depend on dear-imgui-rs)
    ("dear-imgui-winit", "backends/dear-imgui-winit"),
    ("dear-imgui-wgpu", "backends/dear-imgui-wgpu"),
    ("dear-imgui-glow", "backends/dear-imgui-glow"),
    ("dear-imgui-ash", "backends/dear-imgui-ash"),
    ("dear-imgui-sdl3", "backends/dear-imgui-sdl3"),
    
    # Extension sys crates (depend on dear-imgui-sys)
    ("dear-implot-sys", "extensions/dear-implot-sys"),
    ("dear-imnodes-sys", "extensions/dear-imnodes-sys"),
    ("dear-imguizmo-sys", "extensions/dear-imguizmo-sys"),
    ("dear-implot3d-sys", "extensions/dear-implot3d-sys"),
    ("dear-imguizmo-quat-sys", "extensions/dear-imguizmo-quat-sys"),
    ("dear-imgui-test-engine-sys", "extensions/dear-imgui-test-engine-sys"),
    
    # Extension high-level crates (depend on dear-imgui-rs and their sys crates)
    ("dear-implot", "extensions/dear-implot"),
    ("dear-imnodes", "extensions/dear-imnodes"),
    ("dear-imguizmo", "extensions/dear-imguizmo"),
    ("dear-implot3d", "extensions/dear-implot3d"),
    ("dear-imguizmo-quat", "extensions/dear-imguizmo-quat"),
    ("dear-imgui-test-engine", "extensions/dear-imgui-test-engine"),
    ("dear-file-browser", "extensions/dear-file-browser"),
    ("dear-imgui-reflect-derive", "extensions/dear-imgui-reflect-derive"),
    ("dear-imgui-reflect", "extensions/dear-imgui-reflect"),
    
    # Application runner (depends on backends and dear-imgui-rs)
    ("dear-app", "dear-app"),
]


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


def get_crate_version(crate_path: Path) -> Optional[str]:
    """Extract version from Cargo.toml."""
    cargo_toml = crate_path / "Cargo.toml"
    if not cargo_toml.exists():
        return None
    
    try:
        with open(cargo_toml, 'r', encoding='utf-8') as f:
            for line in f:
                if line.strip().startswith('version'):
                    # Extract version from line like: version = "0.4.0"
                    parts = line.split('=')
                    if len(parts) == 2:
                        version = parts[1].strip().strip('"').strip("'")
                        # Skip workspace references
                        if not version.startswith('{'):
                            return version
    except Exception as e:
        print_error(f"Failed to read version from {cargo_toml}: {e}")
    
    return None


def check_crate_published(crate_name: str, version: str) -> bool:
    """Check if a crate version is already published on crates.io."""
    try:
        result = subprocess.run(
            ["cargo", "search", crate_name, "--limit", "1"],
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


def sync_dear_imgui_sdl3_backends(repo_root: Path) -> bool:
    """
    Ensure dear-imgui-sdl3 has local copies of the upstream SDL3/OpenGL3 backends.

    We vendor the backend sources from the dear-imgui-sys cimgui/imgui backends
    directory into backends/dear-imgui-sdl3/backends so that the crate published
    to crates.io does not depend on internal layout of dear-imgui-sys packages.
    """
    imgui_backends = (
        repo_root
        / "dear-imgui-sys"
        / "third-party"
        / "cimgui"
        / "imgui"
        / "backends"
    )
    sdl3_backends = (
        repo_root / "backends" / "dear-imgui-sdl3" / "backends"
    )

    required_files = [
        "imgui_impl_sdl3.h",
        "imgui_impl_sdl3.cpp",
        "imgui_impl_opengl3.h",
        "imgui_impl_opengl3.cpp",
        "imgui_impl_opengl3_loader.h",
    ]

    if not imgui_backends.exists():
        print_error(
            f"dear-imgui-sdl3 sync: upstream imgui backends directory not found: {imgui_backends}"
        )
        return False

    sdl3_backends.mkdir(parents=True, exist_ok=True)

    print_info("Syncing dear-imgui-sdl3 vendored backends from dear-imgui-sys...")
    for fname in required_files:
        src = imgui_backends / fname
        dst = sdl3_backends / fname
        if not src.exists():
            print_error(f"Missing upstream backend file: {src}")
            return False
        try:
            shutil.copy2(src, dst)
        except Exception as e:
            print_error(f"Failed to copy {src} -> {dst}: {e}")
            return False

    print_success("dear-imgui-sdl3 backends synced successfully")
    return True


def publish_crate(
    crate_name: str,
    crate_path: Path,
    repo_root: Path,
    dry_run: bool = False,
    no_verify: bool = False,
    wait_time: int = 30
) -> bool:
    """Publish a single crate."""
    print_header(f"Publishing {crate_name}")

    full_path = repo_root / crate_path
    if not full_path.exists():
        print_error(f"Crate path does not exist: {full_path}")
        return False

    # Get version
    version = get_crate_version(full_path)
    if not version:
        print_error(f"Could not determine version for {crate_name}")
        return False

    print_info(f"Crate: {crate_name}")
    print_info(f"Version: {version}")
    print_info(f"Path: {crate_path}")

    # Special handling: keep dear-imgui-sdl3 vendored backends in sync before publishing.
    #
    # Note: in --dry-run mode, do NOT modify the working tree. The dry run is meant to
    # show the publish plan without side effects.
    if crate_name == "dear-imgui-sdl3" and not dry_run:
        if not sync_dear_imgui_sdl3_backends(repo_root):
            print_error("Failed to sync dear-imgui-sdl3 backends; aborting publish")
            return False
    elif crate_name == "dear-imgui-sdl3" and dry_run:
        print_info("DRY RUN: skipping dear-imgui-sdl3 backend sync")

    # Check if already published
    if not dry_run and check_crate_published(crate_name, version):
        print_warning(f"{crate_name} v{version} is already published on crates.io")
        response = input("Skip this crate? [Y/n]: ").strip().lower()
        if response in ('', 'y', 'yes'):
            print_info(f"Skipping {crate_name}")
            return True

    # Build publish command
    cmd = ["cargo", "publish", "-p", crate_name]
    if no_verify:
        cmd.append("--no-verify")

    # Execute publish (stream output in real-time, don't capture)
    result = run_command(cmd, cwd=repo_root, dry_run=dry_run, capture=False)

    if result != 0:
        print_error(f"Failed to publish {crate_name}")
        return False

    print_success(f"Successfully published {crate_name} v{version}")

    # Wait for crates.io to index the crate
    if not dry_run and wait_time > 0:
        print_info(f"Waiting {wait_time} seconds for crates.io to index...")
        time.sleep(wait_time)

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
        help="Show what would be published without actually publishing"
    )
    parser.add_argument(
        "--no-verify",
        action="store_true",
        help="Skip verification (pass --no-verify to cargo publish)"
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
    
    # Print summary
    print_header("Publishing Summary")
    print_info(f"Repository: {repo_root}")
    print_info(f"Crates to publish: {len(crates_to_publish)}")
    print_info(f"Dry run: {args.dry_run}")
    print_info(f"No verify: {args.no_verify}")
    print_info(f"Wait time: {args.wait}s")
    print()
    print("Publishing order:")
    for i, (name, path) in enumerate(crates_to_publish, 1):
        print(f"  {i}. {name} ({path})")
    print()
    
    if not args.dry_run:
        response = input("Continue with publishing? [y/N]: ").strip().lower()
        if response not in ('y', 'yes'):
            print_info("Publishing cancelled")
            return 0
    
    # Publish each crate
    failed_crates = []
    for name, path in crates_to_publish:
        success = publish_crate(
            name,
            Path(path),
            repo_root,
            dry_run=args.dry_run,
            no_verify=args.no_verify,
            wait_time=args.wait
        )
        
        if not success:
            failed_crates.append(name)
            print_error(f"Failed to publish {name}")
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
        print_success(f"Successfully published all {len(crates_to_publish)} crate(s)!")
        return 0


if __name__ == "__main__":
    sys.exit(main())
