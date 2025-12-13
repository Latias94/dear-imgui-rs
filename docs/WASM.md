# WebAssembly (WASM) support

This project supports compiling Dear ImGui + backends to WebAssembly using an import-style design. It follows the approach used by imgui-rs: Rust code links to a WASM import module named `imgui-sys-v0` that provides the cimgui (C API) implementation.

## Quick start

- Status: the import-style WASM path is developed on `main` and shipped in the 0.7.0 release train. For crates.io releases prior to 0.7, these bindings and `xtask` flows are available only when depending on this repository directly (e.g. via a git dependency).

- Prerequisites
  - `rustup target add wasm32-unknown-unknown`
  - `cargo install -f wasm-bindgen-cli --version 0.2.105`
  - `cargo install -f wasm-tools`
  - Emscripten SDK (for provider). On Windows run `emsdk_env.bat`/`.ps1` so `emcc/em++` are on PATH, or set `EMSDK` to emsdk root.

- Build and run (core ImGui + optional ImPlot/ImNodes/ImGuizmo demos)
  1) Generate pregenerated WASM bindings (import-style, `imgui-sys-v0` provider)
     - Dear ImGui core:
       - `cargo run -p xtask -- wasm-bindgen imgui-sys-v0`
     - ImPlot (optional; enables ImPlot for wasm targets):
       - `cargo run -p xtask -- wasm-bindgen-implot imgui-sys-v0`
     - ImPlot3D (optional; enables ImPlot3D for wasm targets):
       - `cargo run -p xtask -- wasm-bindgen-implot3d imgui-sys-v0`
     - ImNodes (optional; enables ImNodes for wasm targets):
       - `cargo run -p xtask -- wasm-bindgen-imnodes imgui-sys-v0`
     - ImGuizmo (optional; enables ImGuizmo for wasm targets):
       - `cargo run -p xtask -- wasm-bindgen-imguizmo imgui-sys-v0`
     - ImGuIZMO.quat (optional; enables ImGuIZMO.quat for wasm targets):
       - `cargo run -p xtask -- wasm-bindgen-imguizmo-quat imgui-sys-v0`
  2) Build the main WASM + JS and patch for shared memory
     - `cargo run -p xtask -- web-demo`
       - By default, the `dear-imgui-web-demo` crate enables:
         - `web-backends` (wgpu/webgl + dear-imgui-wgpu webgl/webgpu)
         - `implot` (ImPlot integration, guarded by the `implot` feature)
  3) Build the cimgui provider (Emscripten)
     - `cargo run -p xtask -- build-cimgui-provider`
       - Builds `imgui-sys-v0` as a single provider module containing:
         - Dear ImGui + cimgui
         - ImPlot + cimplot (when `wasm-bindgen-implot` has been run and bindings exist)
         - ImPlot3D + cimplot3d (when `wasm-bindgen-implot3d` has been run and bindings exist)
         - ImNodes + cimnodes (when `wasm-bindgen-imnodes` has been run and bindings exist)
         - ImGuizmo + cimguizmo (when `wasm-bindgen-imguizmo` has been run and bindings exist)
         - ImGuIZMO.quat + cimguizmo_quat (when `wasm-bindgen-imguizmo-quat` has been run and bindings exist)
  4) Serve and open in browser
     - `python -m http.server -d target/web-demo 8080`
     - Open http://127.0.0.1:8080 and hard-refresh (Ctrl+F5)

- Expected console logs
  - `[imgui-sys-v0] Shared memory pages= …` and `Module.wasmMemory===memory true`
  - `IO set: logical=… framebuffer_scale=… physical=… surface=…`

## Why Emscripten?

This repo deliberately keeps **two roles** separate:

- The main application (Rust + winit + wgpu) targets `wasm32-unknown-unknown` and uses `wasm-bindgen`.
- The cimgui + Dear ImGui implementation is compiled once as a **provider** module (`imgui-sys-v0`).

For the provider we currently use Emscripten because it solves several hard problems for us:

- **Import‑style + shared memory**:
  - Emscripten has first‑class support for importing a shared memory via `-s IMPORTED_MEMORY=1`.
  - This lets the provider reuse the same `WebAssembly.Memory` that wasm‑bindgen exports, so all ImGui IO/state written by Rust is visible to cimgui.
- **Symbol export and ES modules**:
  - We can feed Emscripten a JSON list of symbols derived from the pregenerated wasm bindings and let it emit an ES module that exports exactly those functions.
  - It handles `EXPORTED_FUNCTIONS`, startup, and runtime glue, so the provider can be imported as a normal JS module.
- **Debuggability and decoupling**:
  - The provider build is independent from the Rust build: you can tweak its initial memory, `GLOBAL_BASE`, logging, and assertions without rebuilding Rust.
  - Warnings, logs, and crashes in cimgui/ImGui are isolated in the provider, which simplifies debugging.
- **Portability**:
  - Emscripten ships a battle‑tested sysroot for C/C++ to WebAssembly, including the minimum runtime pieces we need, and is well supported on all major platforms.

In principle, it is possible to build the provider with a “pure” `wasm32-unknown-unknown` toolchain and hand‑write the glue (imports/exports, memory wiring, JS module), but that significantly increases maintenance complexity for relatively little benefit. For now we intentionally keep:

- **Main module**: `wasm32-unknown-unknown` + wasm‑bindgen.
- **Provider module**: Emscripten‑built cimgui + Dear ImGui (`imgui-sys-v0`).

We do *not* currently plan to replace the provider build with a custom single‑module (`wasm32-unknown-unknown` only) design in the short term.

## Overview

- Import-style: the Rust WASM module imports functions from a separate provider module (`imgui-sys-v0`). This avoids compiling C/C++ during the Rust build and keeps the Rust artifact lean.
- We ship xtask commands that:
  - Generate WASM bindings for `dear-imgui-sys` (importing from `imgui-sys-v0`).
  - Build a minimal web demo (wgpu + winit + Dear ImGui).
  - Build the cimgui provider WASM with Emscripten and generate a small ES module that re-exports its symbols under the `imgui-sys-v0` import name.

## Requirements

- Rust target: `wasm32-unknown-unknown`
  - `rustup target add wasm32-unknown-unknown`
- wasm-bindgen CLI (version must match the crate’s dependency)
  - `cargo install -f wasm-bindgen-cli --version 0.2.105`
- wasm-tools (used by xtask to patch the main Wasm to import memory so it can share memory with the provider)
  - `cargo install -f wasm-tools`
- Emscripten SDK (for provider build)
  - Install emsdk and either:
    - Run its environment script (adds `emcc/em++` to PATH), or
    - Set `EMSDK` env var to the emsdk root.

On Windows, run `emsdk.bat` for setup and `emsdk_env.bat` to update PATH for the current shell (PowerShell users can also run `emsdk_env.ps1`).

## Commands (local)

1) Generate WASM bindings (import-style, defaults to `imgui-sys-v0`):

- Dear ImGui core:

```bash
cargo run -p xtask -- wasm-bindgen imgui-sys-v0
```

- ImPlot (optional extension for 2D plotting on wasm):

```bash
cargo run -p xtask -- wasm-bindgen-implot imgui-sys-v0
```

- ImNodes (optional node editor extension on wasm):

```bash
cargo run -p xtask -- wasm-bindgen-imnodes imgui-sys-v0
```

- ImGuizmo (optional gizmo extension on wasm):

```bash
cargo run -p xtask -- wasm-bindgen-imguizmo imgui-sys-v0
```

- ImGuIZMO.quat (optional quaternion gizmo extension on wasm):

```bash
cargo run -p xtask -- wasm-bindgen-imguizmo-quat imgui-sys-v0
```

2) Build the web demo (main WASM + JS; xtask will also patch it to import memory so it can share memory with the provider):

```bash
# Core ImGui only
cargo run -p xtask -- web-demo

# Core ImGui + ImPlot demo
cargo run -p xtask -- web-demo implot

# Core ImGui + ImPlot3D demo
cargo run -p xtask -- web-demo implot3d

# Core ImGui + ImNodes demo
cargo run -p xtask -- web-demo imnodes

# Core ImGui + ImPlot + ImNodes demos
cargo run -p xtask -- web-demo implot,imnodes

# Core ImGui + ImGuizmo demo
cargo run -p xtask -- web-demo imguizmo

# Core ImGui + ImGuIZMO.quat demo
cargo run -p xtask -- web-demo imguizmo-quat

# Core ImGui + ImPlot + ImPlot3D + ImNodes + ImGuizmo + ImGuIZMO.quat demos
cargo run -p xtask -- web-demo implot,implot3d,imnodes,imguizmo,imguizmo-quat
```

This builds `examples-wasm/dear-imgui-web-demo` with:

- Always: `web-backends` feature (wgpu/webgl + dear-imgui-wgpu/webgl/webgpu)
- Optional: `implot` feature when `web-demo implot` is used; this shows an “ImPlot (Web)” window
- Optional: `implot3d` feature when `web-demo implot3d` is used; this shows an “ImPlot3D (Web)” window
- Optional: `imnodes` feature when `web-demo imnodes` is used; this shows an “ImNodes (Web)” window
- Optional: `imguizmo` feature when `web-demo imguizmo` is used; this shows an “ImGuizmo (Web)” window
- Optional: `imguizmo-quat` feature when `web-demo imguizmo-quat` is used; this shows an “ImGuIZMO.quat (Web)” window

Outputs go to `target/web-demo`.

3) Build the cimgui provider and glue (Emscripten, like imgui-rs):

```bash
# Ensure emsdk is on PATH (Windows: run emsdk_env.bat / emsdk_env.ps1) or set EMSDK env var
cargo run -p xtask -- build-cimgui-provider
```

This creates:

- `target/web-demo/imgui-sys-v0.wasm` (Emscripten-built provider; imports `env.memory`)
- `target/web-demo/imgui-sys-v0.js` (ES module that instantiates the provider)
- `target/web-demo/imgui-sys-v0-wrapper.js` (wrapper that wires shared memory + logs)
- Patches `target/web-demo/index.html` to add an import map that maps the bare import `imgui-sys-v0` to `./imgui-sys-v0-wrapper.js`.

4) Serve locally:

```bash
python -m http.server -d target/web-demo 8080
```

Open http://localhost:8080

If your server serves `.mjs` as `text/plain`, modern browsers will reject it as a module. `xtask` now emits the provider as `.js` for better default MIME (`text/javascript`), and the import map points to `./imgui-sys-v0.js`. If you run a custom server, ensure these MIME types exist:

- `.js` → `text/javascript` or `application/javascript`
- `.wasm` → `application/wasm` (optional if using non-streaming instantiate; `xtask` glue falls back when not set)

## Notes

- Provider flags: the provider build disables OS/file functions and uses `IMGUI_USE_WCHAR32` to match our bindings.
- Web backends: the demo enables both `wgpu/webgl` and `wgpu/webgpu`; the runtime selects an available backend.
- If you see a runtime error like “Failed to resolve import imgui-sys-v0”: run `build-cimgui-provider` and ensure the import map is present in `index.html`.
- If you see link errors about `env.memory` / `memchr` / `qsort` etc. or "function import requires a callable": ensure `wasm-tools` is installed so `xtask web-demo` can patch the main module to import memory, and that `examples-wasm/web/index.html` creates `globalThis.__imgui_shared_memory` (already present).

## Implementation details

- Import‑style linkage
  - `dear-imgui-sys` (Rust) imports all cimgui symbols from module `imgui-sys-v0`.
  - The provider is an Emscripten build of cimgui + Dear ImGui, emitted as an ES module with a thin wrapper.

- Shared memory flow
  - `examples-wasm/web/index.html` creates `globalThis.__imgui_shared_memory` (default 128MiB).
  - `xtask web-demo` uses `wasm-tools` to WAT‑patch `dear-imgui-web-demo_bg.wasm` so it imports `env.memory` and re‑exports it.
  - `xtask web-demo` also patches wasm-bindgen JS (`__wbg_get_imports()`) to inject `imports.env.memory = __imgui_shared_memory`.
  - `xtask build-cimgui-provider` compiles the provider with `-s IMPORTED_MEMORY=1`, and our wrapper passes the same memory via `{ wasmMemory: memory }`.
  - We place provider static data high with `-s GLOBAL_BASE=67108864` to avoid overlap. The shared memory initial size is increased accordingly.

- Canvas/DPI/Surface
  - The canvas backing store is kept in sync with CSS size × `devicePixelRatio`.
  - Each frame we set `io.DisplaySize = logical` and `io.DisplayFramebufferScale = [dpi, dpi]`.
  - The wgpu `Surface` is configured to the physical size and reconfigured when the canvas size changes.

## CI suggestions

- Install target and wasm-bindgen-cli:
  - `rustup target add wasm32-unknown-unknown`
  - `cargo install -f wasm-bindgen-cli --version 0.2.105`
- Build demo:
  - `cargo run -p xtask -- wasm-bindgen imgui-sys-v0`
  - `cargo run -p xtask -- web-demo`
- Optionally build provider (requires emsdk):
  - Ensure PATH includes emsdk (`emcc/em++`) or set `EMSDK` env var to the emsdk root.
  - `cargo run -p xtask -- build-cimgui-provider`

## Known limitations

- Filesystem is disabled in provider builds (`-s FILESYSTEM=0`).
- FreeType is not included in the provider yet.
- Multi-viewport is not enabled for web builds.

## Troubleshooting

- ImGui aborts with “Invalid DisplaySize value”
  - The two modules aren’t sharing the same memory. Ensure `wasm-tools` is installed and check the console for the shared memory logs. Rebuild with `cargo run -p xtask -- web-demo` and hard refresh.

- “Scissor rect is not contained in the render target”
  - The surface must use the physical size. Pull latest and rebuild; we now configure the surface with the physical size and reconfigure on resize.

- Provider link error: “initial memory too small”
  - Increase initial memory (we default to 128MiB) or reduce assets. Our wrapper sets a larger shared memory in `index.html`.
