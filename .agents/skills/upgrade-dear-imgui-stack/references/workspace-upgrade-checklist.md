# Workspace Upgrade Checklist

Use this file for the repository-specific parts of an ImGui-stack upgrade.

## Upstream map

| Workspace crate | Upstream / submodule | Default branch in repo/tooling | Notes |
|---|---|---|---|
| `dear-imgui-sys` | `cimgui` + Dear ImGui | `docking_inter` | Core ABI source. Regenerate native + WASM bindings. Audit `backend_shim` on backend/platform changes. Re-audit the native stack layout `ItemSize()` / `ItemAdd()` patch and prebuilt `stack-layout` manifest feature on every core bump. |
| `dear-implot-sys` | `cimplot` + ImPlot | `master` | Re-audit safe ImPlot wrappers when `ImPlotSpec` or item APIs change. |
| `dear-implot3d-sys` | `cimplot3d` + ImPlot3D | `main` | Re-audit spec/item styling, mesh/image entry points, color enums. |
| `dear-imnodes-sys` | `cimnodes` + ImNodes | `master` | Usually independent, but scan for compatibility if core types changed. |
| `dear-node-editor-sys` | `cimnodes_editor` + imgui-node-editor | `main` | Native only. Keep the local `dne_*` uintptr ID shim in sync with upstream ID and callback APIs. |
| `dear-imguizmo-sys` | `cimguizmo` + ImGuizmo | `master` | Usually independent, but scan if cimgui/imgui integration changed. |
| `dear-imguizmo-quat-sys` | `cimguizmo_quat` + ImGuIZMO.quat | `master` | Independent from `cimguizmo`; select it with `--cimguizmo-quat-branch`. |
| `dear-imgui-test-engine-sys` | `imgui_test_engine` | `main` | Re-audit whenever Dear ImGui internals or hooks changed. Native only, no wasm support. |

## Primary sources to inspect

- Dear ImGui GitHub release notes and `docs/CHANGELOG.txt`
- `cimgui` changelog, commit diff, and generated headers for the target version
- `cimplot` / `cimplot3d` commit diff and public headers
- `imgui_test_engine` changelog / commit diff for hook and integration changes
- Local generated binding diffs in each affected `*-sys` crate

Use primary sources only when determining what changed upstream.

## Canonical command recipes

### Refresh submodules and pregenerated bindings

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:LIBCLANG_PATH = '<canonical LLVM 14 bin directory>'
python tools/update_submodule_and_bindings.py `
  --crates dear-imgui-sys,dear-imgui-test-engine-sys `
  --submodules auto `
  --profile release `
  --cimgui-branch docking_inter `
  --imgui-test-engine-branch main `
  --wasm
```

Use the libclang major version selected by repository CI or binding-generation documentation; do not substitute a newer local install just because it is available. Expand `--crates` and the upstream branch arguments only when those independent libraries are part of the requested upgrade. Core WASM regeneration covers the supported ImGui-dependent extension profiles, so the legacy `--wasm-ext` compatibility argument is normally unnecessary.

Verify that every generated file carries the expected source revision and deterministic hash before editing the safe layer:

```powershell
cargo run -p xtask -- verify-bindings --allow-dirty
```

### Bump unified release version

```powershell
cargo run -p xtask -- release-version 0.16.0
```

The root workspace version is the single source of truth. Publishable packages
inherit it, and internal path dependencies inherit the matching root dependency
requirements. After a version update, refresh and verify the lockfile:

```powershell
cargo metadata --format-version 1 --no-deps | Out-Null
cargo metadata --locked --format-version 1 --no-deps
```

Also refresh standalone example lockfiles if their local path dependencies changed version:

```powershell
cargo update
```

Run that in each standalone example workspace that carries its own `Cargo.lock`.

## Repository-specific audit checklist

1. Safe API completeness
   - Compare new sys symbols against `dear-imgui-rs`, `dear-implot`, `dear-implot3d`, `dear-imnodes`, and `dear-imgui-test-engine`.
   - For `dear-node-editor`, audit the local `dne_*` C ABI shim first; do not expose upstream `NodeId*` / `PinId*` / `LinkId*` helper-pointer APIs directly.
   - Audit new enums, flags, struct fields, style/spec arrays, callback setters, and renamed upstream items.
   - Audit removed functions and changed return types as source-breaking changes; generated code compiling does not prove the old safe wrapper still models upstream behavior.
   - Prefer transparent wrappers over handwritten native structure mirrors. If a mirror is required, validate field offsets and a semantic sentinel that distinguishes count, frame, ID, and pointer fields; size/alignment assertions alone cannot catch same-sized substitutions.
   - Trace hidden queue, ownership, and lifecycle fields through upstream implementation code. Check native auto-transitions against Rust sidecars, renderer feedback, abandoned work, retries, and teardown.
   - If the new sys surface makes the old safe shape awkward, refactor the safe layer instead of layering compatibility hacks.

2. Backend and platform impact
   - Audit `dear-imgui-sys/src/backend_shim/**` and `dear-imgui-sys/build.rs`.
   - Re-check the stack layout patch path in `dear-imgui-sys/build.rs`, `dear-imgui-sys/src/stack_layout_shim.cpp`, and `dear-imgui-sys/src/stack_layout_imgui_*.cpp.inc`. The marker patch must still match the new upstream `imgui.cpp`, and inactive `ItemSize()` / `ItemAdd()` hot paths should remain fast.
   - Check `dear-imgui-sdl3`, `dear-imgui-wgpu`, `dear-imgui-winit`, `dear-imgui-glow`, `dear-imgui-ash`.
   - If backend exposure changed, adapt public APIs and repository-local examples, including iOS / Android smoke examples when relevant.

3. Test engine
   - Update `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine`.
   - Check `dear-imgui-sys` `test-engine` feature integration and hook files.
   - Validate `dear-imgui-test-engine` still links and its bindings remain pregenerated.
   - Compare the complete presentation lifecycle with upstream: render, `pre_swap`, present/swap, then `post_swap`. Exercise both the bounded runner and at least one real presentation integration when those hooks change.

4. Deprecated removals
   - Search `CHANGELOG.md` for deprecations that promised removal in the target release.
   - Remove or migrate them during the breaking release instead of carrying them forward.

5. Docs and release train
   - Update `CHANGELOG.md`
   - Update `README.md` compatibility/release references if version baselines changed
   - Update `docs/COMPATIBILITY.md`
   - Update `docs/PUBLISHING.md` and `docs/RELEASING.md` if publish order, helper crates, or release tooling changed
   - Check `xtask release-version`, `tools/publish.py`, `tools/pre_publish_check.py`, and `tools/tasks.py` if release mechanics changed

6. Helper crate/versioning
   - Keep `tools/build-support` on the unified release train unless there is a deliberate reason not to.
   - `dear-imgui-build-support` inherits the unified workspace release version. Validate packaging and publish ordering whenever its contract changes.

## Validation matrix

Run the smallest set that fully covers the upgraded surface.

### Baseline

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo run -p xtask -- verify-bindings --allow-dirty
cargo fmt --all -- --check
cargo check --workspace
python tools/api_surface_report.py --check
python tools/pre_publish_check.py
python tools/publish.py --dry-run
```

### Recommended targeted tests

```powershell
cargo nextest run -p dear-imgui-rs -p dear-implot -p dear-implot3d -p dear-imnodes -p dear-imgui-test-engine -p dear-imgui-test-engine-sys --test-threads=1
cargo check -p dear-imgui-rs --features wasm --target wasm32-unknown-unknown
```

### Package/publish smoke checks

```powershell
cargo package -p dear-imgui-build-support
cargo package -p dear-imgui-sys --list
```

If the working tree is intentionally dirty during local verification, use `--allow-dirty`.

### docs.rs / offline checks

```powershell
$env:DOCS_RS = '1'; cargo check -p dear-imgui-sys
$env:DOCS_RS = '1'; cargo check -p dear-implot-sys
$env:DOCS_RS = '1'; cargo check -p dear-implot3d-sys
$env:DOCS_RS = '1'; cargo check -p dear-imnodes-sys
$env:DOCS_RS = '1'; cargo check -p dear-node-editor-sys
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-sys
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-quat-sys
$env:DOCS_RS = '1'; cargo check -p dear-imgui-test-engine-sys
```

### Example checks to consider

```powershell
cargo check -p dear-imgui-examples --bin implot_basic --features implot
cargo check -p dear-imgui-examples --bin implot3d_basic --features implot3d
cargo check -p dear-imgui-examples --bin imgui_test_engine_basic --features test-engine
cargo check -p dear-imgui-examples --bin node_editor_showcase --features node-editor-blueprints
```

If backend or mobile integration changed, also re-run the relevant repository-local iOS / Android smoke checks from CI.

## Release-note checklist

For an actual release:

1. Convert the top `Unreleased` notes into `## [x.y.z] - YYYY-MM-DD`.
2. Add a short release summary paragraph.
3. Add `Highlights` for the few changes users should notice first.
4. Keep `Breaking Changes` explicit and migration-oriented.
5. Mention version-train or publish-flow changes if they affect downstream users.
