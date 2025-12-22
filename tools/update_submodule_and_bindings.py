#!/usr/bin/env python3
"""
Update third-party submodules and refresh pregenerated bindings for sys crates.

Why: docs.rs builds are offline and cannot fetch submodules. To guarantee
successful docs.rs builds, we pre-generate Rust bindings and vendor headers
via submodules locally before publishing.

Supported crates (native bindings):
  - dear-imgui-sys (cimgui)
  - extensions/dear-implot-sys (cimplot)
  - extensions/dear-implot3d-sys (cimplot3d)
  - extensions/dear-imnodes-sys (cimnodes)
  - extensions/dear-imguizmo-sys (cimguizmo)
  - extensions/dear-imguizmo-quat-sys (cimguizmo_quat)

WASM pregenerated bindings:
  - dear-imgui-sys: via `xtask wasm-bindgen`
  - optional extensions via `--wasm-ext`:
    - dear-implot-sys: `xtask wasm-bindgen-implot`
    - dear-implot3d-sys: `xtask wasm-bindgen-implot3d`
    - dear-imnodes-sys: `xtask wasm-bindgen-imnodes`
    - dear-imguizmo-sys: `xtask wasm-bindgen-imguizmo`
    - dear-imguizmo-quat-sys: `xtask wasm-bindgen-imguizmo-quat`

Usage examples:
  - Update cimgui and regenerate bindings for dear-imgui-sys (Debug):
      python3 tools/update_submodule_and_bindings.py --crates dear-imgui-sys \
        --submodules update

  - Update all submodules to specific branches and pregen bindings (Release):
      python3 tools/update_submodule_and_bindings.py --crates all --profile release \
        --submodules update \
        --cimgui-branch docking_inter --cimplot-branch master \
        --cimnodes-branch master --cimguizmo-branch master

  - Only regenerate pregenerated bindings without touching submodules:
      python3 tools/update_submodule_and_bindings.py --crates dear-implot-sys,dear-imnodes-sys \
        --submodules skip

  - Dry-run (print commands only):
      python3 tools/update_submodule_and_bindings.py --crates all --dry-run

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
    parser = argparse.ArgumentParser(description="Update third-party submodules and pregenerate bindings for sys crates (incl. wasm)")
    parser.add_argument("--crates", default="dear-imgui-sys", help="Comma-separated list of crates to process (or 'all')")
    parser.add_argument("--profile", default="debug", choices=["debug", "release"], help="Cargo profile when generating bindings")
    parser.add_argument("--submodules", default="auto", choices=["auto", "update", "skip"], help="Whether to update submodules: auto=update only for selected crates; update=update all known submodules; skip=don't touch submodules")
    # Branch selection per submodule
    parser.add_argument("--cimgui-branch", default="docking_inter", help="Branch for cimgui submodule (dear-imgui-sys)")
    parser.add_argument("--cimplot-branch", default="master", help="Branch for cimplot submodule (dear-implot-sys)")
    parser.add_argument("--cimplot3d-branch", default="main", help="Branch for cimplot3d submodule (dear-implot3d-sys)")
    parser.add_argument("--cimnodes-branch", default="master", help="Branch for cimnodes submodule (dear-imnodes-sys)")
    parser.add_argument("--cimguizmo-branch", default="master", help="Branch for cimguizmo submodule (dear-imguizmo-sys)")
    parser.add_argument("--remote", default="origin", help="Remote name for submodules")
    parser.add_argument("--wasm", action="store_true", help="Additionally generate wasm pregenerated bindings for dear-imgui-sys")
    parser.add_argument("--wasm-import", default="imgui-sys-v0", help="WASM import module name for generated bindings")
    parser.add_argument(
        "--wasm-ext",
        default="",
        help=(
            "Comma-separated list of extension wasm bindings to pregenerate via xtask "
            "(choices: implot,implot3d,imnodes,imguizmo,imguizmo-quat)"
        ),
    )
    parser.add_argument("--dry-run", action="store_true", help="Print commands without executing")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    # Known crate roots and submodules
    crate_roots = {
        "dear-imgui-sys": repo_root / "dear-imgui-sys",
        "dear-implot-sys": repo_root / "extensions/dear-implot-sys",
        "dear-implot3d-sys": repo_root / "extensions/dear-implot3d-sys",
        "dear-imnodes-sys": repo_root / "extensions/dear-imnodes-sys",
        "dear-imguizmo-sys": repo_root / "extensions/dear-imguizmo-sys",
        "dear-imguizmo-quat-sys": repo_root / "extensions/dear-imguizmo-quat-sys",
    }
    submodules = {
        "dear-imgui-sys": (crate_roots["dear-imgui-sys"] / "third-party/cimgui", args.cimgui_branch),
        "dear-implot-sys": (crate_roots["dear-implot-sys"] / "third-party/cimplot", args.cimplot_branch),
        "dear-implot3d-sys": (crate_roots["dear-implot3d-sys"] / "third-party/cimplot3d", args.cimplot3d_branch),
        "dear-imnodes-sys": (crate_roots["dear-imnodes-sys"] / "third-party/cimnodes", args.cimnodes_branch),
        "dear-imguizmo-sys": (crate_roots["dear-imguizmo-sys"] / "third-party/cimguizmo", args.cimguizmo_branch),
        "dear-imguizmo-quat-sys": (crate_roots["dear-imguizmo-quat-sys"] / "third-party/cimguizmo_quat", args.cimguizmo_branch),
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
        "dear-implot3d-sys": "IMPLOT3D_SYS_SKIP_CC",
        "dear-imnodes-sys": "IMNODES_SYS_SKIP_CC",
        "dear-imguizmo-sys": "IMGUIZMO_SYS_SKIP_CC",
        "dear-imguizmo-quat-sys": "IMGUIZMO_QUAT_SYS_SKIP_CC",
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

    # Optionally generate wasm pregenerated bindings for dear-imgui-sys
    if args.wasm:
        xtask = repo_root / "xtask"
        if not xtask.exists():
            print("xtask workspace member not found; cannot generate wasm bindings", file=sys.stderr)
            return 4
        print(f"Generating wasm pregenerated bindings (import='{args.wasm_import}') via xtask...")
        rc = run(["cargo", "run", "-p", "xtask", "--", "wasm-bindgen", args.wasm_import], cwd=str(repo_root), dry=args.dry_run)
        if rc != 0:
            return rc
        wasm_preg = crate_roots["dear-imgui-sys"] / "src" / "wasm_bindings_pregenerated.rs"
        if not wasm_preg.exists():
            print(f"WASM pregenerated bindings not found: {wasm_preg}", file=sys.stderr)
            return 5
        print(f"WASM pregenerated bindings ready: {wasm_preg}")

        # Quick compile-check under wasm target (skip native C/C++)
        print("Running cargo check for wasm32-unknown-unknown (skip native cc)...")
        env = os.environ.copy()
        env["IMGUI_SYS_SKIP_CC"] = "1"
        rc = run([
            "cargo", "check", "-p", "dear-imgui", "-F", "wasm", "--target", "wasm32-unknown-unknown"
        ], cwd=str(repo_root), env=env, dry=args.dry_run)
        if rc != 0:
            return rc

    # Optionally generate wasm pregenerated bindings for extension -sys crates
    wasm_exts = [
        e.strip() for e in args.wasm_ext.split(",") if e.strip()
    ]
    if wasm_exts:
        xtask = repo_root / "xtask"
        if not xtask.exists():
            print("xtask workspace member not found; cannot generate extension wasm bindings", file=sys.stderr)
            return 6

        # Map short extension names to xtask subcommands and -sys crate ids
        ext_to_cmd = {
            "implot": "wasm-bindgen-implot",
            "implot3d": "wasm-bindgen-implot3d",
            "imnodes": "wasm-bindgen-imnodes",
            "imguizmo": "wasm-bindgen-imguizmo",
            "imguizmo-quat": "wasm-bindgen-imguizmo-quat",
        }
        ext_to_sys_crate = {
            "implot": "dear-implot-sys",
            "implot3d": "dear-implot3d-sys",
            "imnodes": "dear-imnodes-sys",
            "imguizmo": "dear-imguizmo-sys",
            "imguizmo-quat": "dear-imguizmo-quat-sys",
        }

        for ext in wasm_exts:
            if ext not in ext_to_cmd:
                print(f"Unknown wasm extension '{ext}'. Expected one of: {', '.join(sorted(ext_to_cmd.keys()))}", file=sys.stderr)
                return 7
            cmd = ext_to_cmd[ext]
            sys_crate = ext_to_sys_crate[ext]
            print(f"Generating wasm pregenerated bindings for {sys_crate} (ext='{ext}', import='{args.wasm_import}') via xtask...")
            rc = run(
                ["cargo", "run", "-p", "xtask", "--", cmd, args.wasm_import],
                cwd=str(repo_root),
                dry=args.dry_run,
            )
            if rc != 0:
                return rc
            wasm_preg = crate_roots[sys_crate] / "src" / "wasm_bindings_pregenerated.rs"
            if not wasm_preg.exists():
                print(f"WASM pregenerated bindings not found for {sys_crate}: {wasm_preg}", file=sys.stderr)
                return 8
            print(f"WASM pregenerated bindings ready for {sys_crate}: {wasm_preg}")

    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
