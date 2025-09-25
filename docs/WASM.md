# WebAssembly (WASM) support

This project supports compiling Dear ImGui + backends to WebAssembly using an import-style design. It follows the approach used by imgui-rs: Rust code links to a WASM import module named `imgui-sys-v0` that provides the cimgui (C API) implementation.

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

2) Build the web demo (main WASM + JS):

```
cargo run -p xtask -- web-demo
```

Outputs to `target/web-demo`.

3) Build the cimgui provider and glue:

```
# Ensure emsdk is on PATH (Windows: run emsdk_env.bat / emsdk_env.ps1) or set EMSDK env var
cargo run -p xtask -- build-cimgui-provider
```

This creates:

- `target/web-demo/imgui-sys-v0.wasm` (Emscripten-built cimgui + Dear ImGui)
- `target/web-demo/imgui-sys-v0.js` (ES module re-exporting provider exports)
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
