# dear-imgui-rs

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-rs.svg)](https://crates.io/crates/dear-imgui-rs)
[![Documentation](https://docs.rs/dear-imgui-rs/badge.svg)](https://docs.rs/dear-imgui-rs)
[![Crates.io Downloads](https://img.shields.io/crates/d/dear-imgui-rs.svg)](https://crates.io/crates/dear-imgui-rs)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`dear-imgui-rs` is a Rust bindings ecosystem for Dear ImGui, featuring docking support, WGPU/GL backends, and popular extensions (ImGuizmo, ImNodes, ImPlot).

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/game-engine-docking.png" alt="Docking" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-basic.png" alt="ImGuizmo" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot-basic.png" alt="ImPlot" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imnodes-basic.png" alt="ImNodes" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-quat-basic.png" alt="ImGuizmo.Quat" width="49%"/>
<img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot3d-basic.png" alt="ImPlot3D" width="49%"/>
</p>

## What’s in this repo

- Core
  - `dear-imgui-sys` — low‑level FFI via cimgui (docking branch), bindgen against Dear ImGui v1.92.3
  - `dear-imgui-rs` — safe, idiomatic Rust API (RAII + builder style similar to imgui-rs)
  - Backends: `dear-imgui-wgpu`, `dear-imgui-glow`, `dear-imgui-winit`
- Extensions
  - `dear-imguizmo` — 3D gizmo (cimguizmo C API) + a pure‑Rust GraphEditor
  - `dear-imnodes` — node editor (cimnodes C API)
  - `dear-implot` — plotting (cimplot C API)
  - `dear-file-browser` — native dialogs (rfd) + pure ImGui in-UI file browser

All crates are maintained together in this workspace.

## Hello, ImGui (Hello World)

```rust
use dear_imgui_rs::*;

let mut ctx = Context::create();
let ui = ctx.frame();
ui.window("Hello")
  .size([300.0, 120.0], Condition::FirstUseEver)
  .build(|| {
      ui.text("Hello, world!");
      if ui.button("Click me") { println!("clicked"); }
  });
// Rendering is done by a backend (e.g. dear-imgui-wgpu or dear-imgui-glow)

// Tip: For fallible creation, use `Context::try_create()`
```

## Examples

```bash
# Clone with submodules
git clone https://github.com/Latias94/dear-imgui-rs
git submodule update --init --recursive

# Core & docking
cargo run -p dear-imgui-examples --bin game_engine_docking

# Extensions
cargo run -p dear-imgui-examples --bin imguizmo_basic   --features dear-imguizmo
cargo run -p dear-imgui-examples --bin imnodes_basic    --features dear-imnodes
cargo run -p dear-imgui-examples --bin implot_basic     --features dear-implot
```

Tip: The ImNodes example includes multiple tabs (Hello, Multi-Editor, Style, Advanced Style, Save/Load, Color Editor, Shader Graph, MiniMap Callback).

See `examples/README.md` for a curated index and the planned from‑easy‑to‑advanced layout.

### File Browser

```
# OS-native dialogs (rfd)
cargo run -p dear-imgui-examples --features dear-file-browser --bin file_dialog_native

# Pure ImGui in-UI file browser
cargo run -p dear-imgui-examples --features dear-file-browser --bin file_browser_imgui
```

## Installation

```toml
[dependencies]
dear-imgui-rs = "0.3"
# choose a backend + platform integration
dear-imgui-wgpu = "0.3"   # or dear-imgui-glow
dear-imgui-winit = "0.3"
```

## Build Strategy

- Default: build from source on all platforms. Prebuilt binaries are optional and off by default.
- Windows: we publish prebuilt packages (MD/MT, with/without `freetype`). Linux/macOS may have CI artifacts but are not used automatically.
- Opt-in prebuilt download from Release: enable either the crate feature `prebuilt` or set `<CRATE>_SYS_USE_PREBUILT=1`. Otherwise builds only use prebuilt when you explicitly point to them (e.g., `<CRATE>_SYS_LIB_DIR` or `<CRATE>_SYS_PREBUILT_URL`).

Env vars per -sys crate:
- `<CRATE>_SYS_LIB_DIR` — link from a dir containing the static lib
- `<CRATE>_SYS_PREBUILT_URL` — explicit URL to `.a/.lib` or `.tar.gz` (always honored)
- `<CRATE>_SYS_USE_PREBUILT=1` — allow auto download from GitHub Releases
- `<CRATE>_SYS_PACKAGE_DIR` — local dir with `.tar.gz` packages
- `<CRATE>_SYS_CACHE_DIR` — cache root for downloads/extraction
- `<CRATE>_SYS_SKIP_CC` — skip C/C++ compilation
- `<CRATE>_SYS_FORCE_BUILD` — force source build
- `IMGUI_SYS_USE_CMAKE` / `IMPLOT_SYS_USE_CMAKE` — prefer CMake when available; otherwise cc
- `CARGO_NET_OFFLINE=true` — forbid network; use only local packages or repo prebuilt

Freetype: enable once anywhere. Turning on `freetype` in any extension (imnodes/imguizmo/implot) propagates to `dear-imgui-sys`. When using a prebuilt `dear-imgui-sys` with freetype, ensure the package manifest includes `features=freetype` (our packager writes this).

Quick examples (enable auto prebuilt download):

- Feature: `cargo build -p dear-imgui-sys --features prebuilt`
- Env (Unix): `IMGUI_SYS_USE_PREBUILT=1 cargo build -p dear-imgui-sys`
- Env (Windows PowerShell): `$env:IMGUI_SYS_USE_PREBUILT='1'; cargo build -p dear-imgui-sys`

## Compatibility (Latest)

The workspace follows a release-train model. The table below lists the latest, recommended combinations. See [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for full history and upgrade notes.

Core

| Crate           | Version | Notes                                     |
|-----------------|---------|-------------------------------------------|
| dear-imgui-rs   | 0.3.x   | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.3.x   | Binds Dear ImGui v1.92.3 (docking branch) |

Backends

| Crate            | Version | External deps         | Notes |
|------------------|---------|-----------------------|-------|
| dear-imgui-wgpu  | 0.3.x   | wgpu = 26             |       |
| dear-imgui-glow  | 0.3.x   | glow = 0.16           |       |
| dear-imgui-winit | 0.3.x   | winit = 0.30.12       |       |

Extensions

| Crate         | Version | Requires dear-imgui-rs | Sys crate            | Notes |
|---------------|---------|------------------------|----------------------|-------|
| dear-implot   | 0.3.x   | 0.3.x                  | dear-implot-sys 0.3.x |     |
| dear-imnodes  | 0.3.x   | 0.3.x                  | dear-imnodes-sys 0.3.x |     |
| dear-imguizmo | 0.3.x   | 0.3.x                  | dear-imguizmo-sys 0.3.x |    |

Maintenance rules

- Upgrade dear-imgui-sys together with all -sys extensions to avoid C ABI/API drift.
- dear-imgui-rs upgrades may require minor changes in backends/extensions if public APIs changed.
- Backend external deps (wgpu/winit/glow) have their own breaking cycles and may drive backend bumps independently.

### CI (Prebuilt Binaries)

- Workflow: `.github/workflows/prebuilt-binaries.yml`
  - Inputs:
    - `tag` (release) or `branch` (manual; default `main`)
    - `crates`: comma-separated list (`all`, `dear-imgui-sys`, `dear-implot-sys`, `dear-imnodes-sys`, `dear-imguizmo-sys`)
  - Artifacts (branch builds) or Release assets (tag builds) include `.tar.gz` packages named:
    `dear-<name>-prebuilt-<version>-<target>-static[-mt|-md].tar.gz`
  - Release download URLs default to owner/repo configured in `tools/build-support/src/lib.rs`.
    Override via env: `BUILD_SUPPORT_GH_OWNER`, `BUILD_SUPPORT_GH_REPO`.


## Version & FFI

- FFI layer is generated from the cimgui “docking” branch matching Dear ImGui v1.92.3.
- We avoid the C++ ABI by using the C API + bindgen. The safe layer mirrors imgui-rs style (RAII + builder).

## Crates (workspace)

```text
dear-imgui-rs/         # Safe Rust bindings (renamed from dear-imgui)
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
- **[boxdd](https://github.com/Latias94/boxdd)** - Safe, ergonomic Rust bindings for Box2D v3.

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

