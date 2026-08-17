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
  - extensions/dear-imgui-cte-sys (cimCTE / ImGuiColorTextEdit)
  - extensions/dear-imgui-test-engine-sys (imgui_test_engine)

Native and WASM pregenerated bindings are regenerated together by the canonical
`xtask verify-bindings --update` command. No Cargo OUT_DIR discovery is used.

Usage examples:
  - Update cimgui and regenerate bindings for dear-imgui-sys:
      python3 tools/update_submodule_and_bindings.py --crates dear-imgui-sys \
        --submodules auto

  - Update all submodules to specific branches and regenerate bindings:
      python3 tools/update_submodule_and_bindings.py --crates all \
        --submodules update \
        --cimgui-branch docking_inter --cimplot-branch master \
        --cimplot3d-branch main \
        --cimnodes-branch master --cimguizmo-branch master \
        --cimguizmo-quat-branch master \
        --cimnodes-editor-branch main --cte-branch main_goossens \
        --imgui-test-engine-branch main

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


def binding_command():
    return [
        "cargo",
        "run",
        "-p",
        "xtask",
        "--",
        "verify-bindings",
        "--update",
        "--allow-dirty",
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description="Update third-party submodules and pregenerate bindings for sys crates (incl. wasm)")
    parser.add_argument("--crates", default="dear-imgui-sys", help="Comma-separated list of crates to process (or 'all')")
    parser.add_argument("--submodules", default="auto", choices=["auto", "update", "skip"], help="Whether to update submodules: auto=update only for selected crates; update=update all known submodules; skip=don't touch submodules")
    # Branch selection per submodule
    parser.add_argument("--cimgui-branch", default="docking_inter", help="Branch for cimgui submodule (dear-imgui-sys)")
    parser.add_argument("--cimplot-branch", default="master", help="Branch for cimplot submodule (dear-implot-sys)")
    parser.add_argument("--cimplot3d-branch", default="main", help="Branch for cimplot3d submodule (dear-implot3d-sys)")
    parser.add_argument("--cimnodes-branch", default="master", help="Branch for cimnodes submodule (dear-imnodes-sys)")
    parser.add_argument("--cimnodes-editor-branch", default="main", help="Branch for cimnodes_editor submodule (dear-node-editor-sys)")
    parser.add_argument("--cimguizmo-branch", default="master", help="Branch for cimguizmo submodule (dear-imguizmo-sys)")
    parser.add_argument(
        "--cimguizmo-quat-branch",
        default="master",
        help="Branch for cimguizmo_quat submodule (dear-imguizmo-quat-sys)",
    )
    parser.add_argument(
        "--imgui-test-engine-branch",
        default="main",
        help="Branch for imgui_test_engine submodule (dear-imgui-test-engine-sys)",
    )
    parser.add_argument(
        "--cte-branch",
        default="main_goossens",
        help="Branch for cimCTE submodule (dear-imgui-cte-sys)",
    )
    parser.add_argument("--remote", default="origin", help="Remote name for submodules")
    parser.add_argument("--wasm", action="store_true", help="Additionally generate wasm pregenerated bindings for dear-imgui-sys")
    parser.add_argument("--dry-run", action="store_true", help="Print commands without executing")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    branches = {
        "core": args.cimgui_branch,
        "test-engine": args.imgui_test_engine_branch,
        "implot": args.cimplot_branch,
        "implot3d": args.cimplot3d_branch,
        "imnodes": args.cimnodes_branch,
        "cte": args.cte_branch,
        "node-editor": args.cimnodes_editor_branch,
        "imguizmo": args.cimguizmo_branch,
        "imguizmo-quat": args.cimguizmo_quat_branch,
    }
    submodules = {
        source.crate_name: (
            repo_root
            / Path(source.crate_root.as_posix())
            / Path(source.source_root.as_posix()),
            branches[source.id],
        )
        for source in source_metadata.SOURCE_INVENTORY.sources
    }
    crate_roots = {
        source.crate_name: repo_root / Path(source.crate_root.as_posix())
        for source in source_metadata.SOURCE_INVENTORY.sources
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
            rendered_revisions = " ".join(
                f"{key}={value}"
                for key, value in metadata_update.revisions.items()
            )
            print(
                f"{action} {crate} binding source metadata: "
                f"{rendered_revisions}"
            )
        else:
            print(f"{crate} binding source metadata already matches its submodule")

    print("Generating and validating all maintained binding profiles via xtask...")
    rc = run(binding_command(), cwd=str(repo_root), dry=args.dry_run)
    if rc != 0:
        return rc

    # Optionally compile-check the explicit core WASM provider contract.
    if args.wasm:
        wasm_preg = crate_roots["dear-imgui-sys"] / "src" / "wasm_bindings_pregenerated.rs"
        if not args.dry_run and not wasm_preg.exists():
            print(f"WASM pregenerated bindings not found: {wasm_preg}", file=sys.stderr)
            return 5
        state = "would be generated at" if args.dry_run else "ready"
        print(f"WASM pregenerated bindings {state}: {wasm_preg}")

        print("Running cargo check for the explicit wasm32 provider feature...")
        rc = run([
            "cargo", "check", "-p", "dear-imgui-rs", "-F", "wasm", "--target", "wasm32-unknown-unknown"
        ], cwd=str(repo_root), dry=args.dry_run)
        if rc != 0:
            return rc

    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
