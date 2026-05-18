# Compatibility Matrix

This document tracks compatibility across the workspace crates, upstream Dear ImGui, and key external dependencies. The root README shows the latest recommendations; this file keeps history and upgrade guidance.

For Apple-specific integration notes and the repository-owned iOS example
paths, see `docs/workstreams/apple-platform-support.md`.

## Versioning Policy

- Unified release train: all published `dear-*` crates in this workspace are versioned and released together under the same semver, so consumers can depend on a single minor across the board.
- Current train: unified `v0.13.0` (use `version = "0.13"`).
- Previous train: unified `v0.12.0` (use `version = "0.12"`).
- Previous train: unified `v0.11.0` (use `version = "0.11"`).
- Previous train: unified `v0.10.4` (use `version = "0.10"`).
- Previous train: unified `v0.9.0` (use `version = "0.9"`).
- Previous train: unified `v0.8.0` (use `version = "0.8"`).
- Internal dependency constraints in this repo also pin to the unified minor (e.g., `0.5`). Mixing different minors across our crates is unsupported.

## Latest (Summary)

Core

| Crate           | Version | Upstream        | Notes                                     |
|-----------------|---------|-----------------|-------------------------------------------|
| dear-imgui-rs   | 0.13.0  | —               | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.13.0  | ImGui v1.92.8   | Docking branch via cimgui                 |

Backends

| Crate             | Version | External deps           | Notes |
|-------------------|---------|-------------------------|-------|
| dear-imgui-wgpu   | 0.13.0  | wgpu = 29/28/27        | WebGPU renderer (default wgpu 29; optional wgpu 28/27 via features; experimental multi-viewport on native via winit/SDL3; disabled on wasm) |
| dear-imgui-glow   | 0.13.0  | glow = 0.17            | OpenGL renderer (winit/glutin) |
| dear-imgui-ash    | 0.13.0  | ash = 0.38             | Vulkan renderer (native only). Optional: `ash-window` for winit multi-viewport; SDL3 multi-viewport via `Platform_CreateVkSurface`; `gpu-allocator`/`vk-mem` allocators |
| dear-imgui-winit  | 0.13.0  | winit = 0.30.13        | Winit platform backend |
| dear-imgui-sdl3   | 0.13.0  | sdl3 = 0.18, sdl3-sys 0.6 | SDL3 platform backend with optional official OpenGL3/SDLRenderer3 shims |

Utilities

| Crate     | Version | External deps | Notes |
|-----------|---------|---------------|-------|
| dear-app  | 0.13.0  | winit, wgpu   | App runner (docking, themes, add-ons) |

Tooling

| Crate                    | Version | External deps | Notes |
|--------------------------|---------|---------------|-------|
| dear-imgui-build-support | 0.13.0  | ureq = 3.3    | Shared build/publish helpers for `*-sys` crates and prebuilt archives |

Extensions

| Crate               | Version | Requires dear-imgui-rs | Sys crate                    | Notes                                  |
|---------------------|---------|------------------------|------------------------------|----------------------------------------|
| dear-implot         | 0.13.0  | 0.13.0                 | dear-implot-sys 0.13.0       | 2D plotting                            |
| dear-imnodes        | 0.13.0  | 0.13.0                 | dear-imnodes-sys 0.13.0      | Node editor                            |
| dear-node-editor    | 0.13.0  | 0.13.0                 | dear-node-editor-sys 0.13.0  | Native-only imgui-node-editor integration |
| dear-imguizmo       | 0.13.0  | 0.13.0                 | dear-imguizmo-sys 0.13.0     | 3D gizmo                               |
| dear-file-browser   | 0.13.0  | 0.13.0                 | —                            | ImGui UI + native (rfd) backends       |
| dear-implot3d       | 0.13.0  | 0.13.0                 | dear-implot3d-sys 0.13.0     | 3D plotting                            |
| dear-imguizmo-quat  | 0.13.0  | 0.13.0                 | dear-imguizmo-quat-sys 0.13.0 | Quaternion gizmo                      |
| dear-imgui-test-engine | 0.13.0 | 0.13.0                 | dear-imgui-test-engine-sys 0.13.0 | UI automation and test runner      |
| dear-imgui-reflect  | 0.13.0  | 0.13.0                 | —                            | Reflection-based UI helpers (pure Rust)|

## Trunk (Unreleased)

- Next release train: TBD.
- Main branch currently reflects post-`0.13.0` development and may move independently until the next planned release is cut.
- Current baselines after the `0.13.0` release: Dear ImGui v1.92.8 (docking) via cimgui, unified `dear-*` crate minor `0.13`, MSRV 1.92, and the external dependency baseline described above.
- Safe API soundness changes on trunk:
  - `TextureRef<'tex>` carries managed texture lifetimes; raw `TextureRef::from_raw` is unsafe.
  - Legacy texture references use `TextureId`; raw `u64` texture ids are no longer accepted by
    `TextureRef` or `create_texture_ref`.
  - `Context::render()` and renderer APIs use `&mut DrawData` so texture feedback goes through
    `DrawData::textures_mut()`.
  - `DrawData::textures()` / `PlatformIo::textures()` are read-only; mutable texture access requires
    mutable draw data or platform IO.
  - Core and extension RAII tokens that call `End`/`Pop` on drop are UI/context-bound and
    `!Send + !Sync`.
  - `StateStorageToken<'ui, 'storage>` binds the lifetime of both the active UI and pushed storage.
  - `FontId` is an opaque, atlas-validated, `!Send + !Sync` handle. It can still be stored in
    long-lived style state, but safe font push/draw APIs reject stale or wrong-atlas handles before
    crossing FFI.
  - `Context::font_atlas()` is read-only through `FontAtlasRef<'_>`; atlas mutation goes through
    `font_atlas_mut()` / `fonts()`.
  - Clipboard callback reentrancy is guarded per `ClipboardContext`, so same-context reentry fails
    closed without blocking callbacks for independent ImGui contexts.
  - `dear-imgui-test-engine` safe methods check the bound ImGui context liveness before FFI. Drop or
    explicitly `shutdown()` the test engine before dropping the target `Context`; stale bound-context
    use panics in Rust.
  - `dear-implot` colormap helpers use typed `ColormapIndex`, `ColormapColorIndex`, and
    `ColormapSelection` values. Keep the token returned by `push_colormap` alive instead of calling
    `pop_colormap(count)`.
  - `dear-implot` / `dear-implot3d` plot data layout APIs use typed
    `PlotDataLayout` / `Plot3DDataLayout`, typed sample offsets, and typed byte strides. Replace raw
    `offset, stride` pairs with `.with_offset(PlotDataOffset::samples(...))` plus
    `.with_stride(PlotDataStride::bytes(...))`, or use the `3D` variants for ImPlot3D. Default
    stride is explicit via `PlotDataStride::AUTO` / `Plot3DDataStride::AUTO`; zero-byte strides are
    rejected before FFI.
  - `PlotLines` / `PlotHistogram` `values_offset(...)` now takes `usize`/`PlotValueOffset` instead
    of a raw signed integer. Use `PlotValueOffset::new(...)` or pass `usize` directly.
  - Table freeze helpers now take `usize` frozen column/row counts instead of raw signed values.
  - Table column APIs use typed column values instead of raw signed sentinel integers. Use
    `TableColumnIndex::new(...)` for real columns, `TableColumnRef::Current` for current-column
    defaults, `TableContextMenuTarget` for context menus, and `TableHoveredColumn` for hover results.
    Use `table_set_cell_bg_color*` / `table_set_row_bg{0,1}_color*` instead of passing
    `TableBgTarget` plus `-1`.
  - Table row query APIs use `Option<TableRowIndex>` / `TableHoveredRow` instead of raw signed row
    sentinel values.
  - Legacy Columns APIs use typed counts and selectors: `usize` for counts, `OldColumnIndex` for
    concrete columns, `OldColumnRef::Current` for current-column defaults, and
    `OldColumnOffsetRef::Trailing` for the right-most offset line.
  - `dear-imnodes` node, pin, and link APIs use typed `NodeId`, `PinId`, and `LinkId` handles.
  - `dear-imnodes` style-var helpers take typed `StyleVar` values, and
    `NodeEditor::set_alt_mouse_button` takes `MouseButton`.
  - `dear-implot3d` style and colormap helpers use typed `Plot3DStyleVar`,
    `Plot3DColorElement`, `Colormap`, `ColormapIndex`, and `ColormapColorIndex` values instead of
    raw `i32` identifiers. Push helpers return RAII tokens, so migrate manual `pop_*` count calls to
    token lifetimes or `.pop()`.
  - `dear-imgui-test-engine::TestScript::mouse_click_on_void` takes `MouseButton` instead of a raw
    button index.
  - `dear-imgui-test-engine` table column script helpers use typed table column indices/targets
    instead of raw signed column indices.
  - `dear-imgui-test-engine` script repeat/wait frame counts use `ScriptCount`; construct values
    with `ScriptCount::new(...)` instead of passing raw signed integers.
  - `dear-imguizmo::GizmoUi::set_id(i32)` was removed; use `push_id(...)` and keep the returned
    token alive for the desired scope.
  - Current-context binding policy is part of the public safe API contract documented here and in
    the crate-level migration notes.
- Backend lifecycle changes on trunk:
  - `dear-imgui-glow::GlowRenderer::destroy()` clears renderer-owned multi-viewport state for the
    renderer, matching `Drop`. It makes installed callbacks no-op for that renderer, but callers
    should still use the matching multi-viewport shutdown helper to uninstall callbacks and destroy
    platform windows.
  - `dear-imgui-sdl3` combined platform+renderer init helpers now roll back the SDL3 platform
    backend when the renderer backend init step fails.
  - `dear-imgui-sdl3` now provides `Sdl3PlatformBackend`, `Sdl3OpenGl3Backend`, and
    `Sdl3RendererBackend` RAII owners that bind backend calls to the captured `Context` and
    shut down official backend state on drop.
  - `dear-imgui-wgpu::WgpuRenderer::shutdown()` clears renderer-owned multi-viewport state for the
    renderer, matching `Drop`. It makes installed callbacks no-op for that renderer, but callers
    should still use the matching multi-viewport shutdown helper to uninstall callbacks and destroy
    platform windows.
  - Renderer multi-viewport shutdown helpers in `dear-imgui-wgpu`, `dear-imgui-ash`, and
    `dear-imgui-glow` now destroy platform windows before uninstalling renderer callbacks. This lets
    Dear ImGui call renderer destroy callbacks while renderer-owned per-viewport data is still
    reachable.
  - `dear-imgui-ash` Winit and SDL3 multi-viewport swapchain recreation keeps the old swapchain
    resources alive until the replacement swapchain, image views, and framebuffers are fully
    created. Failed resize or present recovery no longer leaves the secondary viewport in a
    partially destroyed state.
- Backend texture feedback changes on trunk:
  - `dear-imgui-glow::GlowRenderer::update_texture(_with_context)` now updates registered renderer
    texture-map entries by their stored OpenGL texture handle instead of treating the public
    `TextureId` as a raw GL texture name. Non-null unregistered ids still use the legacy raw-id
    fallback; null texture ids now return an error.
  - `dear-imgui-glow` convenience texture registration/update handles `TextureFormat::Alpha8` by
    expanding to RGBA, matching the draw-data texture path.
  - `dear-app` now exposes structured startup/render errors instead of using generic strings for
    missing frame callbacks, WGPU adapter/device/surface initialization, renderer construction,
    and frame preparation/rendering failures. `DearAppError` is now `#[non_exhaustive]`.
  - `dear-app::GpuApi` texture registration/update/removal methods use `TextureId` instead of raw
    `u64` ids.
  - `dear-imgui-glow`, `dear-imgui-wgpu`, and `dear-imgui-ash` now use more specific error
    variants for common invalid-state and resource failures instead of collapsing every path into
    a generic renderer string. Their public renderer error enums are now `#[non_exhaustive]`.
  - `dear-imgui-wgpu::TextureUpdateResult::Destroyed.apply_to(...)` now sets Dear ImGui's
    destroy-next-frame precondition before writing `TextureStatus::Destroyed`, matching the Ash
    backend helper behavior.
  - `dear-imgui-wgpu` preserves the previous GPU texture mapping if full texture recreation fails
    during an update of an existing texture.
  - `dear-imgui-ash` now defers draw-data texture `TexID`/`OK` feedback until Vulkan upload command
    submission succeeds, so failed uploads no longer leave ImGui pointing at an unregistered texture
    id.
  - `dear-imgui-ash` now keeps existing textures and mesh buffers until Vulkan replacement resources
    are created and uploaded successfully, and cleans up partially-created Vulkan resources on
    allocation or upload setup failure.
  - `dear-imgui-sys::backend_shim::{dx11,opengl3,sdlrenderer3}` render entry points and
    `dear-imgui-sdl3` official renderer helpers now take mutable draw data, matching Dear ImGui
    1.92 texture feedback semantics.

## History

Release Train 0.13 (current)

- All crates unified to 0.13.0 across the workspace (use `version = "0.13"`).
- Core + backends aligned with Dear ImGui v1.92.8 (docking) via cimgui.
- Normal source builds use checked-in pregenerated bindings by default; LLVM/libclang is only required for explicit binding regeneration.
- `dear-node-editor` / `dear-node-editor-sys` are native-only in the first integration phase and coexist with the existing `dear-imnodes` wasm-capable node editor.
- `dear-imgui-sys` includes the stack layout ABI used by the node-editor blueprints example; prebuilts must declare `features=stack-layout`.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.12 (previous)

- All crates unified to 0.12.0 across the workspace (use `version = "0.12"`).
- Core + backends aligned with Dear ImGui v1.92.8 (docking) via cimgui.
- `dear-imgui-build-support` ships on the same `0.12.x` train as the published workspace crates.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.11 (previous)

- All crates unified to 0.11.0 across the workspace (use `version = "0.11"`).
- Core + backends aligned with Dear ImGui v1.92.7 (docking) via cimgui.
- `dear-imgui-build-support` moved into the unified release train.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.10 (previous)

- All crates unified to 0.10.4 across the workspace (use `version = "0.10"`).
- Core + backends aligned with Dear ImGui v1.92.6 (docking) via cimgui.
- External dependencies baseline: wgpu 29, winit 0.30.12, glow 0.16, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.9 (previous)

- All crates unified to 0.9.0 across the workspace (use `version = "0.9"`).
- External dependencies baseline: wgpu 28, winit 0.30.12, glow 0.16, sdl3 0.17.
- Minimum supported Rust: 1.92 (required by `wgpu` 28).

Release Train 0.8 (previous)

- Planned changes (subject to adjustment before release):
  - Core + backends remain aligned with Dear ImGui v1.92.5 and the same wgpu/winit/glow/sdl3 baselines.
  - Import-style WASM support (via `imgui-sys-v0` provider) for selected extension crates:
    - `dear-implot` / `dear-implot-sys`: 2D plotting on wasm.
    - `dear-imnodes` / `dear-imnodes-sys`: node editor on wasm.
    - `dear-imguizmo` / `dear-imguizmo-sys`: 3D gizmo on wasm.
    - `dear-imguizmo-quat` / `dear-imguizmo-quat-sys`: quaternion gizmo on wasm.
    - `dear-implot3d` / `dear-implot3d-sys`: 3D plotting on wasm.
  - New/updated `xtask` flows for building the wasm demo and import-style provider:
    - `wasm-bindgen-*` commands for core + extensions (ImPlot, ImPlot3D, ImNodes, ImGuizmo, ImGuIZMO.quat).
    - `web-demo [features]` to toggle which extensions are compiled into the web demo.
    - `build-cimgui-provider` to build the shared `imgui-sys-v0` provider (Emscripten).

Release Train 0.7 (previous)

- All crates unified to 0.7.0 across the workspace (use `version = "0.7"`).
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16, sdl3 0.16.
- Patch note: `dear-imgui-rs` has a core-only patch release at 0.7.1; other workspace crates remain at 0.7.0.

Release Train 0.6 (previous)

- All crates unified to 0.6.0 across the workspace
  - Core: dear-imgui-rs 0.6.0, dear-imgui-sys 0.6.0
  - Backends: dear-imgui-wgpu 0.6.0, dear-imgui-glow 0.6.0, dear-imgui-winit 0.6.0
  - Utilities: dear-app 0.6.0
  - Extensions: dear-implot 0.6.0, dear-imnodes 0.6.0, dear-imguizmo 0.6.0, dear-implot3d 0.6.0, dear-imguizmo-quat 0.6.0, dear-file-browser 0.6.0, dear-imgui-reflect 0.6.0
  - Sys crates: dear-implot-sys 0.6.0, dear-imnodes-sys 0.6.0, dear-imguizmo-sys 0.6.0, dear-implot3d-sys 0.6.0, dear-imguizmo-quat-sys 0.6.0
- dear-imgui-sys 0.6.x binds Dear ImGui v1.92.5 (docking) via cimgui
- New features:
  - New drag/drop flag and style color for improved drop target customization
  - Inherited all bug fixes and behavior changes from Dear ImGui v1.92.5
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16, sdl3 0.16
- Upgrade: change `version = "0.5"` to `version = "0.6"` in your Cargo.toml for all `dear-*` crates

### Backend & multi-viewport support notes (0.6.x)

- Winit + WGPU:
  - Multi-viewport support has experimental code paths in `dear-imgui-winit` + `dear-imgui-wgpu`, but is **not supported** in 0.6.x.
  - The `multi_viewport_wgpu` example is provided strictly as a **testbed** and is known to be unstable on some platforms (especially macOS/winit).
  - Do not rely on winit + WGPU multi-viewport for production use in this release train.
  - SDL3 + OpenGL3:
    - Supported via `dear-imgui-sdl3` (C++ `imgui_impl_sdl3.cpp` + `imgui_impl_opengl3.cpp`).
    - Multi-viewport: **supported** using the upstream SDL3 + OpenGL3 backend behaviour.
    - Example: `sdl3_opengl_multi_viewport` (`cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`).
  - SDL3 + WGPU:
    - Supported via SDL3 platform backend (`dear-imgui-sdl3`) + Rust WGPU renderer (`dear-imgui-wgpu`).
    - A single-window example is provided: `sdl3_wgpu` (`cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform`).
    - Multi-viewport for WebGPU remains **disabled** on this route, matching upstream `imgui_impl_wgpu` which currently does not implement multi-viewport.

Release Train 0.5 (previous)

- All crates unified to 0.5.0 across the workspace
  - Core: dear-imgui-rs 0.5.0, dear-imgui-sys 0.5.0
  - Backends: dear-imgui-wgpu 0.5.0, dear-imgui-glow 0.5.0, dear-imgui-winit 0.5.0
  - Utilities: dear-app 0.5.0
  - Extensions: dear-implot 0.5.0, dear-imnodes 0.5.0, dear-imguizmo 0.5.0, dear-implot3d 0.5.0, dear-imguizmo-quat 0.5.0, dear-file-browser 0.5.0
  - Sys crates: dear-implot-sys 0.5.0, dear-imnodes-sys 0.5.0, dear-imguizmo-sys 0.5.0, dear-implot3d-sys 0.5.0, dear-imguizmo-quat-sys 0.5.0
- dear-imgui-sys 0.5.x binds Dear ImGui v1.92.4 (docking) via cimgui
- New features:
  - Added `StyleColor::UnsavedMarker` for marking unsaved documents/windows
  - Inherited all bug fixes from Dear ImGui v1.92.4
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16
- Upgrade: change `version = "0.4"` to `version = "0.5"` in your Cargo.toml for all `dear-*` crates

Release Train 0.4 (previous)

- All crates unified to 0.4.0 across the workspace
  - Core: dear-imgui-rs 0.4.0, dear-imgui-sys 0.4.0
  - Backends: dear-imgui-wgpu 0.4.0, dear-imgui-glow 0.4.0, dear-imgui-winit 0.4.0
  - Extensions: dear-implot 0.4.0, dear-imnodes 0.4.0, dear-imguizmo 0.4.0, dear-implot3d 0.4.0, dear-imguizmo-quat 0.4.0
  - Sys crates: dear-implot-sys 0.4.0, dear-imnodes-sys 0.4.0, dear-imguizmo-sys 0.4.0, dear-implot3d-sys 0.4.0, dear-imguizmo-quat-sys 0.4.0
- dear-imgui-sys 0.4.x binds Dear ImGui v1.92.3 (docking) via cimgui
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16
- Upgrade: change `version = "0.3"` to `version = "0.4"` in your Cargo.toml for all `dear-*` crates

Release Train 0.3 (previous)

- BREAKING: Main crate renamed from `dear-imgui` to `dear-imgui-rs` (v0.3.0)
- All crates unified to 0.3.0 across the workspace
  - Core: dear-imgui-rs 0.3.0, dear-imgui-sys 0.3.0
  - Backends: dear-imgui-wgpu 0.3.0, dear-imgui-glow 0.3.0, dear-imgui-winit 0.3.0
  - Extensions: dear-implot 0.3.0, dear-imnodes 0.3.0, dear-imguizmo 0.3.0, dear-implot3d 0.3.0, dear-imguizmo-quat 0.3.0
  - Sys crates: dear-implot-sys 0.3.0, dear-imnodes-sys 0.3.0, dear-imguizmo-sys 0.3.0, dear-implot3d-sys 0.3.0, dear-imguizmo-quat-sys 0.3.0
- dear-imgui-sys 0.3.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui-rs 0.3.x layers a safe API over the 0.3.x sys crate.
- External dependencies: wgpu 26, winit 0.30.12, glow 0.16
  - dear-file-browser (preview): optional features `imgui` (pure UI) and `native-rfd` (rfd backend) enabled by default. On wasm32, prefer `native-rfd` (Web File Picker). The ImGui UI enumerates the filesystem via `std::fs` and cannot list local files in the browser environment.

Release Train 0.2 (deprecated, yanked)

- dear-imgui-sys 0.2.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui 0.2.x layers a safe API over the 0.2.x sys crate.
- Backends: wgpu (26), winit (0.30.12), glow (0.16).
- Extensions: dear-implot 0.2.x (with -sys 0.2.x), dear-imnodes 0.1.x, dear-imguizmo 0.1.x — all depend on dear-imgui/dear-imgui-sys 0.2.x.

## Upgrade Guidelines

- When bumping dear-imgui-sys to a new upstream ImGui, bump all -sys extensions (implot/imnodes/imguizmo) in lockstep and verify bindgen output/ABI.
- When bumping dear-imgui, check backends/extensions for API surface changes and update versions accordingly.
- Backend external deps (wgpu/winit/glow) often introduce breaking changes; track and bump backend crates even if core didn’t change.
