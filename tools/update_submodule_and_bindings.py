#!/usr/bin/env python3
"""
Update third-party submodules and refresh pregenerated bindings for sys crates.

Why: docs.rs builds are offline and cannot fetch submodules. To guarantee
successful docs.rs builds, we pre-generate Rust bindings and vendor headers
via submodules locally before publishing.

Supported crates:
  - dear-imgui-sys (cimgui)
  - extensions/dear-implot-sys (cimplot)
  - extensions/dear-imnodes-sys (cimnodes)
  - extensions/dear-imguizmo-sys (cimguizmo)

Usage examples:
  - Update cimgui and regenerate bindings for dear-imgui-sys (Debug):
      python tools/update_submodule_and_bindings.py --crates dear-imgui-sys \
        --submodules update

  - Update all submodules to specific branches and pregen bindings (Release):
      python tools/update_submodule_and_bindings.py --crates all --profile release \
        --submodules update \
        --cimgui-branch docking_inter --cimplot-branch master \
        --cimnodes-branch master --cimguizmo-branch master

  - Only regenerate pregenerated bindings without touching submodules:
      python tools/update_submodule_and_bindings.py --crates dear-implot-sys,dear-imnodes-sys \
        --submodules skip

  - Dry-run (print commands only):
      python tools/update_submodule_and_bindings.py --crates all --dry-run

Requirements:
  - git, cargo in PATH
  - Python 3.7+
"""

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


def run(cmd, cwd=None, env=None, dry=False):
    print("$", " ".join(cmd))
    if dry:
        return 0
    try:
        subprocess.check_call(cmd, cwd=cwd, env=env)
        return 0
    except subprocess.CalledProcessError as e:
        print(f"Command failed (exit {e.returncode}): {' '.join(cmd)}", file=sys.stderr)
        return e.returncode


def find_bindings(target_dir: Path, profile: str, crate: str) -> Path:
    build_dir = target_dir / profile / "build"
    if not build_dir.exists():
        return None
    # build dir prefix is crate name with a hash suffix
    pattern = f"{crate}-*/out/bindings.rs"
    for p in build_dir.glob(pattern):
        return p
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Update third-party submodules and pregenerate bindings for sys crates")
    parser.add_argument("--crates", default="dear-imgui-sys", help="Comma-separated list of crates to process (or 'all')")
    parser.add_argument("--profile", default="debug", choices=["debug", "release"], help="Cargo profile when generating bindings")
    parser.add_argument("--submodules", default="auto", choices=["auto", "update", "skip"], help="Whether to update submodules: auto=update only for selected crates; update=update all known submodules; skip=don't touch submodules")
    # Branch selection per submodule
    parser.add_argument("--cimgui-branch", default="docking_inter", help="Branch for cimgui submodule (dear-imgui-sys)")
    parser.add_argument("--cimplot-branch", default="master", help="Branch for cimplot submodule (dear-implot-sys)")
    parser.add_argument("--cimnodes-branch", default="master", help="Branch for cimnodes submodule (dear-imnodes-sys)")
    parser.add_argument("--cimguizmo-branch", default="master", help="Branch for cimguizmo submodule (dear-imguizmo-sys)")
    parser.add_argument("--remote", default="origin", help="Remote name for submodules")
    parser.add_argument("--dry-run", action="store_true", help="Print commands without executing")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    # Known crate roots and submodules
    crate_roots = {
        "dear-imgui-sys": repo_root / "dear-imgui-sys",
        "dear-implot-sys": repo_root / "extensions/dear-implot-sys",
        "dear-imnodes-sys": repo_root / "extensions/dear-imnodes-sys",
        "dear-imguizmo-sys": repo_root / "extensions/dear-imguizmo-sys",
    }
    submodules = {
        "dear-imgui-sys": (crate_roots["dear-imgui-sys"] / "third-party/cimgui", args.cimgui_branch),
        "dear-implot-sys": (crate_roots["dear-implot-sys"] / "third-party/cimplot", args.cimplot_branch),
        "dear-imnodes-sys": (crate_roots["dear-imnodes-sys"] / "third-party/cimnodes", args.cimnodes_branch),
        "dear-imguizmo-sys": (crate_roots["dear-imguizmo-sys"] / "third-party/cimguizmo", args.cimguizmo_branch),
    }

    # Parse crates list
    if args.crates.strip().lower() == "all":
        crates = list(submodules.keys())
    else:
        crates = [c.strip() for c in args.crates.split(",") if c.strip()]
        unknown = [c for c in crates if c not in submodules]
        if unknown:
            print(f"Unknown crates: {unknown}", file=sys.stderr)
            return 2

    # Optionally update submodules
    if args.submodules != "skip":
        targets = submodules.keys() if args.submodules == "update" else crates
        for c in targets:
            path, branch = submodules[c]
            if not path.exists():
                print(f"Submodule path not found: {path}", file=sys.stderr)
                return 2
            print(f"Updating submodule for {c}: {path} -> {branch}")
            rc = run(["git", "-C", str(path), "fetch", args.remote, "--tags"], dry=args.dry_run)
            if rc != 0:
                return rc
            rc = run(["git", "-C", str(path), "checkout", branch], dry=args.dry_run)
            if rc != 0:
                return rc
            rc = run(["git", "-C", str(path), "pull", args.remote, branch], dry=args.dry_run)
            if rc != 0:
                return rc
            run(["git", "-C", str(path), "submodule", "update", "--init", "--recursive"], dry=args.dry_run)

    # Generate pregenerated bindings for selected crates
    env_base = os.environ.copy()
    profile_flag = ["--release"] if args.profile == "release" else []
    crate_skip_env = {
        "dear-imgui-sys": "IMGUI_SYS_SKIP_CC",
        "dear-implot-sys": "IMPLOT_SYS_SKIP_CC",
        "dear-imnodes-sys": "IMNODES_SYS_SKIP_CC",
        "dear-imguizmo-sys": "IMGUIZMO_SYS_SKIP_CC",
    }
    target_dir = Path(env_base.get("CARGO_TARGET_DIR", repo_root / "target"))
    for crate in crates:
        env = env_base.copy()
        env[crate_skip_env[crate]] = "1"
        print(f"Generating bindings for {crate} (skip native build)...")
        rc = run(["cargo", "build", "-p", crate, *profile_flag], cwd=str(repo_root), env=env, dry=args.dry_run)
        if rc != 0:
            return rc
        bindings = find_bindings(target_dir, args.profile, crate)
        if bindings is None or not bindings.exists():
            print(f"Generated bindings.rs not found for {crate} under {target_dir / args.profile / 'build'}", file=sys.stderr)
            return 3
        dest = crate_roots[crate] / "src" / "bindings_pregenerated.rs"
        header = (
            "// AUTOGENERATED: pregenerated bindings for docs.rs/offline builds\n"
            "// Note: inner attributes are intentionally omitted to avoid include-context errors.\n\n"
        )
        if not args.dry_run:
            content = bindings.read_text(encoding="utf-8", errors="ignore")
            dest.write_text(header + content, encoding="utf-8")
        print(f"Updated pregenerated bindings: {dest}")

    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

