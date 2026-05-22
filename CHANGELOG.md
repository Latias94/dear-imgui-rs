# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes

- Core (`dear-imgui-rs`)
  - `TextureRef` is now `TextureRef<'tex>` for managed textures. Pass `TextureId` for legacy
    renderer-owned handles and `&mut TextureData` / `&mut OwnedTextureData` for ImGui-managed
    textures. `&TextureData` remains usable only as the legacy TexID path.
  - `TextureRef` and `create_texture_ref` no longer accept raw `u64` texture ids; use `TextureId`.
  - `TextureRef::from_raw(...)` is now `unsafe` because raw `ImTextureRef` may contain an arbitrary
    managed `ImTextureData*`.
  - `TextureData` dimensions now use `u32`, and byte counts/pitches use `usize`; oversized values
    are checked before entering Dear ImGui's signed `int` ABI.
  - `TextureData::unique_id()` and threaded snapshot texture request/feedback IDs now use opaque
    `ManagedTextureId` values instead of raw `i32` values.
  - `Context::render()` returns `&mut DrawData`; renderer backends should accept mutable draw data
    and process texture feedback through `DrawData::textures_mut()`.
  - `DrawData::textures()` and `PlatformIo::textures()` are read-only. Mutation of texture status,
    TexID, or update rectangles requires `textures_mut()` from mutable draw data/platform IO.
  - `Context::font_atlas()` now returns read-only `FontAtlasRef<'_>`. Use `font_atlas_mut()` or
    `fonts()` for font loading and atlas mutation.
  - `FontId` is now an opaque, atlas-validated, `!Send + !Sync` handle instead of a simple raw
    `ImFont*` wrapper. Existing long-lived storage remains supported, but stale or wrong-atlas
    handles panic before safe APIs call into Dear ImGui.
  - `PlotLines` / `PlotHistogram` `values_offset(...)` now takes `usize`/`PlotValueOffset` instead
    of raw `i32`.
  - Table freeze helpers now take `usize` frozen column/row counts instead of raw `i32` values.
  - Table column APIs now use `TableColumnIndex`, `TableColumnRef`, `TableHoveredColumn`, and
    `TableContextMenuTarget` instead of raw `column_n: i32` sentinel values.
  - Table/legacy-column user IDs now use `Id` / `Option<Id>` instead of raw `u32` IDs and `0`
    sentinels.
  - Table row query APIs now return `Option<TableRowIndex>` / `TableHoveredRow` instead of raw
    signed row sentinel values.
  - Table background color helpers are split by target: use `table_set_cell_bg_color*` or
    `table_set_row_bg{0,1}_color*` instead of passing `TableBgTarget` plus `-1`.
  - Legacy Columns count/index APIs now use `usize`, `OldColumnIndex`, `OldColumnRef`, and
    `OldColumnOffsetRef` instead of raw signed counts and `-1` sentinels.
  - `ListClipper` now takes `usize` item counts and returns `usize` visible indices/ranges.
  - Draw-list segment counts now use `DrawSegmentCount` / `DrawNgonSegmentCount` instead of raw
    signed `num_segments` values.
  - Draw-list channel split helpers now use `usize` channel counts/indices instead of `u32`.
  - Draw-list low-level `prim_reserve` / `prim_unreserve` counts now use `usize` instead of raw
    signed values.
  - `Io` render/window metrics getters now return `usize` counts instead of raw signed values.
  - `DrawData` total index/vertex counts now use checked `usize` getters instead of public raw
    signed fields.
  - UI key/mouse/frame-count helpers, texture unused-frame counts, and font-atlas bake discard
    frame counts now use `usize`.
  - `DockNode::depth()` now returns `usize` instead of a raw signed value.
  - Docking window class IDs now use `Id`, `Option<Id>`, and `WindowClassParentViewport` instead
    of raw `ImGuiID` sentinels.
  - `Viewport::id()` now returns `Id`, and `Viewport::parent_viewport_id()` /
    `Viewport::set_parent_viewport_id()` now use `Option<Id>` instead of raw viewport IDs.
  - `Ui::push_focus_scope` now takes `Id` instead of raw `ImGuiID`.
  - Logging helpers now take `LogAutoOpenDepth` instead of raw `-1` auto-open-depth sentinels.
  - Drag/drop flags are split by call-site domain: source APIs now use `DragDropSourceFlags`
    and target accept APIs now use `DragDropTargetFlags`. Migrate `DragDropFlags::SOURCE_*`
    constants to `DragDropSourceFlags::*` and `DragDropFlags::ACCEPT_*` constants to
    `DragDropTargetFlags::*`.
  - Hovered flags are split by call-site domain: window hover queries now use
    `WindowHoveredFlags`, item hover queries now use `ItemHoveredFlags`, and style tooltip hover
    defaults now use `TooltipHoveredFlags`.
  - `Io::mouse_down` and `Io::set_mouse_down` now take `MouseButton` instead of raw `usize`
    indices. Use `mouse_down_raw_index` / `set_mouse_down_raw_index` for explicit raw access.
  - `ButtonFlags` for `Ui::invisible_button_flags` now contains only independent behavior flags.
    Use `InvisibleButtonOptions` and `InvisibleButtonMouseButtons` to select right/middle or
    multiple mouse buttons for invisible buttons.
  - Color widget flags are split by call-site domain: color edit APIs now use `ColorEditFlags`,
    color picker APIs now use `ColorPickerFlags`, and color button APIs now use
    `ColorButtonFlags`. Migrate picker-only flags such as `NO_SIDE_PREVIEW` to
    `ColorPickerFlags`, and button-only flags such as `NO_BORDER` to `ColorButtonFlags`.
  - Input widget flags are split by call-site domain: single-line text inputs keep
    `InputTextFlags`, multiline text inputs now use `InputTextMultilineFlags`, and numeric
    input/scalar builders now use `InputScalarFlags`. The resize callback bit is crate-owned
    plumbing and is no longer accepted as a public input flag.
  - Popup flags are split by call-site domain: popup open helpers now use `PopupOpenFlags`,
    context popup helpers/options now use `PopupContextFlags`, and popup-open query helpers now
    use `PopupQueryFlags`.
  - Drag builders no longer accept `SliderFlags` through an implicit conversion; use `DragFlags`
    directly for drag widgets.
  - Style tree-line configuration now uses the dedicated `TreeLineMode` type:
    `Style::tree_lines_mode()` / `Style::set_tree_lines_mode(...)` replace
    `Style::tree_lines_flags()` / `Style::set_tree_lines_flags(...)`.
  - `StateStorageToken` now carries both the active `Ui` lifetime and the pushed storage lifetime.
- Extensions
  - `dear-file-browser` scan request/batch/status generations now use `ScanGeneration`,
    type-to-select timeout config uses `Duration`, and thumbnail frame counters return
    `ThumbnailFrameIndex` instead of raw integers.
  - `dear-file-browser` caller-facing UI configuration now lives in
    `FileDialogUiState::config` / `FileDialogUiConfig`; update direct `state.ui.*` config field
    access such as `state.ui.file_list_view`, `state.ui.toolbar`, `state.ui.thumbnails_enabled`,
    and `state.ui.file_list_columns` to `state.ui.config.*`. The new-folder operation buffers and
    type-to-select runtime buffer are internal UI implementation state.
  - `dear-file-browser` transient path/footer/breadcrumb/search/error state and modal operation
    buffers for rename, delete, paste, and places are now internal `FileDialogUiRuntime` /
    `FileDialogOperationState` fields instead of top-level `FileDialogUiState` fields.
  - ImPlot, ImPlot3D, node-editor, and ImNodes RAII tokens that call extension `End`/`Pop` functions
    on drop are now UI/current-context scoped and intentionally `!Send + !Sync`.
  - `dear-implot` colormap helpers now use typed `ColormapIndex`, `ColormapColorIndex`, and
    `ColormapSelection` values. `push_colormap` returns an RAII token instead of requiring
    `pop_colormap(count)`. Colormap indices/counts/sizes use `usize` at the Rust API boundary.
  - `dear-implot` / `dear-implot3d` plot data layout APIs now use typed
    `PlotDataLayout` / `Plot3DDataLayout` values with typed sample offsets and byte strides.
    Use `PlotDataStride::AUTO` / `Plot3DDataStride::AUTO` for the default stride.
  - `dear-implot::SubplotGrid::new` now takes `usize` row/column counts instead of raw signed
    counts.
  - `dear-implot` histogram bin APIs now use `HistogramBins`, `usize` bin counts, or `BinMethod`
    instead of raw signed bin selectors.
  - `dear-implot` / `dear-implot3d` range-based axis tick helpers now take `usize` tick counts
    instead of raw signed `i32` values.
  - `dear-implot` drag point/line helpers now take `DragToolId` instead of raw `i32` ids.
  - `dear-implot` heatmap row/column counts are checked before FFI and reject values outside
    ImPlot's `i32` range.
  - ImPlot3D `Plot3DBuilder` and `Plot3DToken` now carry the originating frame lifetime.
  - `dear-implot3d::Plot3DContext::current()` now exposes a non-owning current-context wrapper,
    matching the `dear-implot` current-context API shape for advanced callers.
  - `dear-implot3d` style and colormap helpers now use typed `Plot3DStyleVar`,
    `Plot3DColorElement`, `Colormap`, `ColormapIndex`, and `ColormapColorIndex` values instead of
    raw `i32` identifiers. Colormap indices/counts/sizes use `usize`, and style/color/colormap push
    helpers return RAII tokens instead of requiring manual `pop_*` count calls.
  - `dear-implot` / `dear-implot3d` no longer expose raw `push_style_var_i32` helpers. Use
    `push_style_var_f32`, `push_style_var_vec2`, or `dear-implot3d::push_style_var_marker`.
  - `dear-implot3d` surface grid counts now use `usize` and reject values outside ImPlot3D's
    `i32` range before FFI.
  - `dear-imguizmo::graph::GraphStyle::grid_major_every` now uses a positive typed
    `GraphGridMajorInterval` instead of a raw `i32`.
  - `dear-imnodes` node, pin, and link APIs now use `NodeId`, `PinId`, and `LinkId` instead of raw
    `i32` ids.
  - `dear-imnodes` style-var helper methods now take typed `StyleVar` values instead of raw `i32`
    style-var indices.
  - `dear-imnodes::NodeEditor::set_alt_mouse_button` now takes `MouseButton` instead of a raw
    button index.
  - `dear-node-editor::EditorConfig` no longer exposes the raw unchecked mouse-button index
    escape hatches; use the typed `MouseButton` setters.
  - `dear-imgui-test-engine::TestScript::mouse_click_on_void` now takes `MouseButton` instead of a
    raw `i32` button index.
  - `dear-imgui-test-engine` table column script helpers now take typed table column indices/targets
    instead of raw signed column indices.
  - `dear-imgui-test-engine` script repeat/wait frame counts now use `ScriptCount` instead of raw
    `i32` values.
  - `dear-imgui-test-engine` `item_open_all` / `item_close_all` depth and pass limits now use
    `ScriptLimit` instead of raw `-1` sentinels.
  - `dear-imgui-test-engine::ResultSummary` count fields now use `usize` instead of raw signed
    values.
  - Deprecated `dear-imguizmo::GizmoUi::set_id(i32)` was removed; use RAII `push_id(...)` instead.
  - `dear-imguizmo::GizmoUi::get_id_*` helpers now return `dear_imgui_rs::Id` instead of raw
    `ImGuiID`.
- Backends and utilities
  - `dear-app::DearAppError`, `dear_imgui_wgpu::RendererError`,
    `dear_imgui_glow::{InitError, RenderError}`, and `dear_imgui_ash::RendererError` are now
    `#[non_exhaustive]` and include more specific variants for common startup, render, resource,
    and invalid-state failures.
  - `dear-imgui-sys::backend_shim::{dx11,opengl3,sdlrenderer3}` render entry points now take
    `*mut ImDrawData` instead of `*const ImDrawData`, matching the official Dear ImGui backend
    signatures and texture-feedback mutation model.
  - `dear-imgui-sdl3::{render,canvas_render}` now take `&mut DrawData` instead of `&DrawData`.
  - `dear-app::GpuApi` texture registration now returns and accepts `TextureId` instead of raw
    `u64` ids.
  - `dear-imgui-wgpu` texture manager and external texture helpers now return and accept
    `TextureId` instead of raw `u64` texture IDs.
  - `dear-imgui-glow::TextureMap` texture registration/update dimensions now use `u32`.

### Changed

- Core and extension current-context binding/drop policy now follows the safe API compatibility
  policy described in `docs/COMPATIBILITY.md`; per-method migration details live in this changelog.
- `FontAtlas::clear`, `FontAtlas::clear_fonts`, and `FontAtlas::remove_font` now invalidate
  existing `FontId` handles from that atlas. `Ui::push_font`, `Context::push_font`,
  `DrawListMut::add_text_with_font`, and `Ui::push_font_with_size` validate font/atlas membership
  before entering FFI.
- `OwnedDrawData::from(&mut DrawData)` remains available as a non-thread-safe migration bridge, but
  detached texture-request workflows should use `render::snapshot::FrameSnapshot`.
- Public flag wrapper serde deserialization now preserves unknown/raw bits instead of truncating
  them, keeping later validation and diagnostics consistent with direct `from_bits_retain` use.
- Raw-state flag getters now preserve unknown/raw bits instead of truncating them when reading
  Dear ImGui state back through safe flag wrappers.
- Combo box flags/options now live in the `widget::combo` module implementation while preserving
  the existing public re-exports.
- Tree node flags and table option/column flag types now live in their domain modules
  (`widget::tree` and `widget::table`) while preserving the existing public re-exports.
- Refactored internal module organization across core APIs, extensions, and backends to improve
  maintainability while preserving public API compatibility.
- `dear-imgui-sdl3` manual init/new-frame/shutdown helpers now bind the provided `Context` before
  calling the official SDL3 backend. For new code, prefer the RAII backend owner types.

### Added

- Backends
  - Added the experimental `dear-imgui-bevy` backend proof for Bevy `0.19.0-rc.2`, including
    engine-managed ImGui frame scheduling, Bevy input mapping, Bevy-native WGPU rendering,
    texture feedback and `Handle<Image>` interop, plus simple, editor-shell, and ecosystem
    composition examples.
  - `dear-imgui-sdl3` now provides `Sdl3PlatformBackend`, `Sdl3OpenGl3Backend`, and
    `Sdl3RendererBackend` owner types that pair official backend initialization and shutdown,
    bind per-frame/event/device calls to the captured `Context`, and skip drop-time FFI if the
    context has already been destroyed.

### Fixed

- Core (`dear-imgui-rs`)
  - Scope clipboard callback reentrancy protection to each `ClipboardContext` instead of using a
    process-wide guard. Same-context reentry still fails closed, while callbacks for independent
    ImGui contexts no longer block each other.
  - `Viewport::from_raw()` and `Viewport::from_raw_mut()` now assert non-null pointers before
    forming references.
- Backends
  - `dear-imgui-glow`: `GlowRenderer::destroy()` now clears renderer-owned multi-viewport state
    for this renderer, matching `Drop` and preventing stale callbacks from borrowing a destroyed
    renderer when users forget to disable multi-viewport support first.
  - `dear-imgui-glow`: registered texture updates now resolve `TextureId` through the renderer
    texture map before touching OpenGL, and the convenience registration/update path handles
    `TextureFormat::Alpha8` by expanding to RGBA.
  - `dear-imgui-wgpu`, `dear-imgui-ash`, and `dear-imgui-glow`: multi-viewport shutdown helpers now
    destroy platform windows before uninstalling renderer callbacks, so renderer-owned viewport
    resources can be released by `Renderer_DestroyWindow`.
  - `dear-imgui-sdl3`: combined platform+renderer initialization now shuts down the SDL3 platform
    backend if the OpenGL3 or SDLRenderer3 renderer initialization step fails, avoiding partially
    initialized official backend state.
  - `dear-imgui-sys::backend_shim` raw modules now document native-handle, current-context,
    call-ordering, and mutability requirements for official backend shims.
  - `dear-imgui-wgpu`: `WgpuRenderer::shutdown()` now clears renderer-owned multi-viewport state
    for this renderer, matching `Drop` and preventing stale callbacks from borrowing a shut-down
    renderer when users forget to disable multi-viewport support first.
  - `dear-imgui-wgpu`: `TextureUpdateResult::Destroyed.apply_to(...)` now sets Dear ImGui's
    destroy-next-frame precondition before marking the texture destroyed, matching the Ash backend
    helper behavior.
  - `dear-imgui-wgpu`: full texture recreation for an existing texture now creates the replacement
    before removing the old GPU texture, so a failed upload leaves the previous texture mapping
    intact.
  - `dear-imgui-ash`: draw-data texture processing now writes `TexID`/`OK` feedback only after
    upload command submission succeeds, avoiding stale texture IDs after failed Vulkan uploads.
  - `dear-imgui-ash`: Vulkan allocation and upload setup paths now clean up partially-created
    shader modules, pipelines, textures, staging buffers, descriptor sets, fences, command buffers,
    and allocator resources on failure. Existing textures and mesh buffers are kept until
    replacements are ready.
  - `dear-imgui-ash`: Winit and SDL3 multi-viewport swapchain recreation now builds replacement
    swapchain resources before destroying the old viewport resources, so failed resize or present
    recovery leaves the previous viewport state intact.
- Extensions
  - `dear-imgui-test-engine`: check the bound ImGui context liveness before safe engine methods call
    into FFI, so stale context use panics in Rust instead of reaching the upstream test engine.

## [0.13.0] - 2026-05-15

This release upgrades the Dear ImGui stack, removes the normal-build dependency
on LLVM/libclang, adds native `imgui-node-editor` bindings, and continues the
safe API hardening work started in the previous releases. The detailed
`v0.12.0..HEAD` history is grouped below by user-visible effect rather than by
individual validation commit.

### Highlights

- Upgrade the core stack to Dear ImGui v1.92.8 (docking) and refresh the
  extension sys crates against the same baseline.
- Normal source builds now use checked-in pregenerated bindings by default.
  LLVM/libclang is only needed when explicitly regenerating bindings with
  `DEAR_IMGUI_RS_REGEN_BINDINGS=1 --features bindgen`.
- Add native-only `dear-node-editor` / `dear-node-editor-sys` support backed by
  `cimnodes_editor` / `imgui-node-editor`, including examples modeled after the
  upstream basic-interaction and blueprints-style demos.
- Add the stack layout compatibility path required by node-editor blueprints and
  expose safe `begin_horizontal`, `begin_vertical`, and `spring` helpers.
- Harden the safe API across texture ownership, input text buffers, draw-list
  cloning, table/docking/widget preconditions, callback lifetimes, and
  multi-context ownership.
- Fix multi-context and multi-viewport backend paths so Winit/WGPU/Glow/Ash
  state, callbacks, and teardown target the intended ImGui context.

### Breaking Changes

- Core (`dear-imgui-rs`)
  - User texture registration now tracks `OwnedTextureData` lifetimes from the
    safe API. Raw `TextureData` registration moved to explicit `unsafe` raw
    helpers.
  - Font atlas/config loader setters require `&'static FontLoader`, and
    `FontConfig::glyph_exclude_ranges(...)` now accepts inclusive `(start, end)`
    ranges matching Dear ImGui's upstream contract.
  - Infallible `TextureId` conversions into `usize` or raw pointers were
    removed. Use `try_as_usize()`, `try_as_ptr()`, or `try_as_mut_ptr()`.
  - The duplicate read-only draw-list snapshot types in `draw` were removed;
    use the unified `render` draw-data types instead.
  - `WindowFlags::DOCK_NODE_HOST` was removed because Dear ImGui treats it as an
    internal `Begin`/`NewFrame` flag.
  - `TextCallbackData` is now bound to the input-text callback frame lifetime so
    callback handlers cannot retain it after Dear ImGui returns.
  - `PlatformIo` aggregate-return callbacks now use out-parameters, and typed
    callback setters must be installed on the active context's `PlatformIo`.
    Cross-context installation panics instead of splitting the C callback table
    from Rust callback storage.
  - `Viewport::flags()` / `set_flags(...)` now use typed `ViewportFlags`.
    Backend code that must manipulate raw bits should use `raw_flags()` or
    `unsafe set_raw_flags_unchecked(...)`.
  - `WindowClass` now carries typed viewport, tab item, dock node, and platform
    icon fields. Struct literals should add the new fields or switch to
    `WindowClass::new` / `Default`.
  - `set_item_key_owner{,_with_flags}` now returns Dear ImGui's ownership-request
    result, matching Dear ImGui v1.92.8.
  - `DrawCmd` gained `SetSamplerLinear` and `SetSamplerNearest`; exhaustive
    draw-command matches must handle the new variants.
- Core (`dear-imgui-sys`)
  - Stop exposing cimgui's `ImGuiPlatformIO_Set_Platform_GetWindowPos` and
    `ImGuiPlatformIO_Set_Platform_GetWindowSize` helpers from generated bindings.
    Use the repository-owned `*_OutParam` wrappers instead; they do not consume
    `BackendLanguageUserData`.
- Backends
  - Change `dear-imgui-winit::multi_viewport::shutdown_multi_viewport_support`
    to take `&mut Context`, matching the renderer backends and making shutdown
    target an explicit ImGui context.
- Extensions
  - Change `dear-implot::PlotContext::current()` to an explicit `unsafe`
    non-owning wrapper. Code that owns an ImPlot context should use
    `PlotContext::create(...)`; callers that borrow a raw current context must
    acknowledge the raw lifetime and ownership contract.
  - Change `dear-implot` APIs that accept arbitrary ImPlot axes to use typed
    `Axis` / `YAxis` values instead of raw `i32` indices. Raw-axis calls remain
    available through explicit `unsafe *_unchecked` escape hatches.

### Added

- Extensions
  - Add native-only `dear-node-editor` and `dear-node-editor-sys` support backed
    by `cimnodes_editor` / `imgui-node-editor`.
  - Add safe node-editor APIs for editor contexts, typed config/style values,
    node/pin/link scopes, create/delete sessions, selection mutation, queries,
    selection/action-context counts, group hints, node background draw lists,
    shortcut toggles, styled create/reject feedback, and RAII-scoped editor
    suspension.
  - Add `node_editor_basic`, a compact Winit + WGPU example modeled after the
    upstream basic-interaction example.
  - Add `node_editor_showcase`, a blueprints-style Winit + WGPU example using
    the stack layout helpers and a Rust `BlueprintNodeBuilder`.
  - Expose new ImGuizmo handle/move-type queries and custom grid/axis drawing
    helpers through `MoveType`, `GizmoUi::active_handle_type`,
    `hovered_handle_type`, `active_move_type`, `hovered_move_type`,
    `draw_axes`, `draw_grid_custom`, and `draw_grid_custom_color`.
- Core (`dear-imgui-sys`)
  - Add the native stack layout compatibility layer required by the node-editor
    blueprints example, including repository-owned `dear_imgui_stack_*` C ABI
    symbols and build-time `ItemSize()` / `ItemAdd()` hooks in the generated
    `imgui.cpp` build copy.
- Core (`dear-imgui-rs`)
  - Add stack layout helpers used by the node-editor blueprints showcase:
    `begin_horizontal`, `begin_vertical`, and `spring`.
  - Add RAII-scoped helpers for button-repeat and draw-list texture stacks:
    `Ui::push_button_repeat_token`, `Ui::with_button_repeat`,
    `DrawListMut::push_texture_token`, and `DrawListMut::with_texture`.
  - Expose Dear ImGui v1.92.8 style additions:
    `StyleColor::CheckboxSelectedBg`, `StyleVar::DragDropTargetRounding`,
    `Style::drag_drop_target_rounding`, `drag_drop_target_border_size`,
    `drag_drop_target_padding`, and `color_marker_size`.
  - Expose additional `Style` accessors and `StyleVar` variants for scrollbar
    padding, image borders, tab sizing/borders, angled table headers, tree-line
    styling, separator text styling, and docking separator size.
  - Expose Dear ImGui v1.92.8 draw-list helpers `DrawListMut::add_line_h` and
    `DrawListMut::add_line_v`.
  - Expose raw `PlatformIo` accessors for standard draw callbacks:
    `DrawCallback_ResetRenderState`, `DrawCallback_SetSamplerLinear`, and
    `DrawCallback_SetSamplerNearest`.

### Changed

- Workspace
  - Refresh `dear-implot-sys`, `dear-implot3d-sys`, `dear-imnodes-sys`,
    `dear-imguizmo-sys`, `dear-imguizmo-quat-sys`, and
    `dear-imgui-test-engine-sys` submodules and pregenerated native/WASM
    bindings against the Dear ImGui v1.92.8 stack.
- Core (`dear-imgui-rs`)
  - Upgrade the core Dear ImGui/cimgui baseline to Dear ImGui v1.92.8 (docking).
  - Store typed `PlatformIo` callbacks per active `ImGuiContext` instead of in
    process-wide Rust slots, while preserving the `dear-imgui-sys`
    out-parameter shim path for aggregate-return callbacks.
  - Track `DrawListMut` borrows per raw `ImDrawList*` on the current thread
    instead of using process-wide locks, and resolve background/foreground draw
    lists against the main viewport.
  - Make `DrawListMut::clone_output()` return `render::OwnedDrawList`.
- Core (`dear-imgui-sys`)
  - Use checked-in pregenerated bindings by default for normal source builds, so
    users can build without installing LLVM/libclang unless they explicitly
    regenerate bindings. Fixes #28, thanks @dtugend.
  - Make the `bindgen` build dependency optional behind a `bindgen` feature.
    Binding regeneration now requires both `DEAR_IMGUI_RS_REGEN_BINDINGS=1` and
    `--features bindgen`.
  - Compile the stack layout compatibility hooks by default for normal native
    builds, and reject older prebuilts unless their manifest declares
    `features=stack-layout`.
- Extensions
  - Use checked-in pregenerated bindings by default for extension `*-sys` normal
    builds, and make their `bindgen` build dependencies optional behind
    `bindgen` features.
  - Clarify `dear-imguizmo-quat` global static setting semantics and add
    `GizmoQuatSettings` / `GizmoQuatSettingsToken` for temporary restoration of
    getter-backed sensitivity, scale, flip, and reverse settings.
- Backends
  - Register Dear ImGui v1.92.8 standard reset/sampler draw callbacks in the
    WGPU and Glow renderers. WGPU now keeps both linear and nearest sampler bind
    groups; Glow switches texture filtering when sampler callbacks are
    encountered.
  - Route Winit multi-viewport aggregate-return callbacks through the
    out-parameter shim path and bind setup, event routing, shutdown, and callback
    cleanup to the provided `Context`.

### Fixed

- Core (`dear-imgui-rs`)
  - Expand FFI precondition validation across textures, fonts, draw lists,
    tables, legacy columns, docking, windows, child windows, widgets, popups,
    combos, tabs, color editors, sliders, drags, scalar arrays, drag/drop
    payloads, IO values, style values, hover/focus flags, viewport flags, and
    runtime geometry. Invalid values are rejected in safe Rust before reaching
    Dear ImGui assertions or unchecked indexing paths.
  - Expose child-window builder, token, and `ChildFlags` through public
    `dear_imgui_rs::*` paths so `Ui::child_window(...).child_flags(...)` is
    usable from downstream crates. Fixes #29, thanks @CoffeeCatRailway.
  - Add missing public `ConfigFlags`, `BackendFlags`, and `TreeNodeFlags` bits,
    including `ConfigFlags::NO_KEYBOARD`,
    `BackendFlags::HAS_PARENT_VIEWPORT`, tree-line flags, and span/overlap tree
    aliases; align `TreeNodeFlags::COLLAPSING_HEADER` with Dear ImGui's
    upstream flag combination.
  - Make image builder tint and border colors take effect on Dear ImGui v1.92
    by routing tinted images through `ImageWithBg` and applying temporary image
    border style overrides.
  - Keep texture registration, `Context`, and `OwnedTextureData` lifetimes tied
    together so safe Rust cannot produce dangling texture FFI calls.
  - Harden `String` / `ImString` input buffers and input-text callback data,
    including invalid UTF-8 repair, initialized-length tracking, default
    character-filter behavior, cursor/selection validation, and callback-frame
    lifetimes.
  - Reject safe draw-list and draw-data cloning when command buffers contain
    user callbacks, avoiding duplicated opaque callback userdata pointers.
  - Keep multi-select scopes, table draw channels, draw-list channels, font
    stacks, clip-rect stacks, texture stacks, button-repeat state, wrapped text,
    and temporary context binding paths balanced during panic unwinding.
  - Resolve context-owned IO, font atlas, style, viewport, platform-window,
    font-stack, ini settings, clipboard, and `PlatformIo` operations through the
    receiver context instead of whichever context is currently bound.
  - Reject frame lifecycle calls on a non-current `Context`, preventing
    `frame`, `render`, or `draw_data` from accidentally operating on a different
    active context.
  - Clear typed `PlatformIo`, renderer callback, and out-parameter shim storage
    from the receiver context, and clear stale typed slots when raw setters
    replace the corresponding C callbacks.
  - Generate correctly terminated glyph exclude range arrays and reject reversed
    or out-of-range glyph ranges.
- Core (`dear-imgui-sys`)
  - Regenerate native and WASM pregenerated bindings for Dear ImGui v1.92.8 via
    cimgui `docking_inter`.
  - Route Rust-owned `Platform_GetWindowPos` / `Platform_GetWindowSize`
    out-parameter shims through `dear-imgui-sys` storage instead of cimgui's
    `BackendLanguageUserData` helper, avoiding collisions with language/backend
    userdata.
  - Avoid unresolved `PlatformIO` out-parameter shim symbols in builds that
    intentionally skip native C++ hook compilation, while keeping callback
    installation available for normal native builds.
  - Report unavailable `PlatformIO` out-parameter hooks through a capability flag
    and explicit callback-installation panic instead of leaving raw external
    symbols unresolved.
- Backends
  - Fix WGPU window-handle handling and SDL3 close-event behavior.
  - Make Winit multi-viewport main viewport initialization idempotent and guard
    `PlatformUserData` access so the backend does not overwrite or read another
    backend's data.
  - Resolve Winit IME userdata through the `ImGuiContext*` passed to
    `Platform_SetImeDataFn`, preventing cross-context IME userdata lookups.
  - Bind WGPU multi-viewport renderer callback installation and teardown to the
    provided `Context` in both the Winit and SDL3 backends.
  - Add explicit-context WGPU render entry points and clear temporary
    `PlatformIO.Renderer_RenderState` through RAII so draw callbacks do not see
    stale renderer state after early render errors.
  - Keep WGPU, Glow, and Ash multi-viewport renderer/global state per ImGui
    context instead of in process-wide slots.
- Extensions
  - Reject invalid `dear-implot` axis indices, non-finite plot
    sizes/coordinates, invalid axis limits, invalid tick ranges, and invalid
    zoom constraints before safe Rust crosses into ImPlot FFI.
  - Require `dear-imguizmo` `IdToken` to be dropped while its original ImGui
    context is current, preventing the token from silently switching Dear
    ImGui's global current context during cleanup.
  - Bind `dear-implot` `PlotUi` and `PlotToken` operations to the `PlotContext`
    and ImGui context that created them, preventing multi-context applications
    from accidentally plotting through whichever ImPlot context is current.
  - Treat `dear-implot::PlotContext::current()` as non-owning so dropping the
    wrapper cannot destroy the process current ImPlot context.
  - Bind `dear-implot3d` per-frame plotting APIs to the `Plot3DContext` and
    ImGui context that created them, preventing multi-context applications from
    accidentally plotting through whichever ImPlot3D context is current.
  - Bind `dear-imnodes` frame/token operations to the active ImGui context and
    restore the previous Dear ImGui and ImNodes current contexts after
    context/editor cleanup, preventing hidden current-context switches during
    multi-context use.
  - Reset `dear-imnodes` to the default editor context when `NodeEditor` is
    opened without an explicit `EditorContext` or when the current explicit
    editor is dropped, preventing stale editor pointers after using a custom
    editor.

## [0.12.0] - 2026-05-09

This release removes the last invalid `Condition` value from the safe API and
splits several overly broad flag sets into type-safe options so Dear ImGui's
runtime assertions can no longer be triggered from normal Rust code. It also
aligns the unified release train to `0.12.0` across the workspace.

### Highlights

- Remove `Condition::Never` and make APIs that previously accepted invalid or
  overloaded condition/flag values use explicit option types instead.
- Split the combo, table, color, tab, popup, multi-select, shortcut routing,
  drag/drop, and drag/slider APIs so mutually exclusive Dear ImGui choices
  cannot be combined accidentally.
- Update `dear-imgui-reflect` so numeric widget helpers route slider and drag
  flags through the correct safe wrapper types.

### Breaking Changes

- Core (`dear-imgui-rs`)
  - Remove `Condition::Never`.
  - Change `Window::content_size(...)` to accept only the size vector.
  - Replace several public flag arguments with explicit option types or narrower flag sets:
    `DragDropPayloadCond`, `ShortcutFlags`, `ShortcutOptions`,
    `ShortcutRoute`, `NextItemShortcutFlags`, `NextItemShortcutOptions`,
    `ItemKeyOwnerFlags`, `DragFlags`, `ComboBoxOptions`, `TableOptions`,
    `TableColumnWidth`, `TableColumnIndent`, `TableColumnStateFlags`, `ColorEditOptions`,
    `ColorPickerOptions`, `ColorButtonOptions`, `TabBarOptions`,
    `TabItemOptions`, `PopupContextOptions`, `MultiSelectOptions`,
    `MultiSelectBoxSelect`, `MultiSelectScopeKind`, `DrawCornerFlags`, and `PolylineFlags`.
  - Update builder/setup structs to store typed options directly, including
    `ComboBox::options`, `TableColumnSetup::width`, and `TableColumnSetup::indent`.
  - Remove the ambiguous `TableColumnSetup::init_width_or_weight()` helper in favor of
    `fixed_width()` and `stretch_weight()`.
- Extensions
  - `dear-imgui-reflect`: numeric `wrap_around` now applies to drag widgets only.

### Migration Guide

- Replace `Condition::Never` with the nearest valid intent, usually `Condition::Always`,
  `Condition::Once`, or `Condition::FirstUseEver`.
- For one-choice flag domains, move the choice into the dedicated option type instead of
  OR-ing unrelated bits together.
  ```rust
  // before
  // ui.shortcut_with_flags(chord, ShortcutFlags::REPEAT | ShortcutFlags::ROUTE_GLOBAL);
  //
  // after
  ui.shortcut_with_flags(
      chord,
      ShortcutOptions::new()
          .flags(ShortcutFlags::REPEAT)
          .route(ShortcutRoute::Global(ShortcutGlobalRouteFlags::NONE)),
  );
  ```
- Update code that passed mixed flag domains to the new typed options instead of OR-ing
  everything into one flag value. The main new types are `ShortcutOptions`,
  `ShortcutRoute`, `TableColumnIndent`, `MultiSelectClickPolicy`,
  `MultiSelectBoxSelect`, `MultiSelectScopeKind`, `DrawCornerFlags`, and
  `PolylineFlags`.
- For rounded rectangles and image corners, use `DrawCornerFlags`. For closed paths,
  set `PolylineBuilder::closed(true)` or pass `PolylineFlags` only to polyline/path APIs.
- For table columns, move width and indentation into the typed helpers instead of
  encoding them into `TableColumnFlags`.
  ```rust
  // before
  // ui.table_setup_column("Name", TableColumnFlags::INDENT_ENABLE, Some(140.0), 0);
  //
  // after
  ui.table_setup_column_with_indent(
      "Name",
      TableColumnFlags::NONE,
      Some(TableColumnWidth::Fixed(140.0)),
      Some(TableColumnIndent::Enable),
      0,
  );
  ```
- Replace `TableColumnSetup::init_width_or_weight()` with `fixed_width()` or
  `stretch_weight()`, and move indentation to `TableColumnSetup::indent(...)` or
  `indent_enabled(...)`.
- When touching shortcut routing or multi-select, split the former combined flags into
  the dedicated option types before passing them into the API.

### Changed

- Backends
  - Update the default WGPU 29 dependency family to `29.0.3`, SDL3 bindings to
    `sdl3` `0.18.3` / `sdl3-sys` `0.6.5+SDL-3.4.8`, and the WASM binding stack
    to `wasm-bindgen` `0.2.121` / `web-sys` and `js-sys` `0.3.98`.
  - Re-export the selected WGPU crate as `dear_imgui_wgpu::wgpu` so tests and
    downstream integrations can name the exact WGPU major chosen by
    `dear-imgui-wgpu`'s feature flags.

### Fixed

- Core (`dear-imgui-rs`)
  - Fixes #27, thanks @belst. Remove the invalid `Condition::Never` value that could reach
    Dear ImGui's assertion paths, and tighten the APIs that previously exposed unsupported
    condition values in helper builders.
  - Split tab bar fitting policy, table column indent policy, context popup mouse button,
    and multi-select click/box/scope policies out of their corresponding public flag sets so
    normal Rust code cannot combine invalid single-choice flag values or request `NavWrapX`
    outside Dear ImGui's supported window scope.
  - Split shortcut routing policy out of `ShortcutFlags` and
    `NextItemShortcutFlags`, and type global-only routing modifiers separately,
    so normal Rust code cannot combine route modes that Dear ImGui rejects.
  - Split rectangle corner rounding and polyline/path-stroke flags into
    `DrawCornerFlags` and `PolylineFlags`, so `ImDrawFlags_Closed` can no
    longer be passed to rounded rectangle APIs that reject it.
- Extensions
  - `dear-imgui-reflect`: route slider and drag numeric flags through distinct helper methods
    so the derived widgets no longer emit invalid flag combinations for Dear ImGui.

## [0.11.0] - 2026-04-07

Upgrade to Dear ImGui v1.92.7 (docking branch) via cimgui `docking_inter`, refresh
`cimplot` / `cimplot3d`, regenerate native + import-style WASM bindings, and expand
the safe Rust API around the latest ImPlot / ImPlot3D spec-based item styling.
This release also formalizes the repository-owned `backend_shim` surface in
`dear-imgui-sys`, adds repository-local iOS / Android smoke examples, and
simplifies publishing around the Python release scripts.

### Highlights

- Upgrade the core stack to Dear ImGui v1.92.7 and refresh the vendored
  `cimgui` / `cimplot` / `cimplot3d` submodules with regenerated native + WASM bindings.
- Introduce the repository-owned `dear-imgui-sys::backend_shim` surface for official backend glue,
  including new SDLRenderer3 support and clearer low-level backend ownership.
- Expand the ImPlot / ImPlot3D safe APIs around spec-backed item styling,
  per-index array styling, color enums, and the remaining builder gaps.
- Add repository-local iOS and Android smoke examples covering
  `dear-imgui-winit + dear-imgui-wgpu`, `dear-imgui-sdl3 + dear-imgui-wgpu`,
  and low-level Android `NativeActivity` + EGL / GLES integration shapes.
- Simplify release operations by moving `dear-imgui-build-support` onto the unified
  `0.11.0` train and removing the `release-plz` workflow in favor of the Python publishing scripts.

### Breaking Changes

- Core (`dear-imgui-sys`)
  - Replace the provisional `raw_backend::{win32, dx11, android, opengl3}` surface with
    `backend_shim::{win32, dx11, android, opengl3}` behind `backend-shim-*` feature gates.
    Consumers using the old low-level sys surface must migrate to the new repository-owned shim ABI.
- Extensions
  - `dear-imnodes`: remove the deprecated `EditorContext` methods that relied on the global current ImNodes context, as well as `EditorContext::create/try_create`. Use `Context::{create_editor_context,try_create_editor_context}` and `Context::bind_editor(&editor)` instead.

### Added

- Core (`dear-imgui-rs`)
  - Expose the new Dear ImGui v1.92.7 surface in the safe API with `Ui::tree_node_get_open()`, `Viewport::debug_name()`, `StyleVar::SeparatorSize`, `ButtonFlags::ALLOW_OVERLAP`, and updated `MultiSelectFlags` names/compatibility aliases for the upstream `SelectOnAuto` rename.
- Core (`dear-imgui-sys`)
  - Extend that backend shim surface with feature-gated `backend_shim::sdlrenderer3` support for Dear ImGui's official SDLRenderer3 backend, including SDL3 header discovery for both system-provided SDL3 installs and `sdl3-sys` build-from-source outputs (PR #24, thanks @flowkclav).
- Extensions
  - `dear-implot`: add safe `PlotUi::plot_polygon()` / `PlotBuilder::polygon()` wrappers for upstream `PlotPolygon`, plus the new `PolygonFlags`.
  - `dear-implot`: add unified ImPlot v0.18 spec-backed item styling helpers across all `ImPlotSpec`-backed plot builders via `PlotItemStyle` / `PlotItemStyled`, including direct builder methods such as `with_line_color`, `with_fill_alpha`, `with_marker`, and `with_size`. This closes the high-level Rust API gap where plot item color/alpha styling was available in the C bindings but not exposed consistently by the safe layer (addresses #26, thanks @sstscrypto).
  - `dear-implot`: add `PieChartFlags::NO_SLICE_BORDER` plus closure-scoped `PlotItemArrayStyle` / `with_next_plot_item_array_style(...)` helpers for the new upstream per-index `ImPlotSpec` arrays (`LineColors`, `FillColors`, `MarkerSizes`, `MarkerLineColors`, `MarkerFillColors`) without introducing dangling pointer hazards into the safe API.
  - `dear-implot3d`: add unified `ImPlot3DSpec`-backed item styling helpers across spec-backed plot builders via `Plot3DItemStyle` / `Plot3DItemStyled`, covering both `plots::*` items and `Plot3DUi` builders such as `surface_f32()`, `image_by_axes()`, and `mesh()`. Also expose `Marker3D::Auto` so the safe API can round-trip ImPlot3D's default automatic marker selection.
  - `dear-implot3d`: add typed `Plot3DColorElement` values for style colors, including the new axis background slots, plus closure-scoped `Plot3DItemArrayStyle` / `with_next_plot3d_item_array_style(...)` helpers for the new upstream per-index `ImPlot3DSpec` arrays.
  - `dear-implot3d`: wire `Item3DFlags` into spec-backed plot wrappers/builders via `with_item_flags(...)`, so common `NO_LEGEND` / `NO_FIT` flags can now be composed from the safe API instead of remaining a defined-but-unreachable flag set.
- Backends
  - `dear-imgui-sdl3`: add optional `sdlrenderer3-renderer` support and wrapper APIs for the official SDL3 + SDLRenderer3 path, including `init_for_canvas` / `canvas_new_frame` / `canvas_render` / `shutdown_for_canvas`.
- Examples
  - Add a standalone repository-local `examples-android/dear-imgui-android-smoke` Android template that demonstrates the low-level `dear-imgui-rs` + `dear-imgui-sys` route without introducing a new published crate or changing the workspace's default build matrix.
  - Add minimal `cargo-apk2` packaging metadata to the Android smoke template and verify that it can produce a signed debug `NativeActivity` APK for `aarch64-linux-android`.
  - Add a repository-local APK packaging helper for the Android smoke template and document release signing plus per-ABI APK packaging while keeping the checked-in smoke path single-ABI and repository-local.
  - Add a standalone repository-local `examples-ios/dear-imgui-ios-smoke` example that demonstrates a `dear-imgui-winit + dear-imgui-wgpu` iOS integration shape, including XCFramework packaging helpers and a checked-in Xcode host stub for simulator/device validation.
  - Add a standalone repository-local `examples-ios/dear-imgui-ios-sdl3-smoke` example that demonstrates a `dear-imgui-sdl3 + dear-imgui-wgpu` iOS integration shape, including a checked-in Xcode host stub and an SDL3 framework helper that can consume app-owned framework artifacts or build `SDL3.framework` from the upstream `sdl3-src` source tree.
  - Add an `sdl3_sdlrenderer` example plus the `sdl3-sdlrenderer3` example feature for Dear ImGui on SDL3 + SDL_Renderer.

### Changed

- Core (`dear-imgui-sys`)
  - Upgrade vendored `cimgui` `docking_inter` to Dear ImGui v1.92.7 and regenerate native + import-style WASM bindings.
- Core (`dear-imgui-rs`)
  - Keep backend shim feature gates in `dear-imgui-sys` only; the safe crate does not re-export backend-specific toggles until it owns corresponding safe wrappers.
- Extensions
  - `dear-implot-sys` / `dear-implot3d-sys`: refresh the vendored `cimplot` / `cimplot3d` submodules and regenerate native + WASM bindings.
  - `dear-implot` / `dear-implot3d`: initialize the new upstream `ImPlotSpec` / `ImPlot3DSpec` array fields in the safe wrapper defaults so spec-backed plots remain ABI-correct after the latest `cimplot` / `cimplot3d` updates.
  - `dear-implot3d`: adapt the safe `mesh()` wrapper to the new typed `ImPlot3D_PlotMesh_*Ptr` entry points while preserving the existing Rust-facing API shape.
  - `dear-implot`: standardize plot-item styling so `LinePlot`, `ScatterPlot`, `BarPlot`, `HistogramPlot`, `HeatmapPlot`, `ErrorBarsPlot`, `ShadedPlot`, `StairsPlot`, `StemPlot`, `TextPlot`, and other `ImPlotSpec`-based builders now share the same styling surface instead of mixing per-type convenience methods with raw style-object-only paths.
  - `dear-implot`: let `ShadedBetweenPlot` configure `offset` / `stride` like other spec-backed line-style builders, and remove an outdated comment that still described the old pre-wrapper state.
- Core (`dear-imgui-sys`)
  - Link `GLESv3` for the Android OpenGL3 backend shim so downstream Android `NativeActivity` binaries can load successfully before the application creates its own EGL / GLES context.
  - Expand the crate and module documentation around `backend_shim` so the repository-owned shim ABI, Android low-level route, and ownership split with application packaging are explicit in the main docs.
- Backends
  - Re-verify the existing backend crates against the Dear ImGui v1.92.7 / cimgui refresh. No additional backend API surface changes were required for this upstream bump.
  - `dear-imgui-wgpu`: add feature-gated support for `wgpu` v29, make `wgpu-29` the default backend path, and keep `wgpu-28` / `wgpu-27` as explicit compatibility features.
  - `dear-imgui-sdl3`: keep SDL3-specific wrapper/build ownership in the backend crate, but route the optional official OpenGL3 renderer path through `dear-imgui-sys::backend_shim::opengl3` instead of compiling a second local OpenGL3 shim layer.
  - `dear-imgui-sdl3`: stop forcing `sdl3/build-from-source` on Android from the backend crate itself. Android SDL3 acquisition now remains application-owned: consumers may either provide `SDL3_INCLUDE_DIR` or opt into `sdl3/build-from-source` in their own dependency graph.
  - `dear-imgui-sdl3`: on Apple targets, keep SDL3 acquisition application-owned instead of forcing `sdl3/build-from-source` from the backend/examples crates. macOS continues to use the system/Homebrew SDL3 path, while iOS is now documented as an app-owned framework or app-owned build-from-source route.
- Examples
  - Upgrade `examples-android/dear-imgui-android-smoke` from a startup-only smoke path to a minimal NativeActivity + EGL / GLES3 render loop that displays Dear ImGui windows on-device while preserving app-owned Android packaging and lifecycle glue, without turning the template into a published runtime crate.
  - Switch the Android smoke APK helper from a Windows-only PowerShell script to a cross-platform Python script, and tune the README screenshot presentation for GitHub rendering.
  - Document the Apple example split explicitly: keep desktop/native `cargo run` demos in `examples/`, and keep iOS/Android smoke templates in top-level `examples-ios/` / `examples-android/` folders because they require host projects, packaging steps, or mobile-specific tooling.
- Docs
  - Add Apple integration notes that explain how to use the repository-owned iOS examples as reference/teaching material without presenting them as a turn-key mobile runtime layer.
  - Add platform notes and README navigation for the new iOS/Android smoke templates, including a checked-in iOS Simulator screenshot for the SDL3 iOS smoke path.
- Tooling
  - Remove the `release-plz` release path and keep the repository's Python publishing scripts as the single source of truth for release automation and publish order.
  - Fix the `dear-imgui-test-engine-sys` pregenerated-bindings path handling so the standard bindings refresh flow works from the workspace root, and align the repository's `imgui_test_engine` update defaults with upstream `main`.

### Fixed

- Core (`dear-imgui-rs`)
  - Implement the previously placeholder `Ui::set_window_font_scale()` wrapper on top of Dear ImGui's exposed internal window/font-size state, so legacy per-window font scaling now works from the safe API instead of remaining a no-op.
  - Implement `Ui::is_any_column_resizing()` by reading the current window's legacy columns state, so it no longer always reports `false`.
  - Route `PlatformIo::{set_platform_get_window_pos*,set_platform_get_window_size*}` through cimgui's compatibility helpers instead of installing direct `ImVec2`-returning function pointers, fixing the ABI mismatch on platforms where those `ImGuiPlatformIO` callback slots are not C-compatible.
  - Add the missing `Context::platform_io()` shared accessor plus the remaining small `PlatformIo` / `Viewport` wrapper gaps around handler clearing, window DPI / changed-viewport callbacks, viewport centers, and raw platform handles, so this multi-viewport surface no longer forces callers down to `sys` for those basic operations.
- Extensions
  - `dear-implot`: wire `ErrorBarsPlot::horizontal()` to `ImPlotErrorBarsFlags_Horizontal`, add matching `horizontal()` plus `with_offset()` / `with_stride()` on `AsymmetricErrorBarsPlot`, and route simple line/scatter/shaded/stairs/stems builders through the existing single-array ImPlot C bindings instead of allocating temporary X-coordinate buffers.
  - `dear-implot`: finish aligning the remaining `Simple*Plot` builders with their full-builder counterparts by threading through the relevant plot/item flags on simple line/scatter/stem/shaded/error-bar/bar-group helpers as well.
  - `dear-file-browser`: keep the built-in view/column controls aligned with thumbnail backend availability, so the standard toolbar, IGFD-style chrome, and `Columns...` popup no longer offer thumbnail-only toggles when no thumbnail backend is attached.
  - `dear-imgui-test-engine-sys`: refresh the vendored `imgui_test_engine` submodule to the latest upstream `main` compatible with the Dear ImGui v1.92.7 upgrade and regenerate pregenerated bindings.

### CI

- Add an `apple-mobile-check` job that validates the documented iOS integration surface with `cargo check` sentinels for device and simulator targets, including the repository-local iOS smoke templates.
- Align publish/package verification with the new `dear-imgui-build-support` dependency ordering: package the helper crate first and use `cargo package --list` for the pre-release `dear-imgui-sys` smoke check so CI can validate package contents before the helper crate is indexed on crates.io.

### Dependencies

- Workspace
  - Upgrade direct dependency baselines to `bitflags` 2.11, `winit` 0.30.13, `glow` 0.17, `wasm-bindgen` 0.2.117, and `bytemuck` 1.25.
  - Upgrade `dear-imgui-ash`'s optional `gpu-allocator` integration to 0.28.
  - Upgrade ancillary direct dependencies including `ureq` 3.3 and `regex` 1.12 where used in workspace tooling/extensions.
  - Move `dear-imgui-build-support` into the unified `0.11.0` release train and update all `*-sys` crates to depend on `0.11`.
  - Bump the default `wgpu` baseline to v29.

## [0.10.4] - 2026-03-17

### Deprecated

- Extensions
  - `dear-imnodes`: `EditorContext::*` methods that rely on the global current ImNodes context are deprecated and will be removed in `0.11.0`. Use `Context::bind_editor(&EditorContext)` instead.
  - `dear-imnodes`: `EditorContext::create/try_create` are deprecated and will be removed in `0.11.0`. Use `Context::{create_editor_context,try_create_editor_context}` instead.

### Changed

- Core (`dear-imgui-rs`)
  - Widgets: split `widget::input` into focused submodules for text callbacks and numeric builders without intended public API changes.
  - Platform: split platform_io into focused submodules for viewport wrappers and callback trampolines without intended public API changes.

### Fixed

- Core (`dear-imgui-rs`)
  - Fonts: keep `FontAtlas::set_texture_id()` aligned with the managed `ImTextureData` path so the atlas `TexRef` continues to track backend-driven texture ID updates instead of degrading to a legacy ID-only reference.
- Backends
  - `dear-imgui-glow`: harden the modern texture update path by keeping the fallback font-atlas texture in sync with managed atlas create/update/destroy requests.
  - `dear-imgui-glow`: align texture destroy handling with other renderers by setting `WantDestroyNextFrame` before marking managed textures as `Destroyed`.
  - `dear-imgui-glow`: refactor sub-rectangle RGBA conversion into a reusable helper and add regression tests mirroring the `dear-imgui-wgpu` coverage for `RGBA32` / `Alpha8` uploads.
  - `dear-imgui-glow`: fix external-context rendering so `render_with_context()` now uses the caller-provided GL context for managed texture create/update/destroy requests from `DrawData::textures()` instead of failing when the renderer does not own the context. Fixes #22, thanks @CoffeeCatRailway.
  - `dear-imgui-glow`: add `register_texture_with_context()` / `update_texture_with_context()` helpers plus a runnable external-context regression example covering the create/update/destroy flow.
  - `dear-imgui-wgpu`: stop tracking a separate renderer-side font atlas ID cache in the fallback path; legacy fallback uploads now check the atlas `TexRef` plus live texture-manager state directly, keeping modern managed textures as the source of truth.

## [0.10.3] - 2026-02-28

### Fixed

- `dear-imgui-sys`
  - Link `shell32`/`user32`/`kernel32`/`imm32` explicitly on Windows to fix GNU/MinGW toolchains that ignore ImGui's `#pragma comment(lib, ...)` (Fixes #20).

### CI

- Add a Windows GNU (MinGW) link check job to catch missing system libraries (e.g. `shell32`) at PR time.

## [0.10.2] - 2026-02-24

### Added

- Core (`dear-imgui-rs`)
  - `Context::alive_token()` / `ContextAliveToken`: allow extension crates to detect if an ImGui context has been dropped (helps avoid calling FFI with dangling `ImGuiContext*`).

### Fixed

- Extensions
  - `dear-imnodes`: fix potential use-after-free hazards by scoping editor post-frame handles and style tokens to the active editor/frame lifetime.
  - `dear-imnodes`: ensure minimap callbacks remain alive for the full editor frame.
  - `dear-imnodes`: harden context binding (ImGui/ImNodes/editor) for editor operations and token drops.
  - `dear-imnodes`: guard ImGui context rebinding against dropped contexts (panic instead of UB on misuse).
  - `dear-imnodes`: validate raw style-var indices in `push_style_var_*` helpers to avoid out-of-bounds access in release builds.
  - `dear-imnodes`: fix `imnodes_basic` context menu crash (Fixes #19).
  - `dear-implot`: harden ImGui context binding and guard context drop order hazards via `ContextAliveToken` (panic or best-effort leak instead of UB).
  - `dear-implot3d`: guard context drop order hazards via `ContextAliveToken` (best-effort leak instead of UB).
  - `dear-imgui-test-engine`: record bound ImGui context liveness via `ContextAliveToken` and guard shutdown/drop order hazards (panic or best-effort leak instead of UB).

### Changed

- Extensions
  - `dear-imnodes`: refactor the context module into smaller submodules (no public API semantic changes intended).

## [0.10.1] - 2026-02-21

### Added

- Extensions
  - `dear-imgui-test-engine` / `dear-imgui-test-engine-sys`: Dear ImGui Test Engine support (PR #17, thanks @honeyspoon).

## [0.10.0] - 2026-02-19

Upgrade to Dear ImGui v1.92.6 (docking branch) via cimgui `1.92.6dock`, refresh all C API submodules (ImPlot/ImPlot3D, ImNodes, ImGuizmo, ImGuIZMO.quat), and regenerate bindings for native + import-style WASM builds.

### Highlights

- Dear ImGui v1.92.6 (docking) upgrade.
- Dear ImGui v1.92.6 release notes: https://github.com/ocornut/imgui/releases/tag/v1.92.6
- Regenerated pregenerated bindings for core + all extension `*-sys` crates (native + wasm).
- Updated ImPlot/ImPlot3D safe wrappers for upstream spec-based item APIs.

### Breaking Changes

- Core (`dear-imgui-rs`)
  - Fonts: removed `FontConfig::pixel_snap_v` (upstream removed `ImFontConfig::PixelSnapV`).
  - Popups: `begin_popup_context_*` helpers now default to `PopupFlags::NONE` to follow ImGui v1.92.6 semantics (default is still Right mouse; passing a literal `0` no longer requests Left).

### Added

- Core (`dear-imgui-rs`)
  - Style: `StyleVar::ImageRounding` and `Style::{image_rounding,set_image_rounding}`.
  - Popups: `begin_popup_context_*_with_flags` helpers for explicit popup button/flags selection.
- Extensions
  - `dear-implot`: `ItemFlags` and `with_item_flags(...)` helpers to compose common `ImPlotItemFlags` with plot-type-specific flags.

### Changed

- `*-sys` crates
  - Updated `cimgui`/`cimplot`/`cimplot3d`/`cimnodes`/`cimguizmo`/`cimguizmo_quat` submodules.
  - Regenerated pregenerated bindings (`bindings_pregenerated.rs`, `wasm_bindings_pregenerated.rs`).
  - Build scripts now `rerun-if-changed` on pregenerated bindings to avoid stale OUT_DIR reuse when `*_SYS_SKIP_CC=1`.
- Tooling
  - `tools/update_submodule_and_bindings.py`: select the newest generated `bindings.rs` (mtime) to avoid picking stale outputs.

### Fixed

- Extensions
  - `dear-implot`: updated plot item calls to pass the new `ImPlotSpec_c` parameter (keeps high-level Rust API stable).
  - `dear-implot3d`: updated plot item calls to pass the new `ImPlot3DSpec_c` parameter; re-implemented `set_next_*_style()` helpers on top of the new spec mechanism.
  - `dear-implot`/`dear-implot3d`: stabilized default spec construction against platform-dependent bindgen enum constant types.
  - `dear-imguizmo-quat`: fixed Vec4 type mismatches after sys bindings refresh.

## [0.9.0] - 2026-02-12

This release focuses on `dear-app` usability improvements for real applications (GPU configuration presets, smoother startup, and clearer redraw semantics).

### Breaking Changes

- `dear-app`
  - `RunnerConfig` gains a new required field: `wgpu: WgpuConfig`. Struct-literal initializers without `..Default::default()` must be updated.
  - `RunnerCallbacks` gains a new field: `on_gpu_init`. Struct-literal initializers must be updated.
  - `RedrawMode::Wait` now truly waits (no implicit per-frame redraw). Use `Poll` or `WaitUntil` for continuous rendering.
- Core (`dear-imgui-rs`)
  - `PlatformIo::viewports()` / `PlatformIo::viewports_mut()` are no longer public APIs; use `PlatformIo::viewports_iter()` / `PlatformIo::viewports_iter_mut()` instead.
- Backends
  - `dear-imgui-winit`: `multi_viewport::ViewportData` is no longer a public API (internal backend detail).
  - `dear-imgui-wgpu`: `multi_viewport::{ViewportWgpuData}` and `multi_viewport_sdl3::{ViewportWgpuData}` are no longer public APIs (internal renderer details).
- Extensions
  - `dear-file-browser`: `SortBy` gains a new variant `Type` (IGFD-style filter-aware "Type" sorting). Downstream exhaustive `match` statements must be updated.
- `*-sys` crates (prebuilt downloads)
  - Prebuilt downloads/extraction are now gated behind the Cargo feature `prebuilt`. If you set `*_SYS_PREBUILT_URL` to an `http(s)://...` URL or to a `.tar.gz` archive, or set `*_SYS_USE_PREBUILT=1`, you must also enable `--features prebuilt`.
  - Default builds do not enable `prebuilt` (and therefore do not pull in HTTP client dependencies like `ureq`). (Fixes #12)

### Added

- `dear-app`
  - `WgpuConfig` and `RunnerConfig::wgpu`: configure instance/adapter/device selection (backends, power preference, required features/limits, memory hints, etc.).
  - `WgpuPreset` and `WgpuConfig::from_preset`: curated presets for common scenarios (performance, low-power, downlevel compatibility, software fallback).
  - `AppBuilder::on_gpu_init`: a lifecycle hook for one-time GPU resource initialization after `Device/Queue/SurfaceConfiguration` are available.
  - `pub use wgpu;` re-export as `dear_app::wgpu` for downstream convenience.
- Core (`dear-imgui-rs`)
  - `Ui::set_window_focus`: focus a window by name, or clear focus via `None` (`SetWindowFocus(NULL)`).
  - `Context::{register_user_texture,unregister_user_texture}`: safe wrappers for ImGui's experimental `RegisterUserTexture()` API to include user-created `ImTextureData` in `DrawData::textures()`.
  - `Context::register_user_texture_token`: RAII helper returning `RegisteredUserTexture` which unregisters on drop.
  - Threaded renderer support: `render::snapshot` module with `FrameSnapshot` (`Send + Sync`) for Extract → Render architectures (e.g. Bevy). Snapshots include Rust-owned draw lists/commands and managed texture requests extracted from `DrawData::textures()`.
  - `render::snapshot::TextureFeedback` and `PlatformIo::apply_texture_feedback`: apply renderer-thread results (TexID/status) back to ImGui-managed textures on the UI thread.
  - `Context::platform_io_mut` is now available without the `multi-viewport` feature (enables applying texture feedback even when viewports are disabled).
  - `dear_imgui_rs::fonts` module path is now public (e.g. `dear_imgui_rs::fonts::FontConfig`).
  - `PlatformIo::{platform_create_vk_surface_raw,set_platform_create_vk_surface_raw}`: access to ImGui's optional `Platform_CreateVkSurface` callback (used by Vulkan renderers with SDL2/SDL3/GLFW/Win32 multi-viewport).
  - Input capture hints: `Ui::set_next_frame_want_capture_keyboard` / `Ui::set_next_frame_want_capture_mouse`.
  - Navigation: `Ui::set_nav_cursor_visible`.
  - Drag and drop: `Ui::drag_drop_payload` (`GetDragDropPayload`).
  - Focus utilities: `FocusedFlags`, `Ui::is_window_focused_with_flags`.
  - Window builder: `Window::size_constraints` and `Window::scroll` (`SetNextWindowSizeConstraints`, `SetNextWindowScroll`).
  - Window runtime control: `Ui::set_window_pos*`, `Ui::set_window_size*`, `Ui::set_window_collapsed*`.
  - Shortcut routing: `KeyChord`, `KeyMods`, `InputFlags`, plus `Ui::shortcut` / `Ui::set_next_item_shortcut` / `Ui::is_key_chord_pressed`.
  - Vector sliders: `Ui::slider_float2/3/4` and `Ui::slider_int2/3/4`.
  - `Ui::checkbox_flags` is now aliased as `CheckboxFlags` (matches ImGui semantics, no sys call needed).
  - Tab helpers: `Ui::tab_item_button*` and `Ui::set_tab_item_closed`.
  - Color defaults: `Ui::set_color_edit_options` (`SetColorEditOptions`).
  - Color packing helpers: `Ui::get_color_u32*` and `Ui::style_color` now uses `GetStyleColorVec4`; `Color::{to_imgui_u32,from_imgui_u32}` now use `ColorConvert*`.
  - State storage: `Ui::state_storage`, `Ui::push_state_storage`, `Ui::set_next_item_storage_id`, plus `OwnedStateStorage`.
  - Misc queries: `Ui::window_viewport`, `Ui::tree_node_to_label_spacing`, `Ui::item_id`, `Ui::is_item_edited`.
  - More queries: `Ui::calc_item_width`, `Ui::is_mouse_pos_valid*`, `Ui::is_mouse_released_with_delay`, `Ui::find_viewport_by_id`, `Ui::find_viewport_by_platform_handle`.
  - Context helpers: clipboard (`Context::clipboard_text`, `Context::set_clipboard_text`) and INI disk IO (`Context::load_ini_settings_from_disk`, `Context::save_ini_settings_to_disk`).
  - Logging helpers: `Ui::log_to_tty`, `Ui::log_to_file_default`, `Ui::log_to_file`, `Ui::log_to_clipboard`, `Ui::log_buttons`, `Ui::log_finish`.
  - Popup helper: `Ui::open_popup_on_item_click` / `Ui::open_popup_on_item_click_with_flags`.
- Backends
  - `dear-imgui-ash`: external texture helpers mirroring WGPU (`register_external_texture_with_sampler`, `update_external_texture_view`, `update_external_texture_sampler`, `unregister_texture`).
  - `dear-imgui-ash`: `multi-viewport-sdl3` feature to render secondary viewports when using the SDL3 platform backend (creates surfaces via `Platform_CreateVkSurface`).
  - `dear-imgui-wgpu`: add feature-gated support for both `wgpu` v28 (default) and v27 (`wgpu-27`), to better match ecosystems pinned to a specific major.
- Examples
  - `threaded_snapshot_minimal`: headless multi-thread example demonstrating `FrameSnapshot` + `TextureFeedback` (no real GPU renderer).
- Extensions
  - `dear-file-browser` (ImGui backend parity with ImGuiFileDialog)
    - Breadcrumb path composer: end-aligned tail visibility + separator quick-select popup rendered as an IGFD-style path table.
    - Footer: IGFD-like editable file field in Open modes (type a file name/path and confirm), plus improved PickFolder confirm semantics.
    - Columns/sorting: add `SortBy::Type` and make the "Type" column filter-aware (multi-dot extraction matches IGFD behavior).

### Changed

- `dear-app`
  - Theme application now uses the safe high-level `dear-imgui-rs` `Theme/ThemePreset` API (avoids direct `sys::igStyleColors*` usage).
  - Acquire the swapchain texture later in the frame to reduce the time the surface image is held.
  - `restore_previous_geometry = false` now disables INI persistence by forcing the INI filename to `None`.
- Examples
  - `ash_textures` / `wgpu_textures`: register user-created `TextureData` via `Context::register_user_texture_token()` (no manual pre-warm `update_texture()` needed).
  - `sdl3_ash_multi_viewport`: SDL3 + Ash multi-viewport example, including external Vulkan texture + sampler toggle.

### Fixed

- `dear-app`
  - Per-frame OS cursor/IME state updates via `prepare_render_with_ui` (more correct cursor shape + IME toggling).
  - More reliable recovery when recreating the window/GPU stack after fatal surface errors; if recreation fails, `on_exit` is called with the old context for cleanup.
  - `WaitUntil { fps }` control flow now uses `fps` to schedule the next wake consistently.
- Extensions
  - `dear-file-browser`
    - `DialogManager::open_browser*` now opens dialogs by default (visible immediately), matching the IGFD-style `OpenDialog -> Display` flow.
    - Filters now default to the first configured filter unless the user explicitly selects "All files".

## [0.8.0] - 2026-01-03

This release focuses on FFI soundness and correctness improvements for Dear ImGui v1.92+ (texture system, font atlas, callbacks),
plus backend hardening for multi-viewport and renderer integrations.

### Highlights

- Major UB/FFI hardening pass across core + backends (callbacks, drag/drop payloads, font atlas memory ownership, texture system).
- Managed textures: correctness fixes for `ImTextureData` construction/ownership and the status/TexID contract (fewer asserts and safer iteration).
- Font atlas: safer glyph range handling + runtime font merge behavior; align renderer usage with the new `RENDERER_HAS_TEXTURES` flow.
- Multi-viewport: safer teardown and reentrancy guards for WGPU/Glow/Winit callback paths.
- Extensions: pointer-lifetime fixes and reduced const-casts/`transmute` in ImPlot/ImPlot3D/ImNodes/ImGuizmo/Reflect.

### Breaking Changes

- Core (`dear-imgui-rs`)
  - Remove `Viewport::main()` (returned `&'static Viewport`); use `Ui::main_viewport()` or `Context::main_viewport()` to get a viewport reference tied to the caller's lifetime.
- `dear-imgui-wgpu`
  - Bump `wgpu` to v28 (requires Rust 1.92+).

### Changed

- Sys bindings: always enable `IMGUI_USE_WCHAR32` (ImWchar = 32-bit) across all targets, including import-style WASM bindings.
- SDL3 backend: bump SDL3 dependencies (`sdl3` 0.17, `sdl3-sys` 0.6 / SDL 3.4.0).

### Added

- Core (`dear-imgui-rs`)
  - `Color::to_hsv01` / `Color::from_hsv01`: helpers that match Dear ImGui's HSV semantics (h in `[0, 1]`).
  - ImGui-style `i32` index variants for optional selection (`-1` as no selection):
    `Ui::combo_i32`, `Ui::combo_simple_string_i32`, `ListBox::build_simple_i32`.
  - Menu items: shortcut-free and shortcut variants to avoid `None::<&str>` turbofish:
    `Ui::menu_item_toggle_no_shortcut`, `Ui::menu_item_toggle_with_shortcut`,
    `Ui::menu_item_enabled_selected_no_shortcut`, `Ui::menu_item_enabled_selected_with_shortcut`.
  - `Io::mouse_down_button` / `Io::set_mouse_down_button`: typed `MouseButton` helpers for `Io::MouseDown`.
  - Legacy columns: `Ui::begin_columns_token` (RAII `ColumnsToken` ends columns on drop).
  - `Ui::list_box_config`: convenience constructor for `ListBox`.
  - Scratch C string helpers (no allocation): `with_scratch_txt`, `with_scratch_txt_two`, `with_scratch_txt_three`,
    `with_scratch_txt_slice`, `with_scratch_txt_slice_with_opt`.

- Extensions
  - `dear-implot`: add non-variadic text helpers for annotations/tags (avoids calling C `...`, useful for import-style WASM).
  - `dear-imguizmo`: expose safe alternative-window helpers and byte-slice ID helpers (`str_begin/str_end` form).
  - `dear-imnodes`: expose accessors for the current/raw context pointer.
  - `dear-imgui-reflect`: derive now supports tuple/unit structs and enums with payload variants (payload types must implement `Default` to allow switching); vector editor adds per-item context menus (insert/remove/move/clear), nested container edits now include index/key segments in response paths (e.g. `items[0]`, `map["k"]`), and `ReflectEvent` is now `#[non_exhaustive]` (includes new `ReflectEvent::VecCleared`).

### Fixed

- FFI: harden `ImString`/`InputText` buffer handling to avoid unbounded scans and ensure NUL-terminated, zero-initialized backing storage before calling into C.
- Prebuilt: reject ABI-incompatible prebuilts by requiring `features=wchar32` in `manifest.txt` (and `freetype` when enabled).
- Fonts: fix `ImWchar` handling for wchar32 and add non-BMP glyph range/exclude-range test coverage.
- Cross-target FFI: fix remaining `c_char` signedness assumptions (i8 vs u8) and add an armv7 CI type-check sentinel to catch regressions.
- FFI: fix incorrect `c_char` signedness assumptions in a few call sites (thanks @EtherealPsyche, #10).

- Core (`dear-imgui-rs`)
  - Windows: expose ImGui `p_open` via `Window::opened(&mut bool)` so the title-bar close button (X) can toggle window state.
  - Popups/headers: expose the corresponding `bool*` variants via
    `Ui::begin_modal_popup_with_opened`, `Ui::modal_popup_with_opened`, `Ui::collapsing_header_with_visible`.
  - Selectables: `Selectable::build_with_ref` now uses ImGui's `Selectable(..., bool*)` variant for closer upstream behavior parity.
  - Text helpers: avoid passing user text as a C variadic format string (prevents potential UB when strings contain `%`) and prefer non-variadic tooltip/text paths where available.
  - Tree nodes: avoid calling C variadic tree-node APIs with user-provided labels by routing `TreeNode` through non-variadic `TreeNodeEx` + `PushID`/`PopID`, preventing potential UB when labels contain `%`.
  - `TextFilter`: fix a leak by calling `ImGuiTextFilter_destroy` on drop; avoid per-call allocations; add `pass_filter_range` with correct `text_end` semantics.
  - Clipboard: handle non-UTF8 clipboard payloads without panicking (lossy conversion), sanitize interior NUL bytes, and guard against missing clipboard user data.
  - Draw callbacks: avoid creating `&mut` references from `*const` FFI pointers when clearing callback user data; ensure `add_callback_safe` stores a direct userdata pointer (`userdata_size = 0`) so Dear ImGui doesn't copy closure bytes into its internal callback buffer (fixes UB when executing callbacks).
  - Drag and drop typed payloads: avoid passing uninitialized padding bytes across the C++ boundary and read payload bytes using `read_unaligned` instead of creating references into Dear ImGui's unaligned payload storage (fixes UB).
  - Scratch C strings: avoid scratch-buffer pointer invalidation in multi-string widget calls by using the paired scratch helpers (`scratch_txt_two`/`scratch_txt_with_opt`) when passing multiple C string pointers in one FFI call.
  - Scratch C strings: sanitize interior NUL bytes (`'\0'` → `?`) instead of panicking when building temporary C string pointers.
  - Rendering draw lists: handle null vertex/index buffers defensively when constructing slices at the FFI boundary.
  - InputText (String-backed): avoid undefined behavior when trimming at NUL by zero-initializing spare capacity, including during ImGui resize callbacks.
  - Font atlas: make `FontAtlas::add_font_from_memory_ttf` always copy the TTF bytes into Dear ImGui-owned memory (via `igMemAlloc`) and set `FontDataOwnedByAtlas=true` so the atlas can safely rebuild and free the buffer (fixes potential use-after-free/double-free when passing Rust-owned buffers).
  - Font atlas: add safe wrappers for `AddFontFromMemoryCompressedTTF` / `AddFontFromMemoryCompressedBase85TTF` and extend `FontSource` accordingly (Base85 input is now explicitly NUL-terminated via `CString`).
  - Deprecated glyph ranges: fix `GlyphRangesBuilder::add_ranges` to pass the correct `ImWchar` layout, and free internal `ImVector_ImWchar` buffers; ensure `add_ranges` always passes a NUL-terminated list; add `Drop` for the underlying C++ builder.
  - Dynamic fonts: fix `FontConfig::glyph_exclude_ranges` to pass the correct `ImWchar` layout (and now owns the converted ranges buffer, ensuring it is NUL-terminated).
  - Dynamic fonts: discard baked glyph caches when adding merge-mode fonts, so runtime font merging can override previously-missing glyphs (e.g. `style_and_fonts` CJK merge).
  - Texture iteration/access: make `DrawData::textures()` / `PlatformIo::textures()` yield a guarded mutable view instead of `&mut TextureData` from `&self`, and avoid creating `&mut TextureData` internally when returning `&TextureData` from `DrawData::texture()` / `PlatformIo::texture()` (prevents Rust aliasing UB while keeping the renderer API ergonomic).
  - Texture refs: avoid constructing `ImTextureRef` from `&TextureData` via a mutable FFI pointer; `&TextureData` now forwards only the current `TexID` (legacy path), use `&mut TextureData` for managed textures.
  - Texture IDs: make `RawTextureId` match Dear ImGui's `ImTextureID` (`ImU64`) and add debug assertions to catch pointer-width truncation when converting a `TextureId` into `usize`/`*const c_void`.
  - Managed textures: avoid null pointer arithmetic when iterating `DrawData::textures()` / `PlatformIo::textures()` on empty lists; make `DrawData::texture{,_mut}` robust to negative vector sizes.
  - Managed textures: fix `ImTextureData` ownership by introducing `OwnedTextureData` (C++ constructed/destroyed) and making `TextureData::new()` return it; use `ImTextureData_SetStatus`/`ImTextureData_SetTexID` to preserve ImGui's internal state machine.
  - Managed textures: `TextureData::set_status(Destroyed)` now clears `TexID`/`BackendUserData` to match Dear ImGui's texture contract and avoid asserts.
  - Font atlases: `FontAtlas::get_glyph_ranges_default` includes the terminating `0` sentinel.
  - `PlatformIo`: typed callback setters no longer panic if the internal callback mutex is poisoned.
  - Clipboard callbacks: handle null `PlatformIO` defensively and guard `ClipboardContext` access against reentrant mutable borrows (avoids potential aliasing UB in callbacks).
  - `OwnedDrawData`: avoid double-free by letting `ImDrawData` own and free its `CmdLists` storage (we still destroy the cloned `ImDrawList` payloads).
  - `Context::save_ini_settings`: read the returned settings blob using `out_ini_size` instead of relying on NUL termination.
  - `Ui::get_key_name` / `Ui::style_color_name`: handle null pointers defensively at the FFI boundary.
  - Draw data types: add compile-time layout assertions to keep `DrawVert`/`DrawIdx` compatible with sys `ImDrawVert`/`ImDrawIdx`.
  - FFI wrappers: add compile-time layout assertions for transparent wrappers (`Io`, `Style`, `Font`, `PlatformIo`, `Viewport`, `TextureRef`, `TextureData`) to keep pointer casts sound when updating sys bindings.
  - Draw data: make internal `DrawData::cmd_lists` representation match sys (`ImDrawList*` pointers) and add compile-time layout assertions for `DrawData`/`DrawList` (reduces pointer casts and prevents silent layout drift).
  - `Ui::io`: panic on null IO pointer instead of dereferencing it (avoid UB when called without an active context).
  - `Io`/`Style`/`Font`/`PlatformIo`/`Viewport`/`TextureData`: store ImGui-owned sys values behind `UnsafeCell` to make Dear ImGui-driven interior mutability explicit (reduces risk of Rust aliasing UB when the C++ side mutates these structs).
  - Tests: avoid `mem::zeroed()` for `ImGuiPlatformIO` by constructing it via the C++ constructor instead.
  - Additional FFI hardening: treat negative `ImVector` sizes as empty, guard `TextureData::pixels*` against invalid dimensions/overflow, clamp `TextureData::set_width/set_height` to avoid `u32 → i32` wrap-around, validate `InputTextCallbackData::str_as_bytes_mut` buffer bounds before creating slices, and prevent unwinding across FFI (panic → abort).

- Backends
  - `dear-imgui-wgpu`
    - Font atlas: stop calling `FontAtlas::build()` in renderer code when using `BackendFlags::RENDERER_HAS_TEXTURES`; skip legacy TexID fallback in the new texture system.
    - SDL3 multi-viewport: drop unnecessary `unsafe impl Send/Sync` from the SDL window surface target adapter.
    - Render callbacks: require `&mut WgpuRenderState` to access the render pass encoder (avoid `&mut` derived from `&self` / `clippy::mut_from_ref` footgun).
    - Multi-viewport: add `disable()`/`shutdown_multi_viewport_support()` helpers and clear stored globals to avoid stale callback pointers when tearing down a renderer.
    - Multi-viewport: clear the global renderer pointer on drop so callbacks become a no-op if the renderer is dropped without an explicit disable.
    - Multi-viewport: guard access to the global renderer pointer to avoid creating multiple `&mut WgpuRenderer` references across callbacks (skip on reentrancy).
    - SDL3 multi-viewport: avoid nested mutable borrows of per-viewport `RendererUserData` by caching framebuffer sizes before encoding draw commands (prevents aliasing UB).
    - Frame resources: pack vertex/index data into byte buffers explicitly instead of casting typed slices to bytes (avoids reading uninitialized padding bytes).
    - Multi-viewport: avoid `transmute` for `wgpu::Surface` lifetimes in multi-viewport backends, harden draw offsets against integer overflow, and avoid panicking when retrieving cached bind groups.
    - Managed textures: keep existing GPU texture and mark status `OK` when a `WantUpdates` upload fails (avoids invalid `TexID` during the same frame).
  - `dear-imgui-sdl3`
    - Manual gamepad mode: avoid casting away constness in Rust by taking a `*const` pointer array and copying pointers into stable storage on the C++ side.
    - OpenGL3 renderer: make `RenderDrawData` wrapper const-correct to avoid casting `&DrawData` to `*mut` across FFI; use `TextureData::as_raw_mut()` for `UpdateTexture` helper to avoid relying on wrapper pointer casts.
  - `dear-imgui-glow`
    - GL state restore: treat negative `glGetIntegerv` results defensively when restoring bindings (avoid casting `i32` → `u32` blindly).
    - Draw callbacks: abort on panic when executing `ImDrawCmd` raw callbacks (avoid unwinding across `extern "C"` ABI).
    - Multi-viewport: clear the global renderer pointer on drop so callbacks become a no-op if the renderer is dropped without an explicit disable.
    - Texture map: avoid aliasing `&mut self` with `&self.texture_map` during rendering by temporarily moving the texture map out (removes raw-pointer borrow workaround, prevents potential aliasing UB).
    - Multi-viewport: guard access to the global renderer pointer to avoid creating multiple `&mut GlowRenderer` references across callbacks (skip on reentrancy).
    - Vertex/index uploads: restrict slice-to-bytes conversion to `DrawVert`/`DrawIdx` to avoid accidentally reading padding bytes from arbitrary Rust types.
    - Textures: validate `TextureId` range when updating existing OpenGL textures (avoid truncation/panic).
  - `dear-imgui-winit`
    - Multi-viewport: avoid freeing `ViewportData` while an `&mut ViewportData` reference is still live (raw-pointer cleanup instead).
    - Multi-viewport: avoid creating `&mut Window` references from stored raw window pointers in platform callbacks.
    - IME bridge: treat `Platform_ImeUserData` as `*const Window` in callbacks to avoid suggesting mutability across the FFI boundary.
    - Keyboard modifiers: submit `ImGuiMod_*` key events on `ModifiersChanged` so `io.KeyMods` / `io.KeyCtrl` / `io.KeyShift` / `io.KeyAlt` / `io.KeySuper` are updated correctly (#11).

- Extensions
  - `dear-implot`: fix `SubplotToken`/`MultiAxisToken`/`LegendToken` double-end by letting `Drop` perform the actual `End*` call; make subplot ratio buffers owned to avoid casting away constness.
  - `dear-implot`: keep axis formatter/transform closures alive for the full plot scope (avoid dangling `user_data` pointers if tokens are dropped early).
  - `dear-implot`: fix `MultiAxisPlot` axis setup (Y1 is now configurable) and remove a potential panic when keeping axis labels alive.
  - `dear-implot`: avoid `transmute` when passing `dear-imgui-rs::TextureId` to `ImTextureID`.
  - `dear-implot3d`: fix `PlotMesh` vertex layout (convert `[[f32; 3]]` into `ImPlot3DPoint` with `f64` fields before the FFI call).
  - `dear-implot3d`: keep tick label pointers alive for `setup_axis_ticks_*` calls (avoids passing dangling pointers to C).
  - `dear-implot3d`: destroy the correct context pointer on drop (no reliance on "current context").
  - `dear-imnodes`: trim trailing NUL terminators from INI state strings returned by ImNodes save APIs.
  - `dear-imguizmo-quat`: avoid casting `*const` matrices to `*mut` in quaternion helpers by using a local mutable copy.
  - `dear-implot-sys`: build-from-source compiles a patched `cimplot.cpp` copy from `OUT_DIR` to avoid a known out-of-bounds `FormatSpec[16]` access in the upstream-generated wrapper code (submodule remains unchanged; when the bug is detected we also skip the CMake build path and fall back to the patched `cc` build).
  - `dear-imgui-reflect`: harden response/settings scopes against panics, fix `Vec` button ID collisions, reduce `BTreeMap` editor overhead, and clean up map-add popup temporary state.

### Changed

- Core (`dear-imgui-rs`)
  - `ui.window(...).build(...)` only shows a close button when `opened(...)` is provided (matches upstream Dear ImGui behavior).
  - `Ui::get_id` uses the internal scratch buffer instead of allocating a `CString`.
  - `Ui::window` now accepts `Into<Cow<'_, str>>` and avoids per-frame string allocation for borrowed names.
  - `InputText::hint` now accepts `Into<Cow<'_, str>>`, matching label ergonomics and avoiding type changes when passing owned `String`s.
  - Avoid per-frame allocations for common builder labels/IDs by storing `Cow<'_, str>`:
    `Button`, `InputText*`, `InputInt/Float/Double`, `PlotLines/Histogram`, `ColorEdit/Picker/Button`,
    `ImageButton`, `ProgressBar`, `TableBuilder`/`ColumnBuilder`.
  - Non-`Ui` string entrypoints also use the shared scratch strategy where applicable:
    `DockBuilder::dock_window`, `DockBuilder::copy_window_settings`, and `TextCallbackData::insert_chars`.
  - `ImString` now rejects interior NUL bytes in safe constructors/mutators (`new`, `push_str`).
- `dear-imgui-wgpu`
  - Allow enabling both `multi-viewport-winit` and `multi-viewport-sdl3` simultaneously (exports both helper modules).
- Extensions (`dear-implot`, `dear-implot3d`, `dear-imnodes`, `dear-imguizmo`, `dear-imguizmo-quat`)
  - Avoid per-call `CString` allocations in most label/text/title APIs by using the shared scratch string helpers.

## [0.7.0] - 2025-12-13

Unified release train bump to 0.7.0 with Rust-side API improvements and backend updates.
Upstream Dear ImGui/cimgui version is unchanged in this release (still Dear ImGui v1.92.5 docking, same as 0.6.0).

### Highlights

- Experimental native multi-viewport for Winit + WGPU (`dear-imgui-winit::multi_viewport`, `dear-imgui-wgpu::multi_viewport`) with `multi_viewport_wgpu` example.
- Theme API + optional `serde` support for core enums/flags to make layouts/themes easier to persist.
- Multi-select helpers for list/table selection (`Ui::multi_select_*`, `Ui::table_multi_select_indexed`).
- New extension crate `dear-imgui-reflect` for derive-based editors (ImReflect-style auto UI).
- `dear-imgui-wgpu` renderer improvements: unified errors, pipeline layout reuse, tighter texture lifetime handling, and per-texture custom samplers.
- `dear-app` GPU recovery: attempts a full rebuild on fatal WGPU errors.
- WebAssembly import-style provider module `imgui-sys-v0` plus `xtask` commands to build the core + selected extensions.

### Breaking Changes

- `dear-imgui-wgpu`: removed `multi-viewport` feature; use `multi-viewport-winit` (Winit route) or `multi-viewport-sdl3` (SDL3 route).
- `dear-imgui-sdl3`: official OpenGL3 renderer is now opt-in behind `opengl3-renderer` (SDL3 platform-only by default).
  - Example: `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`

### Added

- Core (`dear-imgui-sys`, `dear-imgui-rs`)
  - Optional `glam` integration so `glam::Vec2/Vec4` can be passed directly to drawing and coordinate-taking APIs.
  - IO: mouse source/viewport helpers (`Io::add_mouse_source_event`, `Io::add_mouse_viewport_event`, `MouseSource`, `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT`) to match latest Dear ImGui input model.
  - IO: expanded safe `Io` accessors for common configuration and backend fields (e.g. ini/log filenames read-only, `UserData` and backend user data pointers, key repeat and mouse thresholds, backend names).
  - IO, layout & style: optional `serde` support for core enums and flags (`Key`, `MouseButton`, `MouseCursor`, `MouseSource`, `InputTextFlags`, `ConfigFlags`, `BackendFlags`, `ViewportFlags`, `WindowFlags`, `TableFlags`, `TableColumnFlags`, `TableRowFlags`, `TableBgTarget`, `SortDirection`, `StyleColor`) behind the `serde` feature for easier hotkey, layout/table, and theme configuration persistence.
  - Styling: a small, high-level theme configuration layer (`Theme`, `ThemePreset`, `ColorOverride`, `StyleTweaks`, `WindowTheme`, `TableTheme`) on top of `ImGuiStyle` so applications can define reusable color/rounding/spacing presets and serialize them when the `serde` feature is enabled.
  - Multi-select: high-level helpers on top of `BeginMultiSelect`/`EndMultiSelect` (`MultiSelectFlags`, `Ui::multi_select_indexed`, `Ui::table_multi_select_indexed`, `Ui::multi_select_basic`, `Ui::is_item_toggled_selection`, `BasicSelection`) for list/table selection with ctrl/shift/box-select behavior.
- New extension crate: `dear-imgui-reflect`
  - Derive-based helpers for generating ImGui editors for structs and enums (ImReflect-style auto UI).
- `dear-imgui-winit`
  - IME support for winit 0.30 (cursor area updates and enable/disable helpers) plus a new `ime_debug` example.
  - Convenience API `WinitPlatform::handle_window_event` for `ApplicationHandler`-style event loops.
  - Native (pure-Rust) multi-viewport support for Winit + WGPU (platform windows/events in `dear-imgui-winit::multi_viewport`, renderer callbacks in `dear-imgui-wgpu::multi_viewport`).
    Run with: `cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport`
- WebAssembly (import-style, experimental)
  - Import-style provider module `imgui-sys-v0` and `xtask` commands to build the core + selected extensions (ImPlot, ImPlot3D, ImNodes, ImGuizmo, ImGuizmo.quat) for `wasm32-unknown-unknown`.
- Examples
  - Texture demos (WGPU, dear-app WGPU, Glow) now ship a clean gradient test image (`texture_clean.ppm`) alongside the existing JPEG, making texture sampling artifacts easier to inspect.
  - `style_and_fonts` quickstart example now demonstrates the theme API with several ready-to-use presets (Dark/Light/Classic) plus styled themes (modern dark, Catppuccin Mocha, Darcula, Cherry) adapted from popular Dear ImGui community snippets (including ocornut/imgui#707), showing how to configure and switch custom themes in a single place.
    Run with: `cargo run -p dear-imgui-examples --bin style_and_fonts`

### Changed

- dear-imgui-rs
  - Align several flag types (FreeType font loader flags, child window flags) with upstream Dear ImGui constants to reduce the risk of bit mismatches on future upgrades.
  - Managed textures: `TextureData::new()` now returns `OwnedTextureData` to ensure correct C++ construction/destruction (and thus correct pixel buffer cleanup on drop).
- Extensions (`dear-implot`, `dear-implot3d`, `dear-imnodes`, `dear-imguizmo`, `dear-imguizmo-quat`, `dear-file-browser`)
  - Refresh bindings to the latest C APIs and tighten safe wrappers; includes making file-extension filters in the file browser case-insensitive.
  - ImGuizmo: keep the internal helper window ("gizmo") on the main viewport when ImGui multi-viewport is enabled, preventing an extra black OS window on Windows (workaround for CedricGuillemet/ImGuizmo#378).
- dear-imgui-wgpu
  - Unified internal error handling to use the shared `RendererError` type instead of ad-hoc `Result<_, String>` values in frame/texture paths, making GPU failures easier to diagnose.
  - Simplified pipeline/bind group layout wiring so the render pipeline now reuses the layouts owned by `RenderResources`/`UniformBuffer`, avoiding duplicated layout definitions and potential mismatches.
  - Tightened texture/bind group lifetime coupling: when ImGui textures are created, updated, or destroyed via the 1.92+ texture system, any cached image bind groups are invalidated and rebuilt on demand.
  - Minor internal cleanups (logging feature flag for multi-viewport traces, dead-code reductions) to keep the backend warning-free on newer Rust toolchains.
  - Added optional per-external-texture custom samplers. New APIs:
    `register_external_texture_with_sampler` and `update_external_texture_sampler`.
    See `wgpu_rtt_gameview` for a runtime sampler-switching demo.
  - Added a render-target format preflight when an adapter is provided, requiring the chosen
    `render_target_format` to be `RENDER_ATTACHMENT`-capable and blendable.
  - Experimental native multi-viewport support for SDL3 + WGPU via `multi_viewport_sdl3`, with a new `sdl3_wgpu_multi_viewport` example.
    Run with: `cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport`

- dear-app
  - Render loop now performs basic GPU/surface loss recovery: if a frame render returns a fatal GPU error, dear-app tears down the existing `AppWindow` and attempts to recreate the WGPU device/surface/renderer stack using the same `RunnerConfig`/add-ons.
  - Existing graceful handling of `SurfaceError::Lost`/`Outdated` remains in place (surface is reconfigured in-place when possible); the new logic adds a “full rebuild” path for irrecoverable errors instead of leaving the app in a broken redraw loop.

- Examples
  - `ime_debug`: add a runtime CJK merge button and `DEAR_IMGUI_DEFER_CJK=1` mode to validate dynamic font merging after missing-glyph caching (WGPU path).


## [0.6.0] - 2025-11-28

Upgrade to Dear ImGui v1.92.5 (docking branch), adjust FFI and safe APIs for new return-by-value helpers, and refresh all C API submodules.

### Added

- Backends
  - New `dear-imgui-sdl3` backend crate:
    - Thin, safe wrapper around upstream `imgui_impl_sdl3.cpp` + `imgui_impl_opengl3.cpp`.
    - Provides SDL3 platform integration for Dear ImGui and OpenGL3 rendering.
    - Includes an SDL3 + OpenGL3 multi-viewport example:
      - `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`
    - SDL3 + WGPU is supported via the Rust WGPU backend (`dear-imgui-wgpu`) with a basic example:
      - `cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform`
      - Multi-viewport remains **disabled** for WebGPU, matching upstream `imgui_impl_wgpu` which does not yet support multi-viewport.
  - `dear-imgui-glow`:
    - Experimental multi-viewport support mirroring the upstream OpenGL3 renderer backend:
      - Adds a `multi_viewport` helper module and `multi_viewport::enable(&mut GlowRenderer, &mut Context)` API.
      - Clears secondary viewports using a configurable clear color via `GlowRenderer::set_viewport_clear_color`.
      - Uses Dear ImGui’s `ImGuiPlatformIO::Renderer_RenderWindow` callback in the same way as `imgui_impl_opengl3`.
    - When combined with SDL3 platform backend, provides a pure-Rust SDL3 + Glow multi-viewport stack:
      - New helper `dear-imgui-sdl3::init_platform_for_opengl` to initialize only the SDL3 platform backend (no C++ OpenGL3 renderer).
      - New example using SDL3 + Glow multi-viewport:
        - `cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features multi-viewport,sdl3-platform`

- dear-imgui-rs 0.6.0
  - New drag/drop flag `DragDropFlags::ACCEPT_DRAW_AS_HOVERED`, wrapping `ImGuiDragDropFlags_AcceptDrawAsHovered`.
  - New style color `StyleColor::DragDropTargetBg`, exposing `ImGuiCol_DragDropTargetBg`.
  - Experimental WebAssembly font atlas support behind `wasm-font-atlas-experimental` feature (import-style provider only; APIs and behaviour may change).

- dear-imgui-sys 0.6.0
  - Updated to Dear ImGui v1.92.5 (docking branch).
  - Updated cimgui submodule to `1.92.5dock` (Dear ImGui `v1.92.5-docking`).
  - Regenerated FFI bindings, including new enums/fields added in 1.92.5.
  - Added pregenerated wasm bindings (`wasm_bindings_pregenerated.rs`) importing from module `imgui-sys-v0` for import-style WASM builds.

- Tooling / xtask
  - `xtask wasm-bindgen imgui-sys-v0` generates import-style wasm bindings for `dear-imgui-sys` with `wasm-bindgen-cli 0.2.105`.
  - `xtask web-demo` builds the `dear-imgui-web-demo` wasm example, patches the wasm to import memory from `env`, and injects shared memory wiring into the JS glue.
  - `xtask build-cimgui-provider` builds an Emscripten-based `imgui-sys-v0` provider (`imgui-sys-v0.js` + `.wasm`) and injects an import map mapping `imgui-sys-v0` to `./imgui-sys-v0-wrapper.js`.

### Changed

- Adjusted FFI signatures and safe wrappers to follow upstream return-by-value helpers introduced in 1.92.x:
  - Functions such as `igGetMousePos`, `igGetMouseDragDelta`, `igGetWindowPos/Size`,
    `igGetCursorPos/CursorScreenPos/CursorStartPos`, `igGetItemRectMin/Max/Size` and
    `igGetContentRegionAvail` now return `ImVec2` instead of writing through out-parameters.
  - Docking helpers now return `ImRect` directly (e.g. `ImGuiDockNode_Rect`).
  - Text helpers such as `ImFont_CalcTextSizeA` and `igCalcTextSize` now return an `ImVec2` result.
  - All affected safe APIs in `dear-imgui-rs` have been updated to transparently use the new signatures.
- Inherited all bug fixes and behavior changes from Dear ImGui v1.92.5, including improved drag/drop,
  navigation, InputText, and table behavior (see upstream release notes for details).

- Updated C API submodules for extensions to latest branches and regenerated bindings:
  - `dear-implot-sys` (cimplot, ImPlot C API).
  - `dear-implot3d-sys` (cimplot3d, ImPlot3D C API).
  - `dear-imnodes-sys` (cimnodes, ImNodes C API).
  - `dear-imguizmo-sys` (cimguizmo, ImGuizmo C API).
  - `dear-imguizmo-quat-sys` (cimguizmo_quat, ImGuIZMO.quat C API).
  - Safe wrappers for these crates have been adjusted as needed to match any signature changes reported by bindgen.

### Multi-viewport notes (0.6.x)

- SDL3 + OpenGL3:
  - Multi-viewport is provided by upstream C++ backends (`imgui_impl_sdl3.cpp` + `imgui_impl_opengl3.cpp`) and considered stable for desktop use.
  - The `sdl3_opengl_multi_viewport` example shows how to:
    - Drive Dear ImGui via the official SDL3 platform backend;
    - Render a user-provided OpenGL texture inside an ImGui window (`Game View`) that can be dragged to secondary OS windows.
- SDL3 + Glow (experimental):
  - Uses SDL3 platform backend (`dear-imgui-sdl3`) + Rust Glow renderer backend (`dear-imgui-glow`).
  - Platform responsibilities (window creation, GL context switching, swap buffers) remain in the C++ SDL3 backend; rendering of all viewports is handled by `GlowRenderer` via `multi_viewport::enable`.
  - The `sdl3_glow_multi_viewport` example demonstrates this stack:
    - `cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features multi-viewport,sdl3-backends`
  - This path is intended as an experimental native OpenGL alternative to the C++ `imgui_impl_opengl3` renderer.
- SDL3 + WGPU:
  - Uses the SDL3 platform backend (`imgui_impl_sdl3`) + Rust WGPU renderer (`dear-imgui-wgpu`).
  - The `sdl3_wgpu` example demonstrates SDL3 + WGPU integration (single window); multi-viewport remains **disabled** on this route for WebGPU, matching upstream `imgui_impl_wgpu` which does not yet implement multi-viewport.
- Winit + WGPU:
  - Experimental multi-viewport support exists in `dear-imgui-winit::multi_viewport` + `dear-imgui-wgpu::multi_viewport`, and is exercised by the `multi_viewport_wgpu` example.
  - This path is **not supported** in 0.6.x for production use and is known to be unstable on some platforms (especially macOS/winit).
  - The example is kept as a testbed to illustrate the architecture:
    - the platform backend owns OS windows and fills `ImGuiPlatformIO` callbacks (create/destroy/update window, event routing);
    - the renderer backend installs `Renderer_CreateWindow` / `Renderer_RenderWindow` / `Renderer_SwapBuffers` callbacks to create per-viewport render targets and draw ImGui content into them.

### Version Updates

**All crates in the workspace have been upgraded to 0.6.0** due to the Dear ImGui v1.92.5 upgrade and C API refresh.

**Core:**
- `dear-imgui-sys` → 0.6.0
- `dear-imgui-rs` → 0.6.0

**Backends:**
- `dear-imgui-wgpu` → 0.6.0
- `dear-imgui-glow` → 0.6.0
- `dear-imgui-winit` → 0.6.0

**Application Framework:**
- `dear-app` → 0.6.0

**Extensions:**
- `dear-imnodes` → 0.6.0 (+ `dear-imnodes-sys` → 0.6.0)
- `dear-implot` → 0.6.0 (+ `dear-implot-sys` → 0.6.0)
- `dear-implot3d` → 0.6.0 (+ `dear-implot3d-sys` → 0.6.0)
- `dear-imguizmo` → 0.6.0 (+ `dear-imguizmo-sys` → 0.6.0)
- `dear-imguizmo-quat` → 0.6.0 (+ `dear-imguizmo-quat-sys` → 0.6.0)
- `dear-file-browser` → 0.6.0

### Misc

- Documentation and minor internal cleanups for extension crates:
  - Fix and consolidate READMEs / examples across `dear-implot`, `dear-implot3d`, `dear-imnodes`, `dear-imguizmo`, `dear-imguizmo-quat`, and `dear-file-browser`.

## [0.5.0] - 2025-10-24

Upgrade to Dear ImGui v1.92.4 (docking branch) with new color styling option and bug fixes.

### Added

- dear-imgui-rs 0.5.0
  - New `StyleColor::UnsavedMarker` color for marking unsaved documents/windows
  - This color is used by Dear ImGui to indicate unsaved state in tabs and windows

- dear-imgui-sys 0.5.0
  - Updated to Dear ImGui v1.92.4 (docking branch)
  - Updated cimgui submodule to v1.92.4dock (commit 2d91c9d)
  - Regenerated FFI bindings with new ImGuiCol_UnsavedMarker constant

### Changed

- Updated all documentation references from v1.92.3 to v1.92.4
- ImGuiCol_COUNT increased from 60 to 61 due to new color addition
- dear-imgui-winit: Map extra mouse buttons
  - `winit::event::MouseButton::Back/Forward` and common `Other(3)/Other(4)` are now mapped to `ImGuiMouseButton::Extra1/Extra2`
  - Improves out-of-the-box support for side buttons on modern mice; no API changes

### Fixed

- Inherited all bug fixes from Dear ImGui v1.92.4:
  - InputText: Fixed single-line character clipping regression from v1.92.3
  - InputText: Fixed potential infinite loop in callback handling
  - Improved texture lifecycle management
  - Fixed multi-context ImFontAtlas sharing issues
- dear-imgui-winit: Stabilized tests that create an ImGui context by serializing them to avoid spurious `ContextAlreadyActive` panics (internal, no runtime impact)

### Version Updates

**All crates in the workspace have been upgraded to 0.5.0** due to the Dear ImGui v1.92.4 upgrade.

**Core:**
- `dear-imgui-sys` → 0.5.0
- `dear-imgui-rs` → 0.5.0

**Backends:**
- `dear-imgui-wgpu` → 0.5.0
- `dear-imgui-glow` → 0.5.0
- `dear-imgui-winit` → 0.5.0

**Application Framework:**
- `dear-app` → 0.5.0

**Extensions:**
- `dear-imnodes` → 0.5.0 (+ `dear-imnodes-sys` → 0.5.0)
- `dear-implot` → 0.5.0 (+ `dear-implot-sys` → 0.5.0)
- `dear-implot3d` → 0.5.0 (+ `dear-implot3d-sys` → 0.5.0)
- `dear-imguizmo` → 0.5.0 (+ `dear-imguizmo-sys` → 0.5.0)
- `dear-imguizmo-quat` → 0.5.0 (+ `dear-imguizmo-quat-sys` → 0.5.0)
- `dear-file-browser` → 0.5.0

## [0.4.1] - 2025-10-07

Small, focused improvements to enable real-time texture workflows (game view, atlas tools, image browsers) without frame delay.

### Added

- dear-imgui-wgpu 0.4.1
  - External texture APIs for real-time usage:
    - `WgpuRenderer::register_external_texture(&Texture, &TextureView) -> u64`
    - `WgpuRenderer::update_external_texture_view(id, &TextureView) -> bool`
    - `WgpuRenderer::unregister_texture(id)`
  - These allow displaying existing `wgpu::Texture` resources via legacy `TextureId` in the same frame (no reliance on TextureData state machine), ideal for game views/RTTs or dynamic atlases.

- dear-app 0.4.1
  - New `AddOns.gpu` API exposing:
    - `device()` / `queue()` passthroughs
    - `register_texture`, `update_texture_view`, `unregister_texture`
    - `update_texture_data(&mut TextureData)` that applies the backend result to set `TexID/Status` immediately (no white frame).
  - New example `examples/01-renderers/dear_app_wgpu_textures.rs` showcasing both managed `TextureData` updates and external WGPU textures in real time.

### Changed

- Examples now include a dear-app + wgpu textures demo exhibiting same-frame updates and game-view style external texture display.

### Version Updates

- `dear-imgui-wgpu` → 0.4.1
- `dear-app` → 0.4.1

## [0.4.0] - 2025-10-07

This is a major feature release that introduces several new extensions, improves the docking API, and adds a convenient application runner.

### 🎉 New Features

#### New Extensions

- **dear-app** - A convenient application runner built on Winit + WGPU
  - Provides easy-to-use application framework with docking support
  - Built-in theme support and add-ons integration
  - Simplifies the setup process for new projects
  - See examples: `dear_app_quickstart.rs`, `dear_app_docking.rs`

- **dear-implot3d** - 3D plotting extension
  - Full Rust bindings for ImPlot3D (cimplot3d C API)
  - Support for 3D scatter plots, line plots, surface plots, mesh plots
  - Triangle and quad rendering capabilities
  - 3D image display support
  - Comprehensive style customization
  - Example: `implot3d_basic.rs`

- **dear-imguizmo-quat** - Quaternion-based 3D gizmo
  - Full Rust bindings for ImGuIZMO.quat (cimguizmo_quat C API)
  - Quaternion manipulation widgets
  - 3D direction and rotation controls
  - Example: `imguizmo_quat_basic.rs`

- **dear-file-browser** - File browser and dialog extension
  - Native OS file dialogs via `rfd` backend
  - Pure ImGui in-UI file browser implementation
  - Support for file/folder selection, save dialogs
  - Customizable file filters and multi-selection
  - Examples: `file_dialog_native.rs`, `file_browser_imgui.rs`

#### Core Improvements

- **Safe DockBuilder API** ([#14d96cf](https://github.com/Latias94/dear-imgui-rs/commit/14d96cf2f527d978a23c793e84d34d80cd8c6a5f))
  - Added `Ui::set_next_window_viewport()` and `Ui::get_id()` helper methods
  - Introduced `DockNode<'ui>` with read-only queries and `NodeRect` type
  - Added safe methods: `DockBuilder::node()`, `central_node()`, `node_exists()`
  - Removed unsafe methods: `DockBuilder::get_node()`, `get_central_node()`
  - Updated docking examples to use safe APIs

- **Enhanced Docking Support**
  - Fixed docking split node function for more reliable layout management
  - Improved game engine docking example with better UI organization
  - Updated dockspace minimal example with safe API usage

### 🔧 Improvements

#### Dependencies

- **Updated wgpu to v27** - Latest WGPU version with improved performance and features
- Updated workspace to use Rust edition 2024

#### Build System

- Added prebuilt binary packaging support for `dear-imguizmo-quat-sys`
- Improved CI workflow for prebuilt binaries
- Added cargo clippy checks to CI pipeline
- Optimized cargo exclude patterns for smaller package sizes

#### Documentation

- Comprehensive README updates for all new extensions
- Updated compatibility matrix in `docs/COMPATIBILITY.md`
- Added detailed usage examples for new features
- Improved build instructions and feature flag documentation

### 📦 Version Updates

#### Core Packages (0.4.0)
- `dear-imgui-rs` → 0.4.0
- `dear-imgui-sys` → 0.4.0
- `dear-imgui-wgpu` → 0.4.0
- `dear-imgui-glow` → 0.4.0
- `dear-imgui-winit` → 0.4.0

#### Application Runner (0.4.0)
- `dear-app` → 0.4.0 (new)

#### Extensions (0.4.0)
- `dear-implot` → 0.4.0
- `dear-implot-sys` → 0.4.0
- `dear-imnodes` → 0.4.0
- `dear-imnodes-sys` → 0.4.0
- `dear-imguizmo` → 0.4.0
- `dear-imguizmo-sys` → 0.4.0
- `dear-implot3d` → 0.4.0 (new)
- `dear-implot3d-sys` → 0.4.0 (new)
- `dear-imguizmo-quat` → 0.4.0 (new)
- `dear-imguizmo-quat-sys` → 0.4.0 (new)
- `dear-file-browser` → 0.4.0 (new)

### 📚 Examples

New examples added:
- `dear_app_quickstart.rs` - Quick start guide using dear-app
- `dear_app_docking.rs` - Docking example using dear-app
- `implot3d_basic.rs` - Comprehensive 3D plotting demo
- `imguizmo_quat_basic.rs` - Quaternion gizmo demonstration
- `file_dialog_native.rs` - Native file dialog usage
- `file_browser_imgui.rs` - ImGui file browser UI

Updated examples:
- `game_engine_docking.rs` - Significantly improved with better layout and features
- `dockspace_minimal.rs` - Rewritten to use safe DockBuilder APIs
- `tables_minimal.rs` - Minor improvements

### ⚠️ Breaking Changes

- **DockBuilder API**: Removed unsafe methods `get_node()` and `get_central_node()`. Use the new safe alternatives: `node()` and `central_node()`
- **Docking Split API**: Updated signature for split node functions to be more type-safe

### 🔮 Experimental

- Multi-viewport support is still work-in-progress and not production-ready
  - Test example available: `cargo run --bin multi_viewport_wgpu --features multi-viewport`
  - This feature is excluded from this release as it's not yet complete

### 📖 Migration Guide

#### Updating DockBuilder Usage

**Before (v0.3.0):**
```rust
unsafe {
    let node = DockBuilder::get_node(dock_id);
    let central = DockBuilder::get_central_node(dock_id);
}
```

**After (v0.4.0):**
```rust
if let Some(node) = DockBuilder::node(ui, dock_id) {
    // Use node safely
}
if let Some(central) = DockBuilder::central_node(ui, dock_id) {
    // Use central node safely
}
```

#### Using the New dear-app Runner

**Before (manual setup):**
```rust
// Manual Winit + WGPU setup code...
```

**After (with dear-app):**
```rust
use dear_app::*;

fn main() {
    App::new("My App")
        .run(|ui| {
            ui.window("Hello").build(|| {
                ui.text("Hello, world!");
            });
        });
}
```

### 🙏 Acknowledgments

Special thanks to all contributors and the upstream projects:
- Dear ImGui by Omar Cornut
- ImPlot3D for 3D plotting capabilities
- ImGuIZMO.quat for quaternion manipulation
- rfd for native file dialogs

**Full Changelog**: https://github.com/Latias94/dear-imgui-rs/compare/v0.3.0...v0.4.0

## [0.3.0] - 2025-09-30

### Changed

- **BREAKING**: Renamed main crate from `dear-imgui` to `dear-imgui-rs` following feedback from Dear ImGui maintainer
  - Update your `Cargo.toml` dependencies from `dear-imgui = "0.2"` to `dear-imgui-rs = "0.3"`
  - Update all `use dear_imgui::*` imports to `use dear_imgui_rs::*`
  - The old `dear-imgui` crate (v0.2.0) has been yanked on crates.io
- Updated all backend crates to version 0.3.0 to match the new naming
  - `dear-imgui-wgpu` 0.3.0
  - `dear-imgui-glow` 0.3.0
  - `dear-imgui-winit` 0.3.0
- Updated extension crates to depend on `dear-imgui-rs` 0.3
  - `dear-implot` 0.3.0
  - `dear-imnodes` 0.2.0
  - `dear-imguizmo` 0.2.0

### Migration Guide

To migrate from `dear-imgui` 0.2.x to `dear-imgui-rs` 0.3.x:

1. Update your `Cargo.toml`:
   ```toml
   # Before
   dear-imgui = "0.2"

   # After
   dear-imgui-rs = "0.3"
   ```

2. Update your imports:
   ```rust
   // Before
   use dear_imgui::*;

   // After
   use dear_imgui_rs::*;
   ```

3. Update backend dependencies if you use them:
   ```toml
   dear-imgui-wgpu = "0.3"
   dear-imgui-glow = "0.3"
   dear-imgui-winit = "0.3"
   ```

No API changes were made - only the crate name changed.

## [0.1.0] - 2025-09-13

### Added
- Initial release of dear-imgui Rust bindings with docking support
- Support for Dear ImGui v1.92 features
- Backend support for winit, wgpu, and glow
- Extension support for implot

### Features
- Core dear-imgui bindings with safe Rust API
- Docking support (enabled by default)
- Comprehensive backend ecosystem

### Crates
- `dear-imgui-sys`: Low-level FFI bindings
- `dear-imgui`: High-level safe Rust API
- `dear-imgui-winit`: Winit backend integration
- `dear-imgui-wgpu`: WGPU renderer backend
- `dear-imgui-glow`: OpenGL/GLOW renderer backend
- `dear-implot-sys`: ImPlot FFI bindings
- `dear-implot`: ImPlot Rust API
