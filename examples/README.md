# Examples Overview and Roadmap

This workspace ships a single `dear-imgui-examples` crate with multiple example binaries. The goals:

- Start simple: copy/pasteable, single-file examples you can run and adapt quickly.
- Grow complexity gradually: introduce shared utilities only when needed.
- Cover common real-world patterns: docking, textures, plotting, nodes, gizmos.
- Demonstrate multiple backend stacks: `winit` + `wgpu`, and `glow` + `glutin`.

Also see `examples-wasm/` for WebAssembly (WASM) examples.

Run any example with:

- `cargo run -p dear-imgui-examples --bin <name>`
- Extensions require features, for example:
  - `--features dear-implot` for ImPlot
  - `--features dear-imnodes` for ImNodes
  - `--features dear-imguizmo` for ImGuizmo
  - `--features dear-file-browser` for File Browser / Dialogs

Quick picks:

- Hello world: `cargo run -p dear-imgui-examples --bin hello_world`
- Core + docking: `cargo run -p dear-imgui-examples --bin game_engine_docking`
- Docking minimal: `cargo run -p dear-imgui-examples --bin dockspace_minimal`
- WGPU minimal: `cargo run -p dear-imgui-examples --bin wgpu_basic`
- OpenGL + textures: `cargo run -p dear-imgui-examples --bin glow_textures`
- WGPU + textures: `cargo run -p dear-imgui-examples --bin wgpu_textures`
- WGPU RTT Game View: `cargo run -p dear-imgui-examples --bin wgpu_rtt_gameview`
- Console (log): `cargo run -p dear-imgui-examples --bin console_log`
- Asset browser: `cargo run -p dear-imgui-examples --bin asset_browser_grid`
- File dialog (native): `cargo run -p dear-imgui-examples --features dear-file-browser --bin file_dialog_native`
- File browser (ImGui): `cargo run -p dear-imgui-examples --features dear-file-browser --bin file_browser_imgui`
- Style & Fonts + FreeType: `cargo run -p dear-imgui-examples --features freetype --bin style_and_fonts`

Image preview: both `glow_textures` and `wgpu_textures` load `examples/assets/texture.jpg` and show it alongside generated textures.
- Extensions (e.g., ImPlot): `cargo run -p dear-imgui-examples --bin implot_basic --features dear-implot`
  - ImNodes: `cargo run -p dear-imgui-examples --bin imnodes_basic --features dear-imnodes`
  - ImGuizmo: `cargo run -p dear-imgui-examples --bin imguizmo_basic --features dear-imguizmo`

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
  - multi-viewport (planned, if/when supported in backends).

- 02-docking
  - `dockspace_minimal.rs`: enable docking + a simple dockspace.
  - `game_engine_docking.rs`: complex Unity-style layout, tabs, panels.
    - Menu already includes: Reset to Unity Layout, Save INI, Load INI.
    - Note: INI path is relative to the process CWD; see INI section below.

- 03-extensions (feature-gated)
  - ImPlot: `implot_basic.rs`.
  - ImNodes: `imnodes_basic.rs` with multiple tabs.
  - ImGuizmo: `imguizmo_basic.rs` + notes on camera math.

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
- Extensions: `implot_basic.rs`, `imnodes_basic.rs`, `imguizmo_basic.rs`

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
  - `cargo run -p dear-imgui-examples --features freetype --bin style_and_fonts`
- Requires system FreeType + pkg-config:
  - Windows: MSYS2 (`pacman -S mingw-w64-ucrt-x86_64-freetype pkgconf`) or vcpkg.
  - Linux/macOS: install `freetype` and `pkg-config` via your package manager.
- With `freetype` enabled, `style_and_fonts` can load OTF/CFF and color emoji fonts (e.g. `NotoColorEmoji.ttf`).

## Example ideas (next up)

- “Unity layout” DockBuilder (proportional splits, tabs) with a one-click reset.
- Texture upload and dynamic updates in WGPU matching `glow_textures.rs` capabilities.
- InputText best-practice demos: `String` with `capacity_hint`, and zero-copy `ImString` fields.
- Table angled headers (Ex) demo exercising custom header data.

## How to contribute examples

- Keep small examples single-file and copy/pasteable.
- Use the high-level safe API; put `unsafe` behind helpers inside `support/` if needed.
- For FFI interop or raw enums, cast to `dear_imgui_sys` typedefs (avoid raw `as i32/u32`).
- Prefer assets and INI files under `examples/` and reference them with repo-relative paths.
