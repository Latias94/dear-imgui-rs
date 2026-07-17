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

Native and WASM pregenerated bindings are regenerated together by the canonical
`xtask verify-bindings --update` command. No Cargo OUT_DIR discovery is used.

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
import subprocess
import sys
from pathlib import Path

import source_metadata


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


def binding_commands():
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
    parser.add_argument(
        "--profile",
        default="debug",
        choices=["debug", "release"],
        help="Compatibility option; canonical bindgen output is profile-independent",
    )
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
        help="Compatibility flag that skips canonical generation; a caller will run the xtask",
    )
    parser.add_argument(
        "--wasm-ext",
        default="",
        help=(
            "Compatibility filter checked against the extension WASM profiles now "
            "generated together by verify-bindings"
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
        try:
            metadata_update = source_metadata.update_core_source_metadata(
                repo_root, dry_run=args.dry_run
            )
        except source_metadata.SourceMetadataError as error:
            for message in error.errors:
                print(f"Source metadata error: {message}", file=sys.stderr)
            return 2
        if metadata_update.changed:
            action = "Would update" if args.dry_run else "Updated"
            print(
                f"{action} core source metadata: "
                f"cimgui={metadata_update.revisions['cimgui-revision']} "
                f"imgui={metadata_update.revisions['imgui-revision']}"
            )
        else:
            print("Core source metadata already matches clean submodule revisions")

    binding_specs = {
        spec.crate_name: spec for spec in source_metadata.BINDING_SOURCE_SPECS
    }
    for crate in crates:
        spec = binding_specs.get(crate)
        if spec is None:
            continue
        try:
            metadata_update = source_metadata.update_binding_source_metadata(
                repo_root, spec, dry_run=args.dry_run
            )
        except source_metadata.SourceMetadataError as error:
            for message in error.errors:
                print(f"Binding source metadata error: {message}", file=sys.stderr)
            return 2
        if metadata_update.changed:
            action = "Would update" if args.dry_run else "Updated"
            print(
                f"{action} {crate} binding source metadata: "
                f"{metadata_update.revisions['source-revision']}"
            )
        else:
            print(f"{crate} binding source metadata already matches its submodule")

    if not args.skip_core_bindings:
        print("Generating and validating all maintained binding profiles via xtask...")
        for command in binding_commands():
            rc = run(command, cwd=str(repo_root), dry=args.dry_run)
            if rc != 0:
                return rc

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

    # Kept for command-line compatibility until the release workflow cleanup.
    # The canonical verifier already regenerates every maintained WASM profile.
    wasm_exts = [e.strip() for e in args.wasm_ext.split(",") if e.strip()]
    if wasm_exts:
        supported_wasm_extensions = {
            "implot",
            "implot3d",
            "imnodes",
            "imguizmo",
            "imguizmo-quat",
        }
        unknown = sorted(set(wasm_exts) - supported_wasm_extensions)
        if unknown:
            print(
                f"Unknown wasm extensions: {unknown}. Expected only: "
                f"{sorted(supported_wasm_extensions)}",
                file=sys.stderr,
            )
            return 7
        print("Extension WASM bindings were generated by canonical verify-bindings")

    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
