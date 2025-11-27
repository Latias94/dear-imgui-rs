# Releasing (sys crates with offline docs.rs)

> **Note**: For a complete publishing guide including automated scripts, see [PUBLISHING.md](./PUBLISHING.md).
> This document focuses on the technical details of sys crate bindings generation.

The `-sys` crates must build docs on docs.rs without network or submodules. Before publishing, pre-generate `src/bindings_pregenerated.rs` so docs.rs can compile in a fully offline environment.

Supported crates:
- `dear-imgui-sys` (third-party: cimgui)
- `extensions/dear-implot-sys` (third-party: cimplot)
- `extensions/dear-imnodes-sys` (third-party: cimnodes)
- `extensions/dear-imguizmo-sys` (third-party: cimguizmo)
- `extensions/dear-implot3d-sys` (third-party: cimplot3d)
- `extensions/dear-imguizmo-quat-sys` (third-party: cimguizmo_quat)

## Prerequisites
- `git`, `cargo`, and `python3` (>= 3.7) in PATH.
- Clean working tree (or use a temp branch).
- If you want to update third-party code, allow the script to update submodules/branches.

## Script (generate pregenerated bindings + optional submodule update)

Script: `tools/update_submodule_and_bindings.py`

Key flags:
- `--crates`: comma-separated list or `all`.
- `--profile`: `debug` or `release` (affects target build dir only).
- `--submodules`: `update` (update all known submodules), `auto` (update only selected crates), `skip` (don’t touch submodules).
- Per-submodule branches:
  - `--cimgui-branch` (default `docking_inter`)
  - `--cimplot-branch` (default `master`)
  - `--cimnodes-branch` (default `master`)
  - `--cimguizmo-branch` (default `master`)

Examples
- dear-imgui-sys only (update submodule + pregenerate, Release):
```
python tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules update \
  --cimgui-branch docking_inter \
  --profile release
```

- All 4 -sys crates (update all submodules + pregenerate, Release):
```
python tools/update_submodule_and_bindings.py \
  --crates all --submodules update --profile release \
  --cimgui-branch docking_inter \
  --cimplot-branch master \
  --cimnodes-branch master \
  --cimguizmo-branch master
```

- Regenerate pregenerated bindings only (no submodule changes):
```
python tools/update_submodule_and_bindings.py \
  --crates dear-implot-sys,dear-imnodes-sys \
  --submodules skip --profile debug
```

What the script does
- Optionally updates chosen submodules (`git fetch/checkout/pull` + `submodule update --init --recursive`).
- For each crate, runs `cargo build -p <crate>` with `*_SYS_SKIP_CC=1` to skip native builds and only emit bindgen output.
- Copies `target/<profile>/build/<crate>-*/out/bindings.rs` into `<crate>/src/bindings_pregenerated.rs` (adds a comment header only).

## Pre-publish checks
Verify the 4 `-sys` crates have pregenerated bindings and build in docs mode locally:

Windows (PowerShell):
```
$env:DOCS_RS = '1'; cargo check -p dear-imgui-sys
$env:DOCS_RS = '1'; cargo check -p dear-implot-sys
$env:DOCS_RS = '1'; cargo check -p dear-imnodes-sys
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-sys
```

Linux/macOS:
```
DOCS_RS=1 cargo check -p dear-imgui-sys
DOCS_RS=1 cargo check -p dear-implot-sys
DOCS_RS=1 cargo check -p dear-imnodes-sys
DOCS_RS=1 cargo check -p dear-imguizmo-sys
```

These checks generate/use bindings only and won’t build/link native code.

## Recommended publish order

> **Tip**: Use the automated publishing script for easier workflow:
> ```bash
> python tools/tasks.py release-prep 0.6.0  # Prepare release
> python tools/tasks.py publish             # Publish all crates
> ```
> See [PUBLISHING.md](./PUBLISHING.md) for details.

Manual workflow:

1) Run the script to pregenerate bindings (and update submodules if needed).
2) Commit changes (includes submodule pointers and pregenerated files):
```
git add -A
git commit -m "chore(sys): update third-party and pregenerated bindings"
```
3) Tag and publish crates (or use `python tools/publish.py`):
```
cargo publish -p dear-imgui-sys
cargo publish -p dear-implot-sys
cargo publish -p dear-imnodes-sys
cargo publish -p dear-imguizmo-sys
cargo publish -p dear-implot3d-sys
cargo publish -p dear-imguizmo-quat-sys
```

## Pre-release checklist

Before tagging and publishing, verify the following:

- Versions bumped correctly in all `Cargo.toml` (workspace and crates), and `CHANGELOG.md` updated.
- Compatibility docs are in sync:
  - Root `README.md` “Compatibility (Latest)” table updated.
  - `docs/COMPATIBILITY.md` updated with the new release train and notes.
- `docs.rs` offline builds validated locally for all `-sys` crates (see Pre-publish checks above).
- CI green on Linux/Windows/macOS; examples build with extensions enabled.
- If external deps changed (e.g., `wgpu`, `winit`, `glow`), backends’ readmes compatibility tables updated.
- If interfaces changed, examples and crate-level docs updated accordingly.
- Optional: Run `.github/workflows/prebuilt-binaries.yml` (workflow_dispatch) to produce prebuilt archives for the new tag.
- Ensure GitHub secrets are set for automated release (e.g., `CARGO_REGISTRY_TOKEN` for release-plz).

## Notes
- Docking is always enabled; the `multi-viewport` feature is currently commented out (WIP).
- docs.rs offline builds rely solely on `bindings_pregenerated.rs` (no submodules or network). Source builds still require submodules or prebuilt artifacts.
- If you need extra docs.rs cfgs later, extend each `-sys` crate’s `DOCS_RS` path in its `build.rs`.
