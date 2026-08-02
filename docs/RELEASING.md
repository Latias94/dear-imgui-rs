# Releasing (sys crates with offline docs.rs)

> **Note**: For the current operator commands, see
> [`tools/README.md`](../tools/README.md). This document covers sys binding
> generation, provenance, and the release evidence contract.

The `-sys` crates must build docs on docs.rs without network, Git metadata, or submodules. Core Dear ImGui publishes three checked-in artifacts: `bindings_pregenerated_windows.rs`, `bindings_pregenerated.rs` for the supported non-Windows native whitelist, and `wasm_bindings_pregenerated.rs` for the fixed browser import ABI. Extension sys crates keep their native and optional WASM pregenerated files.

Supported crates:
- `dear-imgui-sys` (third-party: cimgui)
- `extensions/dear-implot-sys` (third-party: cimplot)
- `extensions/dear-imnodes-sys` (third-party: cimnodes)
- `extensions/dear-node-editor-sys` (third-party: cimnodes_editor / imgui-node-editor; native only)
- `extensions/dear-imguizmo-sys` (third-party: cimguizmo)
- `extensions/dear-implot3d-sys` (third-party: cimplot3d)
- `extensions/dear-imguizmo-quat-sys` (third-party: cimguizmo_quat)
- `extensions/dear-imgui-test-engine-sys` (third-party: imgui_test_engine; native only)

## Prerequisites
- `git`, `cargo`, and Python 3.11+ in PATH.
- A clean, committed release candidate. Local and remote release gates bind to
  the exact 40-hex `HEAD`; a branch name is not release evidence.
- If you want to update third-party code, allow the script to update submodules/branches.

## Binding update workflow

Script: `tools/update_submodule_and_bindings.py`

Key flags:
- `--crates`: comma-separated list or `all`.
- `--profile`: `debug` or `release` (affects target build dir only).
- `--submodules`: `update` (update all known submodules), `auto` (update only selected crates), `skip` (don’t touch submodules).
- `--wasm`: regenerate/verify the core WASM profile and compile-check the explicit provider feature.
- The WASM import module is fixed to `imgui-sys-v1`; it is not a command-line option.
- `--wasm-ext`: comma-separated WASM extension bindings (`implot,implot3d,imnodes,imguizmo,imguizmo-quat`).
- Per-submodule branches:
  - `--cimgui-branch` (default `docking_inter`)
  - `--cimplot-branch` (default `master`)
  - `--cimplot3d-branch` (default `main`)
  - `--cimnodes-branch` (default `master`)
  - `--cimnodes-editor-branch` (default `main`)
  - `--cimguizmo-branch` (default `master`)
  - `--imgui-test-engine-branch` (default `main`)

Examples
- dear-imgui-sys only (update submodule + pregenerate, Release):
```
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules auto \
  --cimgui-branch docking_inter \
  --profile release
```

- All known -sys crates (update all submodules + pregenerate, Release):
```
python3 tools/update_submodule_and_bindings.py \
  --crates all --submodules update --profile release \
  --cimgui-branch docking_inter \
  --cimplot-branch master \
  --cimplot3d-branch main \
  --cimnodes-branch master \
  --cimnodes-editor-branch main \
  --cimguizmo-branch master \
  --imgui-test-engine-branch main
```

- All known -sys crates plus current WASM pregenerated bindings, without moving submodules:
```
python3 tools/update_submodule_and_bindings.py \
  --crates all --submodules skip --profile release \
  --wasm \
  --wasm-ext implot,implot3d,imnodes,imguizmo,imguizmo-quat
```

- Regenerate pregenerated bindings only (no submodule changes):
```
python3 tools/update_submodule_and_bindings.py \
  --crates dear-implot-sys,dear-imnodes-sys \
  --submodules skip --profile debug
```

What the script does:

- Optionally updates selected submodules and synchronizes the exact cimgui/nested Dear ImGui revisions stored in `dear-imgui-sys/Cargo.toml` package metadata.
- Refuses staged, unstaged, or untracked changes in the source submodules before recording provenance.
- Routes core generation through the shared binding specification so Windows, non-Windows, and WASM artifacts are produced and compared as one contract.
- Runs extension bindgen with native compilation disabled, then writes the corresponding pregenerated files.
- Invokes `cargo run -p xtask -- verify-bindings --allow-dirty` after generation so the second pass reproduces rather than merely trusting the files just written.

Canonical core verification is also available directly:

```bash
# Reproduce and compare all three checked-in core profiles.
cargo run -p xtask -- verify-bindings

# Maintainer-only regeneration after an intentional source/specification change.
cargo run -p xtask -- verify-bindings --update --allow-dirty
```

Canonical generation rejects `BINDGEN_EXTRA_CLANG_ARGS*`; target facts, header shims, formatter, allow/block lists, enum normalization, opaque types, and provider name all participate in the deterministic specification hash.

## Source and prebuilt provenance

The exact 40-hex cimgui and nested Dear ImGui revisions are package metadata, not values discovered from Git during a consumer build. They survive `cargo package` and are available in an unpacked crate with no `.git` directory.

A `dear-imgui-sys` core native prebuilt is accepted only when its manifest exactly matches crate/version, target triple, static link type, MSVC CRT, normalized features, both source revisions, and binding-spec hash. The normal, freetype, stack-layout, and stack-layout + freetype combinations have different names and manifests. Missing, duplicate, unknown, or mismatched fields reject the core artifact.

The six safe extension crates (`dear-implot`, `dear-implot3d`, `dear-imnodes`,
`dear-imguizmo`, `dear-imguizmo-quat`, and `dear-node-editor`) forward
`prebuilt` and `build-from-source` through both core and extension sys crates.
Source wins when Cargo unifies both routes. Each extension prebuilt manifest
binds its source and binding identity to the exact core artifact and candidate
SHA. Test Engine is separate: its crate-local source/shim/generator provenance
is reproducible, but it is native source-only and never appears in a prebuilt
artifact profile.

## Release gates and evidence

A release requires both gates for the same commit:

1. `python3 tools/tasks.py release-check` succeeds from a clean, committed
   working tree.
2. `.github/workflows/release-gate.yml` returns `Go` for that exact full SHA.

The remote aggregate has a fixed 14-cell inventory: Linux Test Engine runtime,
real Winit/WGPU and SDL3/Glow viewport smokes, Linux WASM, Windows vcpkg,
Windows MSVC `/MD` and `/MT`, Windows GNU imports, macOS, and five prebuilt
producer/consumer targets (Linux x86_64, macOS x86_64/aarch64, and Windows MSVC
`/MD`/`/MT`). A missing, failed, skipped, cancelled, timed-out, malformed,
duplicate, or wrong-SHA cell is `No-Go`; the caller cannot narrow the production
inventory.

The run retains `gate-result.json`, stdout/stderr, runtime invocation/results,
Xvfb/Mesa display and renderer data, target/CRT/vcpkg/MinGW metadata, binding
hashes, manifests, candidate SHA, and SHA256 evidence for approximately 30
days. Verify a downloaded aggregate locally before publishing:

```bash
python3 tools/ci/release_evidence.py verify \
  --repo-root . \
  --candidate-sha CANDIDATE_SHA \
  --gate-result artifacts/release-gate/gate-result.json
```

Headless Test Engine success does not replace the real viewport cell. Missing
display or software-GPU infrastructure is a failed remote gate, not a skipped
success.

## Pre-publish checks
Verify all `-sys` crates have pregenerated bindings and build in docs mode locally:

Windows (PowerShell):
```
$env:DOCS_RS = '1'; cargo check -p dear-imgui-sys
$env:DOCS_RS = '1'; cargo check -p dear-implot-sys
$env:DOCS_RS = '1'; cargo check -p dear-imnodes-sys
$env:DOCS_RS = '1'; cargo check -p dear-node-editor-sys
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-sys
$env:DOCS_RS = '1'; cargo check -p dear-implot3d-sys
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-quat-sys
$env:DOCS_RS = '1'; cargo check -p dear-imgui-test-engine-sys
```

Linux/macOS:
```
DOCS_RS=1 cargo check -p dear-imgui-sys
DOCS_RS=1 cargo check -p dear-implot-sys
DOCS_RS=1 cargo check -p dear-imnodes-sys
DOCS_RS=1 cargo check -p dear-node-editor-sys
DOCS_RS=1 cargo check -p dear-imguizmo-sys
DOCS_RS=1 cargo check -p dear-implot3d-sys
DOCS_RS=1 cargo check -p dear-imguizmo-quat-sys
DOCS_RS=1 cargo check -p dear-imgui-test-engine-sys
```

These checks generate/use bindings only and won’t build/link native code.

## Recommended release workflow

> **Tip**: Preparation and validation are separate because preparation intentionally changes the tree:
> ```bash
> python3 tools/tasks.py release-prepare 0.16.0-alpha.1
> git diff
> git add -A && git commit -m "chore: prepare release v0.16.0-alpha.1"
> python3 tools/tasks.py release-check
> git rev-parse HEAD
> gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
> gh run download RELEASE_GATE_RUN_ID --name release-gate-FULL_40_HEX_SHA --dir artifacts/release-gate
> python3 tools/tasks.py publish --dry-run
> python3 tools/tasks.py publish --release-gate-result artifacts/release-gate/gate-result.json
> ```
> See [`tools/README.md`](../tools/README.md) for the corresponding operator commands.

Manual workflow:

1) Update the single workspace release version with `cargo run -p xtask -- release-version 0.16.0-alpha.1 --allow-prerelease-relabel`.
2) Run the update script to pregenerate bindings and synchronize source metadata.
3) Review and commit the source pointers, metadata, pregenerated files, versions, lockfile, changelog, and docs:
```
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.1"
```
4) Run `python3 tools/tasks.py release-check` from the clean committed tree.
5) Dispatch `.github/workflows/release-gate.yml` with its sole required input,
the full output of `git rev-parse HEAD`:
```
gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
gh run download RELEASE_GATE_RUN_ID \
  --name release-gate-FULL_40_HEX_SHA \
  --dir artifacts/release-gate
```
Wait for all 14 cells and require `Go`; the downloaded artifact contains the
authoritative `gate-result.json`.
6) Publish the complete 27-package train through the shared authoritative order:
```
python3 tools/publish.py --dry-run
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json
```
The script verifies that the aggregate is a complete same-SHA `Go`, reruns the
strict local preflight, targets `crates-io` explicitly, and rechecks the clean
source fingerprint before every upload. The complete manual order is
documented in [PUBLISHING.md](./PUBLISHING.md); do not publish only the sys
crates and leave the release train incomplete.
7) After every package succeeds, create and push the release tag:
```
git tag -a v0.16.0-alpha.1 -m "Release v0.16.0-alpha.1"
git push origin v0.16.0-alpha.1
```
8) Dispatch `.github/workflows/release.yml` with its three required inputs:
```
gh workflow run release.yml \
  -f tag=v0.16.0-alpha.1 \
  -f candidate_sha=FULL_40_HEX_SHA \
  -f gate_run_id=RELEASE_GATE_RUN_ID
```
It verifies the stored aggregate and recomputes it from the retained cells
before creating the GitHub Release or uploading its prebuilt archives.

## Pre-release checklist

Before tagging and publishing, verify the following:

- Root `workspace.package.version` is correct, all 27 publishable manifests inherit it, internal dependencies inherit root workspace declarations, and `CHANGELOG.md` is updated.
- The changelog keeps `## [Unreleased]` as its first release section, its prose is soft-wrapped, and the current release notes can be extracted with `python3 tools/changelog.py extract --version <version>`.
- Compatibility docs are in sync:
  - Root `README.md` “Compatibility Candidate (0.16.0-alpha.1)” table updated.
  - `docs/COMPATIBILITY.md` updated with the new release train and notes.
- `docs.rs` offline builds validated locally for all `-sys` crates (see Pre-publish checks above).
- All three core binding profiles reproduce exactly with `cargo run -p xtask -- verify-bindings`.
- The packaged sys crate contains source metadata and all three profiles, and an unpacked offline build succeeds without `.git`.
- CI green on Linux/Windows/macOS; examples build with extensions enabled.
- `release-check` passed on the clean committed candidate and the remote fixed
  14-cell aggregate is `Go` for exactly the same SHA.
- If external deps changed (e.g., `wgpu`, `winit`, `glow`), backends’ readmes compatibility tables updated.
- If interfaces changed, examples and crate-level docs updated accordingly.
- The five prebuilt cells were produced and consumed inside the authoritative
  release gate; prebuilts are not an optional post-release side workflow.
- `.github/workflows/release.yml` was given the matching tag, candidate SHA,
  and gate run, and refused to create a release before verifying them.
- Ensure the publishing environment has access to a valid crates.io token (`cargo login` or `CARGO_REGISTRY_TOKEN`) before running the Python publishing scripts.

## Notes
- Docking is available in the core build. Multi-viewport remains an explicit feature because platform and renderer callback lifecycles must be selected together.
- docs.rs offline builds rely solely on checked-in target-appropriate bindings; source builds still require submodules or an exact matching prebuilt artifact.
- The node-editor blueprints stack layout is a separate native artifact profile and cannot be substituted for the normal core or WASM profile.
- If you need extra docs.rs cfgs later, extend each `-sys` crate’s `DOCS_RS` path in its `build.rs`.
