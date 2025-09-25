# WebAssembly (WASM) support

This project supports compiling Dear ImGui + backends to WebAssembly using an import-style design. It follows the approach used by imgui-rs: Rust code links to a WASM import module named `imgui-sys-v0` that provides the cimgui (C API) implementation.

## Quick start

- Prerequisites
  - `rustup target add wasm32-unknown-unknown`
  - `cargo install -f wasm-bindgen-cli --version 0.2.104`
  - `cargo install -f wasm-tools`
  - Emscripten SDK (for provider). On Windows run `emsdk_env.bat`/`.ps1` so `emcc/em++` are on PATH, or set `EMSDK` to emsdk root.

- Build and run
  1) Build the main WASM + JS and patch for shared memory
     - `cargo run -p xtask -- web-demo`
  2) Build the cimgui provider (Emscripten)
     - `cargo run -p xtask -- build-cimgui-provider`
  3) Serve and open in browser
     - `python -m http.server -d target/web-demo 8080`
     - Open http://127.0.0.1:8080 and hard‑refresh (Ctrl+F5)

- Expected console logs
  - `[imgui-sys-v0] Shared memory pages= …` and `Module.wasmMemory===memory true`
  - `IO set: logical=… framebuffer_scale=… physical=… surface=…`

## Why Emscripten?

- Import‑style split builds keep Rust compilation fast and clean: the C/C++ side (cimgui + Dear ImGui) is compiled once as a provider.
- Emscripten has mature support for exporting exactly the symbols we need (via `EXPORTED_FUNCTIONS`), emitting ES modules, and most importantly importing a shared memory (`-s IMPORTED_MEMORY=1`).
- Sharing one `WebAssembly.Memory` between the Rust app (wasm-bindgen) and the provider is critical: ImGui IO/state written by Rust must be visible to cimgui.
- Decoupling makes it easy to size and debug the provider independently (initial memory, GLOBAL_BASE, logs) without touching the Rust build.

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
  - `cargo install -f wasm-bindgen-cli --version 0.2.104`
- wasm-tools (used by xtask to patch the main Wasm to import memory so it can share memory with the provider)
  - `cargo install -f wasm-tools`
- Emscripten SDK (for provider build)
  - Install emsdk and either:
    - Run its environment script (adds `emcc/em++` to PATH), or
    - Set `EMSDK` env var to the emsdk root.

On Windows, run `emsdk.bat` for setup and `emsdk_env.bat` to update PATH for the current shell (PowerShell users can also run `emsdk_env.ps1`).

## Commands (local)

1) Generate WASM bindings (optional, defaults to `imgui-sys-v0`):

```
cargo run -p xtask -- wasm-bindgen imgui-sys-v0
```

2) Build the web demo (main WASM + JS; xtask will also patch it to import memory so it can share memory with the provider):

```
cargo run -p xtask -- web-demo
```

Outputs to `target/web-demo`.

3) Build the cimgui provider and glue (Emscripten, like imgui-rs):

```
# Ensure emsdk is on PATH (Windows: run emsdk_env.bat / emsdk_env.ps1) or set EMSDK env var
cargo run -p xtask -- build-cimgui-provider
```

This creates:

- `target/web-demo/imgui-sys-v0.wasm` (Emscripten-built cimgui + Dear ImGui; imports `env.memory`)
- `target/web-demo/imgui-sys-v0.js` (ES module that instantiates the provider with top-level await and re-exports its symbols)
- Patches `target/web-demo/index.html` to add an import map that maps the bare import `imgui-sys-v0` to `./imgui-sys-v0.js`.

4) Serve locally:

```
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
  - `cargo install -f wasm-bindgen-cli --version 0.2.104`
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
