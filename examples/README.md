# Examples Overview and Roadmap

This workspace ships a single `dear-imgui-examples` crate with multiple example binaries. The goals:

- Start simple: copy/pasteable, single-file examples you can run and adapt quickly.
- Grow complexity gradually: introduce shared utilities only when needed.
- Cover common real-world patterns: docking, textures, plotting, nodes, gizmos.
- Demonstrate multiple backend stacks: `winit` + `wgpu`, and `glow` + `glutin`.

Also see `examples-wasm/` for WebAssembly (WASM) examples.

Run any example with:

- `cargo run --bin <name>`
- Extensions require features, for example:
  - `--features implot` for ImPlot
  - `--features imnodes` for ImNodes
  - `--features imguizmo` for ImGuizmo
  - `--features imguizmo-quat` for ImGuIZMO.quat
  - `--features implot3d` for ImPlot3D (and, if your workspace doesn’t pre-enable add-ons on dear-app, also add `, dear-app/implot3d`)
  - `--features file-browser` for File Browser / Dialogs
  - `--features reflect` for dear-imgui-reflect demo

Quick picks:

- Hello world: `cargo run --bin hello_world`
- Core + docking: `cargo run --bin game_engine_docking`
  - With ImPlot (FPS graph): `cargo run --bin game_engine_docking --features implot`
  - With ImGuizmo (3D gizmo): `cargo run --bin game_engine_docking --features imguizmo`
  - With all extensions: `cargo run --bin game_engine_docking --features "implot,imguizmo"`
  - With experimental multi-viewport (winit + WGPU, native only): `cargo run --bin game_engine_docking --features multi-viewport`
- Docking minimal: `cargo run --bin dockspace_minimal`
- WGPU minimal: `cargo run --bin wgpu_basic`
- OpenGL + textures: `cargo run --bin glow_textures`
- WGPU + textures: `cargo run --bin wgpu_textures`
- ImGuIZMO.quat (WGPU): `cargo run --features imguizmo-quat --bin imguizmo_quat_basic`
- WGPU RTT Game View: `cargo run --bin wgpu_rtt_gameview`
- Console (log): `cargo run --bin console_log`
- Asset browser: `cargo run --bin asset_browser_grid`
- File dialog (native): `cargo run --features file-browser --bin file_dialog_native`
- File browser (ImGui): `cargo run --features file-browser --bin file_browser_imgui`
- Style & Fonts + FreeType: `cargo run --features freetype --bin style_and_fonts`
- ImPlot3D Demo: `cargo run --bin implot3d_basic --features "implot3d"`
  - If your workspace doesn’t pre-enable dear-app add-on features, use: `cargo run --bin implot3d_basic --features "implot3d, dear-app/implot3d"`
- Reflect demo: `cargo run --bin reflect_demo --features reflect`

Image preview: both `glow_textures` and `wgpu_textures` load `examples/assets/texture.jpg` and show it alongside generated textures.
- Extensions (e.g., ImPlot): `cargo run --bin implot_basic --features implot`
  - ImNodes: `cargo run --bin imnodes_basic --features imnodes`
  - ImGuizmo: `cargo run --bin imguizmo_basic --features imguizmo`

## Structure (from easy to advanced)

This is the intended organization.

- 00-quickstart (single-file, no shared utils)
  - `hello_world.rs`: context, a window, a button.
  - `wgpu_basic.rs`: minimal WGPU renderer + platform.
  - `glow_basic.rs`: minimal OpenGL renderer + platform.
  - `input_text_minimal.rs`: basic text inputs (String + ImString), multiline.
  - `tables_minimal.rs`: basic Tables API (sort/resize/reorder).
  - `drawlist_minimal.rs`: primitives and shapes.
  - `menus_and_popups.rs`: main/window menu bars, context menu, modal popup.
  - `tables_property_grid.rs`: 2-column property grid (labels + editors).
  - `list_clipper_log.rs`: virtualized log with filtering and context actions.
  - `style_and_fonts.rs`: theme switching, StyleVar demo, and font merging.

- 01-renderers (single-file, backend topics)
  - `glow_textures.rs`: modern texture system (register/update).
  - `wgpu_textures.rs`: CPU-updated texture registered in WGPU backend.
  - `sdl3_wgpu.rs`: SDL3 window + WGPU renderer (single window; uses official SDL3 platform backend).
    - Run with: `cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform`
    - For native multi-viewport, see `sdl3_wgpu_multi_viewport.rs` below.

- 02-docking
  - `dockspace_minimal.rs`: enable docking + a simple dockspace.
  - `game_engine_docking.rs`: complex Unity-style layout, tabs, panels.
    - Menu already includes: Reset to Unity Layout, Save INI, Load INI.
    - Optional extensions: `--features implot` (FPS graph), `--features imguizmo` (3D gizmo manipulation).
    - Note: INI path is relative to the process CWD; see INI section below.
  - `multi_viewport_wgpu.rs`: **Experimental test example only** - winit + WGPU multi-viewport support is not production-ready.
    - Run with: `cargo run --bin multi_viewport_wgpu --features multi-viewport`
    - Native only, enabled on Windows/macOS/Linux; tested on Windows/macOS, Linux untested.
    - The examples feature `multi-viewport` enables the required backend features:
      `dear-imgui-winit/multi-viewport` + `dear-imgui-wgpu/multi-viewport-winit`.
    - Important pattern: only the main window should drive WGPU rendering and surface resize;
      secondary viewport windows are rendered via ImGui platform/renderer callbacks.
  - `sdl3_opengl_multi_viewport.rs`: SDL3 + OpenGL3 multi-viewport example using official C++ backends.
    - Run with: `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`
    - Shows how to combine SDL3 platform backend with OpenGL renderer, including a simple "Game View" texture inside an ImGui window that can be dragged across OS windows.
    - Relies on the official OpenGL3 renderer in `dear-imgui-sdl3` (feature `opengl3-renderer`).
  - `sdl3_glow_multi_viewport.rs`: SDL3 + Glow multi-viewport example using Rust Glow renderer backend.
    - Run with: `cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features multi-viewport,sdl3-platform`
    - Uses SDL3 platform backend for window/GL context management and `dear-imgui-glow` for rendering all viewports.
  - `sdl3_wgpu_multi_viewport.rs`: SDL3 + WGPU multi-viewport example (experimental, native only) using Rust WGPU renderer backend.
    - Run with: `cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport`
    - Uses SDL3 platform backend for window management and `dear-imgui-wgpu` to render all viewports.

- 03-extensions (feature-gated)
  - ImPlot: `implot_basic.rs`.
  - ImNodes: `imnodes_basic.rs` with multiple tabs.
  - ImGuizmo: `imguizmo_basic.rs` + notes on camera math.
  - Reflect: `reflect_demo.rs` (struct/enum reflection + auto-generated UI).

- 04-integration patterns (real-world snippets)
  - `wgpu_rtt_gameview.rs`: Render-to-texture Game View drawn in an ImGui window.
  - `console_log.rs`: Logging console with filter, autoscroll, and history.
  - `asset_browser_grid.rs`: Asset browser grid with thumbnails and filter.
  - `file_dialog_native.rs`: OS-native file dialog using `dear-file-browser` (non-blocking thread).
  - `file_browser_imgui.rs`: Pure ImGui file browser widget.

- support/
  - `wgpu_init.rs` (WGPU init + resize helpers)
  - Additional helpers may be introduced as examples grow.

## Mapping current examples

- Quickstart/backends: `wgpu_basic.rs`, `glow_basic.rs`, `input_text_minimal.rs`, `tables_minimal.rs`, `drawlist_minimal.rs`, `glow_textures.rs`, `menus_and_popups.rs`, `tables_property_grid.rs`, `list_clipper_log.rs`, `style_and_fonts.rs`
- Docking: `02-docking/dockspace_minimal.rs`, `02-docking/game_engine_docking.rs` (+ `examples/02-docking/game_engine_docking.ini`)
- Integration: `04-integration/wgpu_rtt_gameview.rs`, `04-integration/console_log.rs`, `04-integration/asset_browser_grid.rs`
  and `04-integration/file_dialog_native.rs`, `04-integration/file_browser_imgui.rs`
- Extensions: `implot_basic.rs`, `imnodes_basic.rs`, `imguizmo_basic.rs`, `reflect_demo.rs`

### dear-app helpers

These examples use the `dear-app` runner (Winit + WGPU) with minimal code:

- Quickstart: `cargo run --bin dear_app_quickstart`
- Docking template: `cargo run --bin dear_app_docking`

These will remain runnable as-is; future additions may live in subfolders with explicit `[[bin]]` paths.

## INI handling and paths

Dear ImGui persists window/docking state via an INI file. Useful patterns:

- Disable persistence (useful while testing layouts):
  - `context.set_ini_filename(Option::<std::path::PathBuf>::None)?;`
  - or `context.set_ini_filename::<std::path::PathBuf>(None)?;`
- Use a known INI path relative to the repo for reproducible layouts:
  - `context.set_ini_filename(Some(std::path::PathBuf::from("examples/02-docking/game_engine_docking.ini")))?;`
- When mixing DockBuilder with INI restore, apply builder only on a pristine state (first run or after reset), then let INI drive subsequent runs.

Tip: On Windows/macOS, CWD may differ when launching from an IDE. Prefer absolute or repo-relative paths when you need deterministic INI behavior.

## FreeType (OTF/Color Emoji)

- Enable at build time:
  - `cargo run --features freetype --bin style_and_fonts`
- Requires system FreeType + pkg-config:
  - Windows: MSYS2 (`pacman -S mingw-w64-ucrt-x86_64-freetype pkgconf`) or vcpkg.
  - Linux/macOS: install `freetype` and `pkg-config` via your package manager.
- With `freetype` enabled, `style_and_fonts` can load OTF/CFF and color emoji fonts (e.g. `NotoColorEmoji.ttf`).

## Example ideas (next up)

- “Unity layout�?DockBuilder (proportional splits, tabs) with a one-click reset.
- Texture upload and dynamic updates in WGPU matching `glow_textures.rs` capabilities.
- InputText best-practice demos: `String` with `capacity_hint`, and zero-copy `ImString` fields.
- Table angled headers (Ex) demo exercising custom header data.

## How to contribute examples

- Keep small examples single-file and copy/pasteable.
- Use the high-level safe API; put `unsafe` behind helpers inside `support/` if needed.
- For FFI interop or raw enums, cast to `dear_imgui_sys` typedefs (avoid raw `as i32/u32`).
- Prefer assets and INI files under `examples/` and reference them with repo-relative paths.
