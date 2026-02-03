# dear-imgui-rs

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-rs.svg)](https://crates.io/crates/dear-imgui-rs)
[![Documentation](https://docs.rs/dear-imgui-rs/badge.svg)](https://docs.rs/dear-imgui-rs)
[![Crates.io Downloads](https://img.shields.io/crates/d/dear-imgui-rs.svg)](https://crates.io/crates/dear-imgui-rs)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`dear-imgui-rs` is a Rust bindings ecosystem for Dear ImGui, featuring docking support, WGPU/GL backends, and a rich set of extensions (ImPlot/ImPlot3D, ImGuizmo/ImGuIZMO.quat, ImNodes, file browser, reflection-based UI).

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/game-engine-docking.png" alt="Docking" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-basic.png" alt="ImGuizmo" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot-basic.png" alt="ImPlot" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imnodes-basic.png" alt="ImNodes" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-quat-basic.png" alt="ImGuizmo.Quat" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot3d-basic.png" alt="ImPlot3D" width="49%"/>
  <br/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/wasm.png" alt="WASM" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/docking-sdl3-glow.png" alt="Docking" width="49%"/>

</p>

## What’s in this repo

- Core
  - `dear-imgui-sys` — low‑level FFI via cimgui (docking branch), bindgen against Dear ImGui v1.92.5
  - `dear-imgui-rs` — safe, idiomatic Rust API (RAII + builder style similar to imgui-rs)
  - Backends: `dear-imgui-wgpu`, `dear-imgui-glow`, `dear-imgui-ash`, `dear-imgui-winit`, `dear-imgui-sdl3`
  - `dear-app` — convenient Winit + WGPU application runner (docking, themes, add-ons)
- Extensions
  - `dear-imguizmo` — 3D gizmo (cimguizmo C API) + a pure‑Rust GraphEditor
  - `dear-imnodes` — node editor (cimnodes C API)
  - `dear-implot` — plotting (cimplot C API)
  - `dear-implot3d` — 3D plotting (cimplot3d C API)
  - `dear-imguizmo-quat` — quaternion + 3D gizmo (cimguizmo_quat C API)
  - `dear-file-browser` — native dialogs (rfd) + pure ImGui in-UI file browser
  - `dear-imgui-reflect` — reflection-based UI helpers (auto-generate ImGui widgets from Rust types)

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
// Tip: pass `.opened(&mut open)` if you want a title-bar close button (X).

// Tip: For fallible creation, use `Context::try_create()`
```

## Examples

```bash
# Clone with submodules
git clone https://github.com/Latias94/dear-imgui-rs
git submodule update --init --recursive

# Core & docking examples
cargo run --bin game_engine_docking
cargo run --bin dockspace_minimal

# dear-app examples (application runner with docking support)
cargo run --bin dear_app_quickstart
cargo run --bin dear_app_docking

# Extension examples (using wgpu + winit directly)
cargo run --bin imguizmo_basic --features imguizmo
cargo run --bin imnodes_basic --features imnodes
cargo run --bin implot_basic --features implot
cargo run --bin imguizmo_quat_basic --features imguizmo-quat
cargo run --bin reflect_demo --features reflect

# implot3d example (uses dear-app)
cargo run --bin implot3d_basic --features implot3d

# WebAssembly (WASM) web demo (import-style, ImGui + optional ImPlot/ImPlot3D/ImNodes/ImGuizmo/ImGuIZMO.quat)
# Note: this import-style WASM path is developed on `main` and shipped in the 0.7.x+ release trains.
# For the full setup (bindings generation, web demo build, provider build, and troubleshooting),
# see the "WebAssembly (WASM) support" section below and docs/WASM.md.

# SDL3 backends (native)
# SDL3 + OpenGL3 with official C++ backends (multi-viewport via imgui_impl_sdl3/imgui_impl_opengl3)
cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3
# SDL3 + Glow (experimental multi-viewport using Rust Glow renderer)
cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features multi-viewport,sdl3-platform
# SDL3 + WGPU (single-window)
cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform
# SDL3 + WGPU (experimental multi-viewport, native only)
cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport

# winit + WGPU (experimental multi-viewport testbed, native only)
# Enabled on Windows/macOS/Linux; tested on Windows/macOS, Linux untested.
cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport
```

Tip: The ImNodes example includes multiple tabs (Hello, Multi-Editor, Style, Advanced Style, Save/Load, Color Editor, Shader Graph, MiniMap Callback).

See `examples/README.md` for a curated index and the planned from‑easy‑to‑advanced layout.

### File Browser

```bash
# OS-native dialogs (rfd)
cargo run --bin file_dialog_native --features file-browser

# Pure ImGui in-UI file browser
cargo run --bin file_browser_imgui --features file-browser
```

## Installation

### Core + Backends

```toml
[dependencies]
dear-imgui-rs = "0.8"
# Choose a backend + platform integration
dear-imgui-wgpu = "0.8"   # or dear-imgui-glow
dear-imgui-winit = "0.8"
```

### Application Runner (Recommended for Quick Start)

```toml
[dependencies]
dear-app = "0.8"  # Includes dear-imgui-rs, wgpu backend, and docking support
```

### Extensions

```toml
[dependencies]
# Plotting
dear-implot = "0.8"      # 2D plotting
dear-implot3d = "0.8"    # 3D plotting

# 3D Gizmos
dear-imguizmo = "0.8"         # Standard 3D gizmo + GraphEditor
dear-imguizmo-quat = "0.8"    # Quaternion-based gizmo

# Node Editor
dear-imnodes = "0.8"

# File Browser
dear-file-browser = "0.8"  # Native dialogs + ImGui file browser

# Reflection-based UI helpers
dear-imgui-reflect = "0.8"
```

### Reflection-based UI (dear-imgui-reflect)

`dear-imgui-reflect` lets you derive `ImGuiReflect` on your structs/enums and automatically get Dear ImGui editors for them. It is inspired by the C++ ImReflect library but implemented in pure Rust on top of `dear-imgui-rs`.

Typical flow:

```rust
use dear_imgui_reflect as reflect;
use reflect::ImGuiReflect;
use reflect::ImGuiReflectExt;

#[derive(ImGuiReflect, Default)]
struct Settings {
    #[imgui(slider, min = 0, max = 100)]
    volume: i32,
    fullscreen: bool,
}

fn ui_frame(ui: &reflect::imgui::Ui, settings: &mut Settings) {
    ui.input_reflect("Settings", settings);
}
```

## Build Strategy

- Default: build from source on all platforms. Prebuilt binaries are optional and off by default.
- Windows: we publish prebuilt packages (MD/MT, with/without `freetype`). Linux/macOS may have CI artifacts but are not used automatically.
- Opt-in prebuilt download from Release: enable the crate feature `prebuilt` (the env toggle `<CRATE>_SYS_USE_PREBUILT=1` is still accepted but requires that feature). Otherwise builds only use prebuilt when you explicitly point to them (e.g., `<CRATE>_SYS_LIB_DIR` or `<CRATE>_SYS_PREBUILT_URL`).

Env vars per -sys crate:
- `<CRATE>_SYS_LIB_DIR` — link from a dir containing the static lib
- `<CRATE>_SYS_PREBUILT_URL` — explicit URL or local path to `.a/.lib` or `.tar.gz` (HTTP(S) and `.tar.gz` extraction require feature `prebuilt`)
- `<CRATE>_SYS_USE_PREBUILT=1` — allow auto download from GitHub Releases (requires feature `prebuilt`)
- `<CRATE>_SYS_PACKAGE_DIR` — local dir with `.tar.gz` packages
- `<CRATE>_SYS_CACHE_DIR` — cache root for downloads/extraction
- `<CRATE>_SYS_SKIP_CC` — skip C/C++ compilation
- `<CRATE>_SYS_FORCE_BUILD` — force source build
- `IMGUI_SYS_USE_CMAKE` / `IMPLOT_SYS_USE_CMAKE` — prefer CMake when available; otherwise cc
- `CARGO_NET_OFFLINE=true` — forbid network; use only local packages or repo prebuilt

Freetype: enable once anywhere. Turning on `freetype` in any extension (imnodes/imguizmo/implot) propagates to `dear-imgui-sys`. When using a prebuilt `dear-imgui-sys` with freetype, ensure the package manifest includes `features=freetype` (our packager writes this).

Quick examples (enable auto prebuilt download):

- Feature: `cargo build -p dear-imgui-sys --features prebuilt`
- Env (Unix): `IMGUI_SYS_USE_PREBUILT=1 cargo build -p dear-imgui-sys --features prebuilt`
- Env (Windows PowerShell): `$env:IMGUI_SYS_USE_PREBUILT='1'; cargo build -p dear-imgui-sys --features prebuilt`

## Compatibility (Latest)

The workspace follows a release-train model. The table below lists the latest, recommended combinations. See [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for full history and upgrade notes.

Core

| Crate           | Version | Notes                                     |
|-----------------|---------|-------------------------------------------|
| dear-imgui-rs   | 0.8.x   | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.8.x   | Binds Dear ImGui v1.92.5 (docking branch) |

Backends

| Crate            | Version | External deps     | Notes                          |
|------------------|---------|-------------------|--------------------------------|
| dear-imgui-wgpu  | 0.8.x   | wgpu = 28         | WebGPU renderer (experimental multi-viewport on native via winit/SDL3; disabled on wasm) |
| dear-imgui-glow  | 0.8.x   | glow = 0.16       | OpenGL renderer (winit/glutin) |
| dear-imgui-winit | 0.8.x   | winit = 0.30.12   | Winit platform backend         |
| dear-imgui-sdl3  | 0.8.x   | sdl3 = 0.17       | SDL3 platform backend (C++ imgui_impl_sdl3/GL3) |

Application Runner

| Crate     | Version | Requires dear-imgui-rs | Notes                                            |
|-----------|---------|------------------------|--------------------------------------------------|
| dear-app  | 0.8.x   | 0.8.x                  | App runner (docking, themes, add-ons)            |

Extensions

| Crate               | Version | Requires dear-imgui-rs | Sys crate                   | Notes                                  |
|---------------------|---------|------------------------|-----------------------------|----------------------------------------|
| dear-implot         | 0.8.x   | 0.8.x                  | dear-implot-sys 0.8.x       | 2D plotting                            |
| dear-imnodes        | 0.8.x   | 0.8.x                  | dear-imnodes-sys 0.8.x      | Node editor                            |
| dear-imguizmo       | 0.8.x   | 0.8.x                  | dear-imguizmo-sys 0.8.x     | 3D gizmo + GraphEditor                 |
| dear-file-browser   | 0.8.x   | 0.8.x                  | —                           | ImGui UI + native (rfd) backends       |
| dear-implot3d       | 0.8.x   | 0.8.x                  | dear-implot3d-sys 0.8.x     | 3D plotting                            |
| dear-imguizmo-quat  | 0.8.x   | 0.8.x                  | dear-imguizmo-quat-sys 0.8.x| Quaternion gizmo                       |
| dear-imgui-reflect  | 0.8.x   | 0.8.x                  | —                           | Reflection-based UI helpers (pure Rust)|

Note: if you need `wgpu = 27` (or an older toolchain), use the 0.7.x train. The latest core patch is
`dear-imgui-rs 0.7.1` (core-only); the rest of the workspace crates remain at 0.7.0.

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

- FFI layer is generated from the cimgui “docking” branch matching Dear ImGui v1.92.5.
- We avoid the C++ ABI by using the C API + bindgen. The safe layer mirrors imgui-rs style (RAII + builder).

## Crates (workspace)

```text
dear-imgui-rs/         # Safe Rust bindings (renamed from dear-imgui)
dear-imgui-sys/        # cimgui FFI (docking; ImGui v1.92.5)
backends/
  dear-imgui-wgpu/     # WGPU renderer
  dear-imgui-glow/     # OpenGL renderer
  dear-imgui-winit/    # Winit platform
dear-app/              # Application runner (Winit + WGPU + docking + themes)
extensions/
  dear-imguizmo/       # ImGuizmo + pure‑Rust GraphEditor
  dear-imnodes/        # ImNodes (node editor)
  dear-implot/         # ImPlot (2D plotting)
  dear-implot3d/       # ImPlot3D (3D plotting)
  dear-imguizmo-quat/  # ImGuIZMO.quat (quaternion gizmo)
  dear-file-browser/   # File dialogs (rfd) + pure ImGui browser
  dear-imgui-reflect/  # Reflection-based UI helpers for dear-imgui-rs
```

## WebAssembly (WASM) support

This workspace includes an import-style WASM build that reuses a separate cimgui
provider module (`imgui-sys-v0`) and shares a single `WebAssembly.Memory` between
the Rust app (wasm-bindgen) and the provider.

Status:

- The web demo (`dear-imgui-web-demo`) is wired up and runs on `wasm32-unknown-unknown`.
- The core UI + WGPU backend are supported; clipboard, raw draw callbacks and
  multi-viewport remain disabled on wasm for safety.
- Font atlas access on wasm is available behind an experimental feature flag.
- Import-style WASM bindings and the `xtask wasm-bindgen-*` / `web-demo` / `build-cimgui-provider`
  helpers are developed on `main` and shipped in the 0.7.x release train; for 0.6.x on crates.io,
  use a git dependency on this repository if you need these flows.

Prerequisites:

- Rust target:
  - `rustup target add wasm32-unknown-unknown`
- wasm-bindgen CLI (version must match the crate’s dependency):
  - `cargo install -f wasm-bindgen-cli --version 0.2.105`
- wasm-tools (used by `xtask web-demo` to patch memory imports/exports):
  - `cargo install -f wasm-tools`
- Emscripten SDK (`emsdk`) for building the cimgui provider:
  - Install emsdk and run its env script (`emsdk_env.*`) or set `EMSDK` so that
    `emcc`/`em++` are on `PATH`.

Quick start (web demo):

```bash
# 1) Generate wasm bindings for dear-imgui-sys (optional; xtask will also
#    generate them on-demand if missing, using import module name imgui-sys-v0)
cargo run -p xtask -- wasm-bindgen imgui-sys-v0

# 2) Build the main wasm module + JS glue (optionally enable experimental fonts)
cargo run -p xtask -- web-demo --features experimental-fonts

# 3) Build the cimgui provider (emscripten) and import map
cargo run -p xtask -- build-cimgui-provider

# 4) Serve and open in the browser
python -m http.server -d target/web-demo 8080
# Then open http://127.0.0.1:8080
```

Notes:

- The provider build emits:
  - `target/web-demo/imgui-sys-v0.wasm` and `imgui-sys-v0.js`
  - `imgui-sys-v0-wrapper.js` (ESM wrapper) and an import map entry mapping
    `"imgui-sys-v0"` to `"./imgui-sys-v0-wrapper.js"`.
- `xtask web-demo`:
  - Patches the wasm-bindgen output to import memory from `env.memory` and export it.
  - Patches the JS glue to pass `globalThis.__imgui_shared_memory` to `env.memory`.
- Font atlas mutation on wasm is guarded by the feature:
  - `dear-imgui-rs/wasm-font-atlas-experimental`
  - `examples-wasm/experimental-fonts` turns this on for the web demo only.

For more details and troubleshooting, see `docs/WASM.md`.

## Limitations

- **Multi-viewport support**
  - **SDL3 + OpenGL3**: supported via upstream C++ backends (`imgui_impl_sdl3` + `imgui_impl_opengl3`).
    - Example: `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`
  - **winit + WGPU**: experimental only; not supported for production use (feature `dear-imgui-wgpu/multi-viewport-winit`).
    - Native only, enabled on Windows/macOS/Linux. Linux is currently untested.
	    - To use in your own app, enable:
	      - `dear-imgui-rs/multi-viewport`
	      - `dear-imgui-winit/multi-viewport`
	      - `dear-imgui-wgpu/multi-viewport-winit`
	      and call `Context::enable_multi_viewport()`.
	    - Test example: `cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport`
	    - Editor-style docking example also supports this mode:
	      `cargo run -p dear-imgui-examples --bin game_engine_docking --features multi-viewport`
  - **winit + OpenGL (glow/glutin)**: no official multi-viewport stack at the moment.
    Use SDL3 + OpenGL3 / SDL3 + Glow if you need multi-viewport OpenGL.
  - **SDL3 + WGPU**: experimental multi-viewport on native via Rust WGPU renderer + SDL3 platform backend; wasm/WebGPU remains single-window.
    - Example (native multi-viewport): `cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport`
    - Example (single window): `cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform`
- **WebAssembly (WASM)**: Supported via the import-style build described above; some features
  (clipboard, raw draw callbacks, multi-viewport) remain disabled on wasm.

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
