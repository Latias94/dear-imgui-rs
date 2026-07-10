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
  - extensions/dear-node-editor-sys (cimnodes_editor / imgui-node-editor)
  - extensions/dear-imguizmo-sys (cimguizmo)
  - extensions/dear-imguizmo-quat-sys (cimguizmo_quat)
  - extensions/dear-imgui-test-engine-sys (imgui_test_engine)

WASM pregenerated bindings:
  - dear-imgui-sys: regenerated with the native profiles via `xtask verify-bindings --update`
  - optional extensions via `--wasm-ext`:
    - dear-implot-sys: `xtask wasm-bindgen-implot`
    - dear-implot3d-sys: `xtask wasm-bindgen-implot3d`
    - dear-imnodes-sys: `xtask wasm-bindgen-imnodes`
    - dear-imguizmo-sys: `xtask wasm-bindgen-imguizmo`
    - dear-imguizmo-quat-sys: `xtask wasm-bindgen-imguizmo-quat`

Usage examples:
  - Update cimgui and regenerate bindings for dear-imgui-sys (Debug):
      python3 tools/update_submodule_and_bindings.py --crates dear-imgui-sys \
        --submodules auto

  - Update all submodules to specific branches and pregen bindings (Release):
      python3 tools/update_submodule_and_bindings.py --crates all --profile release \
        --submodules update \
        --cimgui-branch docking_inter --cimplot-branch master \
        --cimplot3d-branch main \
        --cimnodes-branch master --cimguizmo-branch master \
        --cimnodes-editor-branch main --imgui-test-engine-branch main

  - Only regenerate pregenerated bindings without touching submodules:
      python3 tools/update_submodule_and_bindings.py --crates dear-implot-sys,dear-imnodes-sys \
        --submodules skip

  - Dry-run (print commands only):
      python3 tools/update_submodule_and_bindings.py --crates all --dry-run

Requirements:
  - git, cargo in PATH
  - Python 3.11+
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


SOURCE_METADATA_SECTION = "package.metadata.dear-imgui-sources"
SOURCE_METADATA_KEYS = {"cimgui-revision", "imgui-revision"}
GIT_REVISION_RE = re.compile(r"^[0-9a-fA-F]{40}$")


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
    matches = list(build_dir.glob(pattern))
    if not matches:
        return None
    # When multiple build outputs exist (incremental builds, feature changes),
    # prefer the most recently modified bindings.
    matches.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return matches[0]


def read_core_source_metadata(manifest_path: Path):
    data = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    try:
        metadata = data["package"]["metadata"]["dear-imgui-sources"]
    except (KeyError, TypeError) as error:
        raise RuntimeError(f"missing [{SOURCE_METADATA_SECTION}] in {manifest_path}") from error
    if set(metadata) != SOURCE_METADATA_KEYS:
        raise RuntimeError(
            f"[{SOURCE_METADATA_SECTION}] must contain exactly "
            f"{sorted(SOURCE_METADATA_KEYS)}, found {sorted(metadata)}"
        )
    for key, value in metadata.items():
        if not isinstance(value, str) or not GIT_REVISION_RE.fullmatch(value):
            raise RuntimeError(f"{key} must be a 40-character hexadecimal git revision")
    return metadata


def git_revision(path: Path) -> str:
    revision = subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()
    if not GIT_REVISION_RE.fullmatch(revision):
        raise RuntimeError(f"invalid git revision from {path}: {revision!r}")
    return revision


def require_clean_git_tree(path: Path):
    status = subprocess.check_output(
        [
            "git",
            "-C",
            str(path),
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        text=True,
    )
    if status:
        raise RuntimeError(f"source tree is dirty: {path}\n{status.rstrip()}")


def sync_core_source_metadata(manifest_path: Path, cimgui_path: Path, dry: bool):
    imgui_path = cimgui_path / "imgui"
    require_clean_git_tree(cimgui_path)
    require_clean_git_tree(imgui_path)
    revisions = {
        "cimgui-revision": git_revision(cimgui_path),
        "imgui-revision": git_revision(imgui_path),
    }
    current = read_core_source_metadata(manifest_path)
    if current == revisions:
        print("Core source metadata already matches clean submodule revisions")
        return

    print(
        "Updating core source metadata: "
        f"cimgui={revisions['cimgui-revision']} imgui={revisions['imgui-revision']}"
    )
    if dry:
        return

    lines = manifest_path.read_text(encoding="utf-8").splitlines(keepends=True)
    section_header = f"[{SOURCE_METADATA_SECTION}]"
    try:
        section_start = next(
            index for index, line in enumerate(lines) if line.strip() == section_header
        )
    except StopIteration as error:
        raise RuntimeError(f"missing {section_header} in {manifest_path}") from error
    section_end = next(
        (
            index
            for index in range(section_start + 1, len(lines))
            if lines[index].lstrip().startswith("[")
        ),
        len(lines),
    )
    found = set()
    for index in range(section_start + 1, section_end):
        match = re.match(r"^(\s*)([A-Za-z0-9_-]+)(\s*=).*$", lines[index])
        if match is None or match.group(2) not in revisions:
            continue
        key = match.group(2)
        newline = "\n" if lines[index].endswith("\n") else ""
        lines[index] = f'{match.group(1)}{key}{match.group(3)} "{revisions[key]}"{newline}'
        found.add(key)
    if found != SOURCE_METADATA_KEYS:
        raise RuntimeError(
            f"could not update all source metadata keys in {manifest_path}: found {sorted(found)}"
        )
    manifest_path.write_text("".join(lines), encoding="utf-8")
    if read_core_source_metadata(manifest_path) != revisions:
        raise RuntimeError("source metadata update did not round-trip through TOML parsing")


def core_binding_commands():
    return [
        [
            "cargo",
            "run",
            "-p",
            "xtask",
            "--",
            "verify-bindings",
            "--update",
            "--allow-dirty",
        ],
        [
            "cargo",
            "run",
            "-p",
            "xtask",
            "--",
            "verify-bindings",
            "--allow-dirty",
        ],
    ]


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
    parser.add_argument("--cimnodes-editor-branch", default="main", help="Branch for cimnodes_editor submodule (dear-node-editor-sys)")
    parser.add_argument("--cimguizmo-branch", default="master", help="Branch for cimguizmo submodule (dear-imguizmo-sys)")
    parser.add_argument(
        "--imgui-test-engine-branch",
        default="main",
        help="Branch for imgui_test_engine submodule (dear-imgui-test-engine-sys)",
    )
    parser.add_argument("--remote", default="origin", help="Remote name for submodules")
    parser.add_argument("--wasm", action="store_true", help="Additionally generate wasm pregenerated bindings for dear-imgui-sys")
    parser.add_argument(
        "--skip-core-bindings",
        action="store_true",
        help="Skip core generation after source/metadata sync; a caller will run the core xtask",
    )
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
        "dear-node-editor-sys": repo_root / "extensions/dear-node-editor-sys",
        "dear-imguizmo-sys": repo_root / "extensions/dear-imguizmo-sys",
        "dear-imguizmo-quat-sys": repo_root / "extensions/dear-imguizmo-quat-sys",
        "dear-imgui-test-engine-sys": repo_root / "extensions/dear-imgui-test-engine-sys",
    }
    submodules = {
        "dear-imgui-sys": (crate_roots["dear-imgui-sys"] / "third-party/cimgui", args.cimgui_branch),
        "dear-implot-sys": (crate_roots["dear-implot-sys"] / "third-party/cimplot", args.cimplot_branch),
        "dear-implot3d-sys": (crate_roots["dear-implot3d-sys"] / "third-party/cimplot3d", args.cimplot3d_branch),
        "dear-imnodes-sys": (crate_roots["dear-imnodes-sys"] / "third-party/cimnodes", args.cimnodes_branch),
        "dear-node-editor-sys": (
            crate_roots["dear-node-editor-sys"] / "third-party/cimnodes_editor",
            args.cimnodes_editor_branch,
        ),
        "dear-imguizmo-sys": (crate_roots["dear-imguizmo-sys"] / "third-party/cimguizmo", args.cimguizmo_branch),
        "dear-imguizmo-quat-sys": (crate_roots["dear-imguizmo-quat-sys"] / "third-party/cimguizmo_quat", args.cimguizmo_branch),
        "dear-imgui-test-engine-sys": (
            crate_roots["dear-imgui-test-engine-sys"] / "third-party/imgui_test_engine",
            args.imgui_test_engine_branch,
        ),
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
    core_requested = "dear-imgui-sys" in crates or args.wasm or args.submodules == "update"

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
            rc = run(
                ["git", "-C", str(path), "submodule", "update", "--init", "--recursive"],
                dry=args.dry_run,
            )
            if rc != 0:
                return rc

    if core_requested:
        sync_core_source_metadata(
            crate_roots["dear-imgui-sys"] / "Cargo.toml",
            submodules["dear-imgui-sys"][0],
            args.dry_run,
        )

    if core_requested and not args.skip_core_bindings:
        print("Generating and validating all core native/WASM binding profiles via xtask...")
        for command in core_binding_commands():
            rc = run(command, cwd=str(repo_root), dry=args.dry_run)
            if rc != 0:
                return rc

    # Generate pregenerated bindings for selected crates
    env_base = os.environ.copy()
    # Force build.rs to run bindgen instead of re-copying existing pregenerated files.
    env_base["DEAR_IMGUI_RS_REGEN_BINDINGS"] = "1"
    profile_flag = ["--release"] if args.profile == "release" else []
    crate_skip_env = {
        "dear-imgui-sys": "IMGUI_SYS_SKIP_CC",
        "dear-implot-sys": "IMPLOT_SYS_SKIP_CC",
        "dear-implot3d-sys": "IMPLOT3D_SYS_SKIP_CC",
        "dear-imnodes-sys": "IMNODES_SYS_SKIP_CC",
        "dear-node-editor-sys": "NODE_EDITOR_SYS_SKIP_CC",
        "dear-imguizmo-sys": "IMGUIZMO_SYS_SKIP_CC",
        "dear-imguizmo-quat-sys": "IMGUIZMO_QUAT_SYS_SKIP_CC",
        "dear-imgui-test-engine-sys": "IMGUI_TEST_ENGINE_SYS_SKIP_CC",
    }
    target_dir = Path(env_base.get("CARGO_TARGET_DIR", repo_root / "target"))
    for crate in crates:
        if crate == "dear-imgui-sys":
            continue
        env = env_base.copy()
        if crate != "dear-imgui-test-engine-sys":
            env[crate_skip_env[crate]] = "1"
            print(f"Generating bindings for {crate} (skip native build)...")
        else:
            # dear-imgui-test-engine-sys can regenerate bindings directly via
            # DEAR_IMGUI_RS_REGEN_BINDINGS, but its build.rs intentionally
            # rejects the SKIP_CC path when regeneration is requested.
            print(f"Generating bindings for {crate} (regen-only build)...")
        rc = run(
            ["cargo", "build", "-p", crate, "--features", "bindgen", *profile_flag],
            cwd=str(repo_root),
            env=env,
            dry=args.dry_run,
        )
        if rc != 0:
            return rc
        dest = crate_roots[crate] / "src" / "bindings_pregenerated.rs"
        if args.dry_run:
            print(f"Would update pregenerated bindings: {dest}")
            continue
        bindings = find_bindings(target_dir, args.profile, crate)
        if bindings is None or not bindings.exists():
            print(f"Generated bindings.rs not found for {crate} under {target_dir / args.profile / 'build'}", file=sys.stderr)
            return 3
        header = (
            "// AUTOGENERATED: pregenerated bindings for docs.rs/offline builds\n"
            "// Note: inner attributes are intentionally omitted to avoid include-context errors.\n\n"
        )
        content = bindings.read_text(encoding="utf-8", errors="ignore")
        dest.write_text(header + content, encoding="utf-8")
        print(f"Updated pregenerated bindings: {dest}")

    # Optionally compile-check the explicit core WASM provider contract.
    if args.wasm:
        wasm_preg = crate_roots["dear-imgui-sys"] / "src" / "wasm_bindings_pregenerated.rs"
        if not wasm_preg.exists():
            print(f"WASM pregenerated bindings not found: {wasm_preg}", file=sys.stderr)
            return 5
        print(f"WASM pregenerated bindings ready: {wasm_preg}")

        print("Running cargo check for the explicit wasm32 provider feature...")
        rc = run([
            "cargo", "check", "-p", "dear-imgui-rs", "-F", "wasm", "--target", "wasm32-unknown-unknown"
        ], cwd=str(repo_root), dry=args.dry_run)
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
            print(
                f"Generating wasm pregenerated bindings for {sys_crate} "
                f"(ext='{ext}', provider='imgui-sys-v0') via xtask..."
            )
            rc = run(
                ["cargo", "run", "-p", "xtask", "--", cmd],
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
