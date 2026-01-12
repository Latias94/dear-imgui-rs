# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0] - Not Released

This release focuses on `dear-app` usability improvements for real applications (GPU configuration presets, smoother startup, and clearer redraw semantics).

### Breaking Changes

- `dear-app`
  - `RunnerConfig` gains a new required field: `wgpu: WgpuConfig`. Struct-literal initializers without `..Default::default()` must be updated.
  - `RunnerCallbacks` gains a new field: `on_gpu_init`. Struct-literal initializers must be updated.
  - `RedrawMode::Wait` now truly waits (no implicit per-frame redraw). Use `Poll` or `WaitUntil` for continuous rendering.
- Backends
  - `dear-imgui-winit`: `multi_viewport::ViewportData` is no longer a public API (internal backend detail).
  - `dear-imgui-wgpu`: `multi_viewport::{ViewportWgpuData}` and `multi_viewport_sdl3::{ViewportWgpuData}` are no longer public APIs (internal renderer details).
- `*-sys` crates (prebuilt downloads)
  - Prebuilt downloads/extraction are now gated behind the Cargo feature `prebuilt`. If you set `*_SYS_PREBUILT_URL` to an `http(s)://...` URL or to a `.tar.gz` archive, or set `*_SYS_USE_PREBUILT=1`, you must also enable `--features prebuilt`.
  - Default builds do not enable `prebuilt` (and therefore do not pull in HTTP client dependencies like `ureq`). (Fixes #12)

### Added

- `dear-app`
  - `WgpuConfig` and `RunnerConfig::wgpu`: configure instance/adapter/device selection (backends, power preference, required features/limits, memory hints, etc.).
  - `WgpuPreset` and `WgpuConfig::from_preset`: curated presets for common scenarios (performance, low-power, downlevel compatibility, software fallback).
  - `AppBuilder::on_gpu_init`: a lifecycle hook for one-time GPU resource initialization after `Device/Queue/SurfaceConfiguration` are available.
  - `pub use wgpu;` re-export as `dear_app::wgpu` for downstream convenience.

### Changed

- `dear-app`
  - Theme application now uses the safe high-level `dear-imgui-rs` `Theme/ThemePreset` API (avoids direct `sys::igStyleColors*` usage).
  - Acquire the swapchain texture later in the frame to reduce the time the surface image is held.
  - `restore_previous_geometry = false` now disables INI persistence by forcing the INI filename to `None`.

### Fixed

- `dear-app`
  - Per-frame OS cursor/IME state updates via `prepare_render_with_ui` (more correct cursor shape + IME toggling).
  - More reliable recovery when recreating the window/GPU stack after fatal surface errors; if recreation fails, `on_exit` is called with the old context for cleanup.
  - `WaitUntil { fps }` control flow now uses `fps` to schedule the next wake consistently.

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
