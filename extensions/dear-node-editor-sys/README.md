# dear-node-editor-sys

Low-level FFI bindings for `imgui-node-editor` through the `cimnodes_editor`
submodule.

The public bindings are generated from the local `dne_*` shim rather than the
raw upstream C wrapper. This keeps C++ ID helper objects out of the Rust API and
lets the high-level crate expose `NodeId`, `PinId`, and `LinkId` as plain
pointer-sized newtypes.

## Build Modes

- Normal builds use `src/bindings_pregenerated.rs` and compile the native C++ sources with `cc`.
- Binding regeneration requires `DEAR_IMGUI_RS_REGEN_BINDINGS=1` and `--features bindgen`.
- `NODE_EDITOR_SYS_SKIP_CC=1` skips native compilation and uses pregenerated bindings.
- `NODE_EDITOR_SYS_LIB_DIR`, `NODE_EDITOR_SYS_PREBUILT_URL`, and the `prebuilt` feature follow
  the same prebuilt-library flow as the other workspace `*-sys` crates.
- `wasm32` is intentionally unsupported in the first integration phase.
