# Dear ImGui (Rust) Workspace

[![Crates.io](https://img.shields.io/crates/v/dear-imgui.svg)](https://crates.io/crates/dear-imgui)
[![Documentation](https://docs.rs/dear-imgui/badge.svg)](https://docs.rs/dear-imgui)
[![Crates.io Downloads](https://img.shields.io/crates/d/dear-imgui.svg)](https://crates.io/crates/dear-imgui)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Rust bindings and ecosystem around Dear ImGui, with docking, WGPU/GL backends, and popular extensions (ImGuizmo, ImNodes, ImPlot).

<p align="center">
  <img src="screenshots/game-engine-docking.png" alt="Docking" width="49%"/>
  <img src="screenshots/imguizmo-basic.png" alt="ImGuizmo" width="49%"/>
  <br/>
  <img src="screenshots/implot-basic.png" alt="ImPlot" width="49%"/>
  <img src="screenshots/imnodes-basic.png" alt="ImNodes Shader Graph" width="49%"/>
</p>

## What’s in this repo

- Core
  - `dear-imgui-sys` — low‑level FFI via cimgui (docking branch), bindgen against Dear ImGui v1.92.3
  - `dear-imgui` — safe, idiomatic Rust API (RAII + builder style similar to imgui-rs)
  - Backends: `dear-imgui-wgpu`, `dear-imgui-glow`, `dear-imgui-winit`
- Extensions
  - `dear-imguizmo` — 3D gizmo (cimguizmo C API) + a pure‑Rust GraphEditor
  - `dear-imnodes` — node editor (cimnodes C API)
  - `dear-implot` — plotting (cimplot C API)

All crates are maintained together in this workspace.

## Hello, ImGui (Hello World)

```rust
use dear_imgui::*;

let mut ctx = Context::create_or_panic();
let ui = ctx.frame();
ui.window("Hello")
  .size([300.0, 120.0], Condition::FirstUseEver)
  .build(|| {
      ui.text("Hello, world!");
      if ui.button("Click me") { println!("clicked"); }
  });
// Rendering is done by a backend (e.g. dear-imgui-wgpu or dear-imgui-glow)
```

## Examples

```bash
# Clone with submodules
git clone https://github.com/Latias94/dear-imgui
git submodule update --init --recursive

# Core & docking
cargo run -p dear-imgui-examples --bin game_engine_docking

# Extensions
cargo run -p dear-imgui-examples --bin imguizmo_basic   --features dear-imguizmo
cargo run -p dear-imgui-examples --bin imnodes_basic    --features dear-imnodes
cargo run -p dear-imgui-examples --bin implot_basic     --features dear-implot
```

Tip: The ImNodes example includes multiple tabs (Hello, Multi-Editor, Style, Advanced Style, Save/Load, Color Editor, Shader Graph, MiniMap Callback).

## Installation

```toml
[dependencies]
dear-imgui = "0.2"
# choose a backend + platform integration
dear-imgui-wgpu = "0.2"   # or dear-imgui-glow
dear-imgui-winit = "0.2"
```

## Compatibility (Latest)

The workspace follows a release-train model. The table below lists the latest, recommended combinations. See [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) for full history and upgrade notes.

Core

| Crate           | Version | Notes                                     |
|-----------------|---------|-------------------------------------------|
| dear-imgui      | 0.2.x   | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.2.x   | Binds Dear ImGui v1.92.3 (docking branch) |

Backends

| Crate            | Version | External deps         | Notes |
|------------------|---------|-----------------------|-------|
| dear-imgui-wgpu  | 0.2.x   | wgpu = 26             |       |
| dear-imgui-glow  | 0.2.x   | glow = 0.16           |       |
| dear-imgui-winit | 0.2.x   | winit = 0.30.12       |       |

Extensions

| Crate         | Version | Requires dear-imgui | Sys crate         | Notes |
|---------------|---------|---------------------|-------------------|-------|
| dear-implot   | 0.2.x   | 0.2.x               | dear-implot-sys 0.2.x |     |
| dear-imnodes  | 0.1.x   | 0.2.x               | dear-imnodes-sys 0.1.x |     |
| dear-imguizmo | 0.1.x   | 0.2.x               | dear-imguizmo-sys 0.1.x |    |

Maintenance rules

- Upgrade dear-imgui-sys together with all -sys extensions to avoid C ABI/API drift.
- dear-imgui upgrades may require minor changes in backends/extensions if public APIs changed.
- Backend external deps (wgpu/winit/glow) have their own breaking cycles and may drive backend bumps independently.

## Prebuilt vs Build-From-Source

- Default strategy: prebuilt (with fallback). All `-sys` crates default to feature `prebuilt`.
  - Prefer using prebuilt static libraries when available (download or local package),
    otherwise fall back to building from source (cc/CMake) automatically.
- Force build from source: disable defaults and enable `build-from-source`.
  - Example (single crate): `cargo build -p dear-imgui-sys --no-default-features --features "docking,build-from-source"`

### Environment Variables (per -sys crate)

- `<CRATE>_SYS_LIB_DIR` — link directly from a directory containing the static library.
- `<CRATE>_SYS_PREBUILT_URL` — direct file URL to `.a/.lib` or `.tar.gz` package; `.tar.gz` is extracted to a cache.
- `<CRATE>_SYS_PACKAGE_DIR` — local directory containing `.tar.gz` packages (no network).
- `<CRATE>_SYS_CACHE_DIR` — cache root for downloads and extractions (default under `target/<crate>-prebuilt`).
- `<CRATE>_SYS_SKIP_CC` — skip C/C++ compilation path.
- `<CRATE>_SYS_FORCE_BUILD` — force building from source (same effect as `--features build-from-source`).
- `IMGUI_SYS_USE_CMAKE` / `IMPLOT_SYS_USE_CMAKE` — prefer building via CMake when available, otherwise fall back to cc.
- `CARGO_NET_OFFLINE=true` — forbid network access; use local packages or repo prebuilt only.

Note: Use env vars for fine-grained control per crate (e.g., prebuilt for ImPlot while building ImNodes from source). Use features for a global strategy switch.

### CI (Prebuilt Binaries)

- Workflow: `.github/workflows/prebuilt-binaries.yml`
  - Inputs:
    - `tag` (release) or `branch` (manual; default `main`)
    - `crates`: comma-separated list (`all`, `dear-imgui-sys`, `dear-implot-sys`, `dear-imnodes-sys`, `dear-imguizmo-sys`)
  - Artifacts (branch builds) or Release assets (tag builds) include `.tar.gz` packages named:
    `dear-<name>-prebuilt-<version>-<target>-static[-mt|-md].tar.gz`


## Version & FFI

- FFI layer is generated from the cimgui “docking” branch matching Dear ImGui v1.92.3.
- We avoid the C++ ABI by using the C API + bindgen. The safe layer mirrors imgui-rs style (RAII + builder).

## Crates (workspace)

```text
dear-imgui/            # Safe Rust bindings
dear-imgui-sys/        # cimgui FFI (docking; ImGui v1.92.3)
backends/
  dear-imgui-wgpu/     # WGPU renderer
  dear-imgui-glow/     # OpenGL renderer
  dear-imgui-winit/    # Winit platform
extensions/
  dear-imguizmo/       # ImGuizmo + pure‑Rust GraphEditor
  dear-imnodes/        # ImNodes (node editor)
  dear-implot/         # ImPlot (plotting)
```

## Limitations

- **Multi-viewport support**: Currently not supported
- **WebAssembly (WASM)**: Currently not supported

## Related Projects

If you're working with graphics applications in Rust, you might also be interested in:

- **[asset-importer](https://github.com/Latias94/asset-importer)** - A comprehensive Rust binding for the latest [Assimp](https://github.com/assimp/assimp) 3D asset import library, providing robust 3D model loading capabilities for graphics applications

## Acknowledgments

This project builds upon the excellent work of several other projects:

- **[Dear ImGui](https://github.com/ocornut/imgui)** by Omar Cornut - The original C++ immediate mode GUI library
- **[imgui-rs](https://github.com/imgui-rs/imgui-rs)** - Provided the API design patterns and inspiration for the Rust binding approach
- **[easy-imgui-rs](https://github.com/rodrigorc/easy-imgui-rs/)** by rodrigorc
- **[imgui-wgpu-rs](https://github.com/Yatekii/imgui-wgpu-rs/)** - Provided reference implementation for WGPU backend integration

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (<http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (<http://opensource.org/licenses/MIT>)
