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
