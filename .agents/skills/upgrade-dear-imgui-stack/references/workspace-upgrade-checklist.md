# Workspace Upgrade Checklist

Use this file for the repository-specific parts of an ImGui-stack upgrade.

## Upstream map

| Workspace crate | Upstream / submodule | Default branch in repo/tooling | Notes |
|---|---|---|---|
| `dear-imgui-sys` | `cimgui` + Dear ImGui | `docking_inter` | Core ABI source. Regenerate native + WASM bindings. Audit `backend_shim` on backend/platform changes. |
| `dear-implot-sys` | `cimplot` + ImPlot | `master` | Re-audit safe ImPlot wrappers when `ImPlotSpec` or item APIs change. |
| `dear-implot3d-sys` | `cimplot3d` + ImPlot3D | `main` | Re-audit spec/item styling, mesh/image entry points, color enums. |
| `dear-imnodes-sys` | `cimnodes` + ImNodes | `master` | Usually independent, but scan for compatibility if core types changed. |
| `dear-imguizmo-sys` | `cimguizmo` + ImGuizmo | `master` | Usually independent, but scan if cimgui/imgui integration changed. |
| `dear-imguizmo-quat-sys` | `cimguizmo_quat` + ImGuIZMO.quat | script currently shares `cimguizmo` branch arg | Verify upstream default branch before changing tooling. |
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
python tools/update_submodule_and_bindings.py `
  --crates all `
  --submodules update `
  --profile release `
  --cimgui-branch docking_inter `
  --cimplot-branch master `
  --cimplot3d-branch main `
  --cimnodes-branch master `
  --cimguizmo-branch master `
  --imgui-test-engine-branch main `
  --wasm `
  --wasm-import imgui-sys-v0 `
  --wasm-ext implot,implot3d,imnodes,imguizmo,imguizmo-quat
```

Narrow `--crates` / `--wasm-ext` if only part of the stack changed.

### Bump unified release version

```powershell
python tools/bump_version.py 0.11.0
```

This updates published workspace crate versions and internal dependency minors. After a version bump, refresh lockfiles:

```powershell
cargo update -w
```

Also refresh example lockfiles if their local path dependencies changed version:

```powershell
cargo update -p dear-imgui-build-support --precise 0.11.0
```

Run that in each standalone example workspace that carries its own `Cargo.lock`.

## Repository-specific audit checklist

1. Safe API completeness
   - Compare new sys symbols against `dear-imgui-rs`, `dear-implot`, `dear-implot3d`, `dear-imnodes`, and `dear-imgui-test-engine`.
   - Audit new enums, flags, struct fields, style/spec arrays, callback setters, and renamed upstream items.
   - If the new sys surface makes the old safe shape awkward, refactor the safe layer instead of layering compatibility hacks.

2. Backend and platform impact
   - Audit `dear-imgui-sys/src/backend_shim/**` and `dear-imgui-sys/build.rs`.
   - Check `dear-imgui-sdl3`, `dear-imgui-wgpu`, `dear-imgui-winit`, `dear-imgui-glow`, `dear-imgui-ash`.
   - If backend exposure changed, adapt public APIs and repository-local examples, including iOS / Android smoke examples when relevant.

3. Test engine
   - Update `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine`.
   - Check `dear-imgui-sys` `test-engine` feature integration and hook files.
   - Validate `dear-imgui-test-engine` still links and its bindings remain pregenerated.

4. Deprecated removals
   - Search `CHANGELOG.md` for deprecations that promised removal in the target release.
   - Remove or migrate them during the breaking release instead of carrying them forward.

5. Docs and release train
   - Update `CHANGELOG.md`
   - Update `README.md` compatibility/release references if version baselines changed
   - Update `docs/COMPATIBILITY.md`
   - Update `docs/PUBLISHING.md` and `docs/RELEASING.md` if publish order, helper crates, or release tooling changed
   - Check `tools/publish.py`, `tools/pre_publish_check.py`, `tools/bump_version.py`, and `tools/tasks.py` if release mechanics changed

6. Helper crate/versioning
   - Keep `tools/build-support` on the unified release train unless there is a deliberate reason not to.
   - If `dear-imgui-build-support` changes version, update every `*-sys` crate dependency and validate packaging/publish ordering again.

## Validation matrix

Run the smallest set that fully covers the upgraded surface.

### Baseline

```powershell
cargo fmt --all
cargo check --workspace
python tools/pre_publish_check.py
python tools/publish.py --dry-run
```

### Recommended targeted tests

```powershell
cargo nextest run -p dear-imgui-rs -p dear-implot -p dear-implot3d -p dear-imnodes
cargo test -p dear-imgui-test-engine --lib
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
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-sys
$env:DOCS_RS = '1'; cargo check -p dear-imguizmo-quat-sys
$env:DOCS_RS = '1'; cargo check -p dear-imgui-test-engine-sys
```

### Example checks to consider

```powershell
cargo check -p dear-imgui-examples --bin implot_basic --features implot
cargo check -p dear-imgui-examples --bin implot3d_basic --features implot3d
cargo check -p dear-imgui-examples --bin imgui_test_engine_basic --features test-engine
```

If backend or mobile integration changed, also re-run the relevant repository-local iOS / Android smoke checks from CI.

## Release-note checklist

For an actual release:

1. Convert the top `Unreleased` notes into `## [x.y.z] - YYYY-MM-DD`.
2. Add a short release summary paragraph.
3. Add `Highlights` for the few changes users should notice first.
4. Keep `Breaking Changes` explicit and migration-oriented.
5. Mention version-train or publish-flow changes if they affect downstream users.
