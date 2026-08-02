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

A release requires `python3 tools/tasks.py release-check` from a clean commit and a remote `Go` decision for the same commit. The end-to-end `.github/workflows/release.yml` calls the gate directly; `.github/workflows/release-gate.yml` remains independently dispatchable for diagnostics.

The remote aggregate has a fixed 16-cell inventory: Linux Test Engine runtime, real Winit/WGPU and SDL3/Glow viewport smokes, Linux WASM, all 27 publishable source packages, Windows vcpkg, Windows MSVC `/MD` and `/MT`, Windows GNU imports, macOS, and five prebuilt producer/consumer targets (Linux x86_64, macOS x86_64/aarch64, and Windows MSVC `/MD`/`/MT`). A missing, failed, skipped, cancelled, timed-out, malformed, duplicate, or wrong-SHA cell is `No-Go`; callers cannot narrow the production inventory.

The run retains `gate-result.json`, stdout/stderr, runtime invocation/results, Xvfb/Mesa display and renderer data, target/CRT/vcpkg/MinGW metadata, binding hashes, manifests, candidate SHA, source-package results, and SHA256 evidence for approximately 30 days. Verify a downloaded aggregate locally with:

```bash
python3 tools/ci/release_evidence.py verify \
  --repo-root . \
  --candidate-sha CANDIDATE_SHA \
  --gate-result artifacts/release-gate/gate-result.json
```

Headless Test Engine success does not replace the real viewport cells. Missing display or software-GPU infrastructure is a failed remote gate, not a skipped success.

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

Preparation and validation remain separate because preparation intentionally changes the worktree:

```bash
python3 tools/tasks.py release-prepare 0.16.0-alpha.1
git diff
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.1"
python3 tools/tasks.py release-check
```

After the candidate is merged to `main` and normal CI is green, run the single release entry point:

```bash
gh workflow run release.yml --ref main -f tag=v0.16.0-alpha.1
```

`release.yml` binds the tag, workspace version, `main` ref, and exact candidate; runs the 16-cell gate; stages only checksummed recorded archives; publishes the complete 27-package train with crates.io Trusted Publishing; verifies every exact version; then creates the tag and GitHub Release. See [PUBLISHING.md](./PUBLISHING.md) for setup and recovery.

## Pre-release checklist

Before dispatching, verify:

- Root `workspace.package.version` is correct, all 27 publishable manifests inherit it, internal dependencies inherit root workspace declarations, and `CHANGELOG.md` has matching release notes.
- Binding profiles reproduce, packaged sys crates work without Git metadata, and `docs.rs` offline routes remain valid.
- Compatibility documentation and examples reflect public API or dependency changes.
- The candidate is merged to `main`, the worktree is clean, local `release-check` passed, and normal CI is green.
- The protected `release` environment and all 27 crates.io Trusted Publishers target `.github/workflows/release.yml`.
- No release tag was manually created for a different commit.

## Notes
- Docking is available in the core build. Multi-viewport remains an explicit feature because platform and renderer callback lifecycles must be selected together.
- docs.rs offline builds rely solely on checked-in target-appropriate bindings; source builds still require submodules or an exact matching prebuilt artifact.
- The node-editor blueprints stack layout is a separate native artifact profile and cannot be substituted for the normal core or WASM profile.
- If you need extra docs.rs cfgs later, extend each `-sys` crate’s `DOCS_RS` path in its `build.rs`.
