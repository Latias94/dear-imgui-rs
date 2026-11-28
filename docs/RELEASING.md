# Releasing (sys crates with offline docs.rs)

> **Note**: For the current operator commands, see
> [`tools/README.md`](../tools/README.md). This document covers sys binding
> generation, provenance, and the release evidence contract.

The `-sys` crates must build docs on docs.rs without network, Git metadata, or
submodules. Core Dear ImGui checks in `bindings_pregenerated.rs` for both native
ABI profiles and `wasm_bindings_pregenerated.rs` for the fixed browser import ABI.
Extension sys crates keep their native and optional WASM pregenerated files.

Supported crates:
- `dear-imgui-sys` (third-party: cimgui)
- `extensions/dear-implot-sys` (third-party: cimplot)
- `extensions/dear-imnodes-sys` (third-party: cimnodes)
- `extensions/dear-node-editor-sys` (third-party: cimnodes_editor / imgui-node-editor; native only)
- `extensions/dear-imguizmo-sys` (third-party: cimguizmo)
- `extensions/dear-implot3d-sys` (third-party: cimplot3d)
- `extensions/dear-imguizmo-quat-sys` (third-party: cimguizmo_quat)
- `extensions/dear-imgui-cte-sys` (third-party: cimCTE / ImGuiColorTextEdit)
- `extensions/dear-imgui-test-engine-sys` (third-party: imgui_test_engine; native only)

## Prerequisites
- `git`, `cargo`, and Python 3.11+ in PATH.
- A clean, committed release candidate. Local and remote release gates bind to
  the exact 40-hex `HEAD`; a branch name is not release evidence.
- If you want to update third-party code, allow the script to update submodules/branches.

## Binding update workflow

Build `xtask` with the workspace Rust toolchain. Canonical generation uses
libclang 14.0 and the rustfmt from Rust 1.95.0. Install the formatter toolchain
with `rustup toolchain install 1.95.0 --profile minimal --component rustfmt`.
Set `LIBCLANG_PATH` to the LLVM 14 library directory if needed.

Run `tools/update_submodule_and_bindings.py` to update source metadata and
regenerate the maintained binding files.

Key flags:
- `--crates`: comma-separated list or `all`.
- `--submodules`: `update` (update all known submodules), `auto` (update only selected crates), `skip` (don’t touch submodules).
- `--wasm`: compile-check the explicit core WASM provider feature after generation.
- The WASM import module is fixed to `imgui-sys-v1`; it is not a command-line option.
- Per-submodule branches:
  - `--cimgui-branch` (default `docking_inter`)
  - `--cimplot-branch` (default `master`)
  - `--cimplot3d-branch` (default `main`)
  - `--cimnodes-branch` (default `master`)
  - `--cimnodes-editor-branch` (default `main`)
  - `--cimguizmo-branch` (default `master`)
  - `--cimguizmo-quat-branch` (default `master`)
  - `--cte-branch` (default `main_goossens`)
  - `--imgui-test-engine-branch` (default `main`)

Examples
- dear-imgui-sys only (update submodule + pregenerate, Release):
```
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules auto \
  --cimgui-branch docking_inter
```

- All known -sys crates (update all submodules + pregenerate, Release):
```
python3 tools/update_submodule_and_bindings.py \
  --crates all --submodules update \
  --cimgui-branch docking_inter \
  --cimplot-branch master \
  --cimplot3d-branch main \
  --cimnodes-branch master \
  --cimnodes-editor-branch main \
  --cimguizmo-branch master \
  --cimguizmo-quat-branch master \
  --cte-branch main_goossens \
  --imgui-test-engine-branch main
```

- All known -sys crates plus current WASM pregenerated bindings, without moving submodules:
```
python3 tools/update_submodule_and_bindings.py \
  --crates all --submodules skip \
  --wasm
```

- Regenerate pregenerated bindings only (no submodule changes):
```
python3 tools/update_submodule_and_bindings.py \
  --crates dear-implot-sys,dear-imnodes-sys \
  --submodules skip
```

What the script does:

- Optionally updates selected submodules and synchronizes the exact cimgui/nested Dear ImGui revisions stored in `dear-imgui-sys/Cargo.toml` package metadata.
- Refuses staged, unstaged, or untracked changes in the source submodules before recording provenance.
- Invokes `cargo run -p xtask -- verify-bindings --update --allow-dirty` to
  reproduce each maintained profile twice before writing the checked-in files.
- Verifies the full compatibility target sets for the Windows and non-Windows
  profiles, then merges their two Windows-only padding fields behind
  `#[cfg(windows)]` in the shared native file.

Run the binding checks or generation commands directly when source metadata
already matches the vendored sources.

```bash
# Reproduce and compare the native and WASM artifacts for all maintained crates.
cargo run -p xtask -- verify-bindings

# Validate checked-in bindings without running bindgen.
cargo run -p xtask -- verify-bindings --check-only

# Regenerate the files after an intentional source or specification change.
cargo run -p xtask -- verify-bindings --update --allow-dirty

# Regenerate just the CTE WASM import binding.
cargo run -p xtask -- wasm-bindgen-cte
```

Canonical generation rejects `BINDGEN_EXTRA_CLANG_ARGS*`; target facts, header shims, formatter, allow/block lists, enum normalization, opaque types, and provider name all participate in the deterministic specification hash.

## Source and prebuilt provenance

The exact 40-hex cimgui and nested Dear ImGui revisions are package metadata, not values discovered from Git during a consumer build. They survive `cargo package` and are available in an unpacked crate with no `.git` directory.

A `dear-imgui-sys` core native prebuilt is accepted only when its manifest exactly matches crate/version, target triple, static link type, MSVC CRT, normalized features, both source revisions, and binding-spec hash. The normal, freetype, stack-layout, and stack-layout + freetype combinations have different names and manifests. Missing, duplicate, unknown, or mismatched fields reject the core artifact.

The seven safe extension crates (`dear-implot`, `dear-implot3d`, `dear-imnodes`,
`dear-imguizmo`, `dear-imguizmo-quat`, `dear-imgui-cte`, and `dear-node-editor`) forward
`prebuilt` and `build-from-source` through both core and extension sys crates.
Source wins when Cargo unifies both routes. Each extension prebuilt manifest
binds its source and binding identity to the exact core artifact and candidate
SHA. Test Engine is separate: its crate-local source/shim/generator provenance
is reproducible, but it is native source-only and never appears in a prebuilt
artifact profile.

## Release gates

A release requires `python3 tools/tasks.py release-check` from a clean commit
and a successful `ci.yml` push run for that exact `main` commit. Normal CI owns
the Test Engine, real Winit/WGPU, SDL3/Glow, and Ash/Vulkan viewport smokes,
WASM, source packages, Windows native routes, macOS, bindings, MSRV, tests, and
documentation checks.

The release workflow does not replay those jobs or aggregate a second evidence
schema. It directly builds and consumes all prebuilt profiles on five target/CRT
combinations; any failed producer or isolated consumer stops publication.
Normal Actions logs and artifacts remain the diagnostic record. Headless Test
Engine success does not replace the real viewport jobs in normal CI.

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
$env:DOCS_RS = '1'; cargo check -p dear-imgui-cte-sys
$env:DOCS_RS = '1'; cargo check -p dear-imgui-test-engine-sys
Remove-Item Env:DOCS_RS
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
DOCS_RS=1 cargo check -p dear-imgui-cte-sys
DOCS_RS=1 cargo check -p dear-imgui-test-engine-sys
```

These checks generate/use bindings only and won’t build/link native code.

## Recommended release workflow

Preparation and validation remain separate because preparation intentionally changes the worktree:

```bash
python3 tools/tasks.py release-prepare 0.23.0
git diff
git add -p
# Stage reviewed new files explicitly with git add -- <path>.
git commit -m "chore(release): prepare v0.23.0"
python3 tools/tasks.py release-check
```

After the candidate is merged to `main` and normal CI is green, run the single release entry point:

```bash
gh workflow run release.yml --ref main -f tag=v0.23.0
```

`release.yml` binds the tag, workspace version, `main` ref, and exact candidate;
requires a successful `main` push CI run for that SHA; builds and consumes all
five prebuilt targets; atomically creates or verifies the candidate tag inside
the protected environment before the first upload; publishes the complete
29-package train with crates.io Trusted Publishing; verifies every exact
version; then validates asset names and SHA-256 digests before completing the
checksummed GitHub Release. A rerun accepts only an identical existing asset
subset and never overwrites a differing same-name asset. See
[PUBLISHING.md](./PUBLISHING.md) for setup and recovery.

## Pre-release checklist

Before dispatching, verify:

- Root `workspace.package.version` is correct, all 29 publishable manifests inherit it, internal dependencies inherit root workspace declarations, and `CHANGELOG.md` has matching release notes.
- Binding profiles reproduce, packaged sys crates work without Git metadata, and `docs.rs` offline routes remain valid.
- Compatibility documentation and examples reflect public API or dependency changes.
- The candidate is merged to `main`, the worktree is clean, local `release-check` passed, and normal CI is green.
- The protected `release` environment and all 29 crates.io Trusted Publishers target `.github/workflows/release.yml`.
- No release tag was manually created for a different commit.

## Notes
- Docking is available in the core build. Multi-viewport remains an explicit feature because platform and renderer callback lifecycles must be selected together.
- docs.rs offline builds rely solely on checked-in target-appropriate bindings; source builds still require submodules or an exact matching prebuilt artifact.
- The node-editor blueprints stack layout is a separate native artifact profile and cannot be substituted for the normal core or WASM profile.
- If you need extra docs.rs cfgs later, extend each `-sys` crate’s `DOCS_RS` path in its `build.rs`.
