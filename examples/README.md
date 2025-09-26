# Examples Overview and Roadmap

This workspace ships a single `dear-imgui-examples` crate with multiple example binaries. The goals:

- Start simple: copy‑pasteable, single‑file examples you can run and adapt quickly.
- Grow complexity gradually: introduce shared utilities only when needed.
- Cover common real‑world patterns: docking, textures, plotting, nodes, gizmos.
- Demonstrate multiple backend stacks: `winit` + `wgpu`, and `glow` + `glutin`.

Run any example with:

- `cargo run -p dear-imgui-examples --bin <name>`
- Extensions require features, for example:
  - `--features dear-implot` for ImPlot
  - `--features dear-imnodes` for ImNodes
  - `--features dear-imguizmo` for ImGuizmo

Quick picks:

- Hello world: `cargo run -p dear-imgui-examples --bin hello_world`
- Core + docking: `cargo run -p dear-imgui-examples --bin game_engine_docking`
- Docking minimal: `cargo run -p dear-imgui-examples --bin dockspace_minimal`
- WGPU minimal: `cargo run -p dear-imgui-examples --bin wgpu_basic`
- OpenGL + textures: `cargo run -p dear-imgui-examples --bin glow_textures`
- WGPU + textures: `cargo run -p dear-imgui-examples --bin wgpu_textures`

Image preview: both `glow_textures` and `wgpu_textures` load `examples/assets/texture.jpg` and show it alongside generated textures.
- Extensions (e.g., ImPlot): `cargo run -p dear-imgui-examples --bin implot_basic --features dear-implot`

## Structure (from easy to advanced)

This is the intended organization. We’ll evolve existing examples towards it without breaking current paths.

- 00‑quickstart (single‑file, no shared utils)
  - `hello_world.rs` (existing): context, a window, a button.
  - `wgpu_basic.rs` (existing): minimal WGPU renderer + platform.
  - `glow_basic.rs` (existing): minimal OpenGL renderer + platform.
  - `input_text_minimal.rs` (planned): basic text inputs and callbacks.
  - `tables_minimal.rs` (planned): basic Tables API.
  - `drawlist_minimal.rs` (planned): primitives and shapes.

- 01‑renderers (single‑file, backend topics)
  - `glow_textures.rs` (existing): modern texture system (register/update).
  - `wgpu_textures.rs` (existing): CPU‑updated texture registered in WGPU backend.
  - multi‑viewport (planned, if/when supported in backends).

- 02‑docking
  - `dockspace_minimal.rs` (existing): enable docking + a simple dockspace.
  - `game_engine_docking.rs` (existing): complex Unity‑style layout, tabs, panels.
    - Menu already includes: “Reset to Unity Layout”, “Save INI”, “Load INI”.
    - Note: INI path is relative to the process CWD; see INI section below.

- 03‑extensions (feature‑gated)
  - ImPlot: `implot_basic.rs` (existing).
  - ImNodes: `imnodes_basic.rs` (existing) with multiple tabs.
  - ImGuizmo: `imguizmo_basic.rs` (existing) + notes on camera math.

- 04‑integration patterns (real‑world snippets)
  - Render‑to‑texture “Game View” shown in an ImGui window (planned).
  - Logging console with filter and history (planned).
  - Asset browser grid with thumbnails (planned).

- support/ (shared runner introduced later, not required for quickstarts)
  - `runner.rs` (event loop + frame plumbing)
  - `renderer_wgpu.rs`, `renderer_glow.rs`
  - `platform_winit.rs`
  - `fonts.rs`, `clipboard.rs`, small helpers

- wasm/
  - See `examples-wasm/` in the repo for web targets. We’ll keep content aligned so minimal examples can be reused on native and web when feasible.

## Mapping current examples

- Quickstart/backends: `wgpu_basic.rs`, `glow_basic.rs`, `glow_textures.rs`
- Docking: `game_engine_docking.rs` (+ `game_engine_docking.ini`)
- Extensions: `implot_basic.rs`, `imnodes_basic.rs`, `imguizmo_basic.rs`

These will remain runnable as‑is; future additions may live in subfolders with explicit `[[bin]]` paths.

## INI handling and paths

Dear ImGui persists window/docking state via an INI file. Useful patterns:

- Disable persistence (useful while testing layouts):
  - `context.set_ini_filename(Option::<std::path::PathBuf>::None)?;`
  - or `context.set_ini_filename::<std::path::PathBuf>(None)?;`
- Use a known INI path relative to the repo for reproducible layouts:
  - `context.set_ini_filename(Some(std::path::PathBuf::from("examples/game_engine_docking.ini")))?;`
- When mixing DockBuilder with INI restore, apply builder only on a pristine state (first run or after reset), then let INI drive subsequent runs.

Tip: On Windows/macOS, CWD may differ when launching from an IDE. Prefer absolute or repo‑relative paths when you need deterministic INI behavior.

## Example ideas (next up)

- “Unity layout” DockBuilder (proportional splits, tabs) with a one‑click reset.
- Texture upload and dynamic updates in WGPU matching `glow_textures.rs` capabilities.
- InputText best‑practice demos: `String` with `capacity_hint`, and zero‑copy `ImString` fields.
- Table angled headers (Ex) demo exercising custom header data.

## How to contribute examples

- Keep small examples single‑file and copy‑pasteable.
- Use the high‑level safe API; put `unsafe` behind helpers inside `support/` if needed.
- For FFI interop or raw enums, cast to `dear_imgui_sys` typedefs (avoid raw `as i32/u32`).
- Prefer assets and INI files under `examples/` and reference them with repo‑relative paths.
