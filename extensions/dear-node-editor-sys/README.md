# dear-node-editor-sys

Low-level FFI bindings for `imgui-node-editor` through the `cimnodes_editor`
submodule.

The public bindings are generated from the local `dne_*` shim rather than the
raw upstream C wrapper. This keeps C++ ID helper objects out of the Rust API and
lets the high-level crate expose `NodeId`, `PinId`, and `LinkId` as plain
pointer-sized newtypes.

## Upstream Sources

This crate vendors:

- [`cimnodes_editor`](https://github.com/cimgui/cimnodes_editor), the C wrapper
  used for binding generation.
- [`imgui-node-editor`](https://github.com/thedmd/imgui-node-editor), the native
  C++ node editor implementation.

The Rust examples, including the blueprints-style showcase, intentionally follow
the upstream `imgui-node-editor` examples where practical. The stack layout
helpers required by those examples (`BeginHorizontal`, `BeginVertical`, and
`Spring`) are provided by `dear-imgui-sys` as a repository-owned compatibility
shim; they are not official Dear ImGui APIs.

## Build Modes

- Normal builds use `src/bindings_pregenerated.rs` and compile the native C++ sources with `cc`.
- Binding regeneration requires `DEAR_IMGUI_RS_REGEN_BINDINGS=1` and `--features bindgen`.
- `NODE_EDITOR_SYS_SKIP_CC=1` skips native compilation and uses pregenerated bindings.
- `NODE_EDITOR_SYS_LIB_DIR`, `NODE_EDITOR_SYS_PREBUILT_URL`, and the `prebuilt` feature follow
  the same prebuilt-library flow as the other workspace `*-sys` crates.
- `wasm32` is intentionally unsupported in the first integration phase.

## WebAssembly Scope

`dear-node-editor-sys` is native-only in this integration phase. This is not
because `imgui-node-editor` can never be built for WebAssembly; it is because
this workspace's WASM support uses an import-style provider module
(`imgui-sys-v0`) with explicitly generated imports and provider exports.

To support `dear-node-editor` on wasm, the workspace would need a complete
provider integration for `cimnodes_editor` / `imgui-node-editor`: wasm
pregenerated bindings, provider export generation, Emscripten source wiring,
texture/input smoke coverage, and web examples. Until that exists, use
`dear-imnodes`, which is already part of the current wasm-capable provider path.

## License Notes

This Rust crate is dual-licensed under the workspace `MIT OR Apache-2.0` terms.
The vendored native `imgui-node-editor` sources are MIT-licensed; see
`third-party/cimnodes_editor/imgui-node-editor/LICENSE`.
