# Browser Example

This directory contains the `dear-imgui-web-demo` package. Start with the
[local demo instructions](../docs/WASM.md#local-demo) for the required Rust target,
matching `wasm-bindgen-cli`, `wasm-tools`, Emscripten SDK, build commands, and local
HTTP server.

The browser integration has three entry points:

- [`src/lib.rs`](src/lib.rs) owns the Winit/WGPU application and UI. Optional
  extensions are selected through the features in [`Cargo.toml`](Cargo.toml).
- [`web/index.html`](web/index.html) provides the canvas and shared memory.
- [`xtask`](../xtask/src/main.rs) builds the Rust module and native import provider
  into `target/web-demo/`.

Run the documented commands from the workspace root. Serve the generated
`target/web-demo/` directory over HTTP and open it in a browser with WebGL2 or
WebGPU support. The checked-in HTML becomes runnable after both the Rust module
and the `imgui-sys-v1` provider have been generated.

Canvas size comes from CSS in `web/index.html`; Winit supplies the corresponding
backing size and pixel density. UI sizes stay in logical points while
framebuffer scaling handles backing pixels. The initial panel and demo layout
uses the available client area, and existing windows are clamped before drawing
if they extend outside it.

`src/lib.rs` acquires a presentable surface before opening an ImGui frame and
uses a frame token to close the frame on errors. The Rust WGPU renderer handles
managed texture requests, including font-atlas updates; the native import
provider is not a renderer backend. Enable `experimental-fonts` to exercise
custom font configuration (`cte` includes that feature).
