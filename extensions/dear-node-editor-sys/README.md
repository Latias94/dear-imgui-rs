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
- The `stack-layout` feature forwards the patched native core artifact profile to `dear-imgui-sys`.
- `NODE_EDITOR_SYS_BUILD_MODE` accepts `auto` (default), `source`, `prebuilt`, or `bindings-only`. Strict prebuilt mode never falls back to source; bindings-only is for checks and does not supply native symbols.
- `NODE_EDITOR_SYS_SKIP_CC=1` is a legacy check-only request without artifact intent, or strict prebuilt when an explicit selector or feature `prebuilt` is present. It conflicts with source requirements. Do not combine enabled legacy `SKIP_CC` or `FORCE_BUILD` flags with `BUILD_MODE`; false boolean values disable the legacy flag.
- `NODE_EDITOR_SYS_LIB_DIR`, `NODE_EDITOR_SYS_PREBUILT_URL`, and the `prebuilt` feature follow
  the same verified prebuilt-library flow as the other workspace `*-sys` crates.
  Supply a matching library and manifest, or a packaged archive with `prebuilt`;
  choose at most one explicit selector from `NODE_EDITOR_SYS_LIB_DIR`, `NODE_EDITOR_SYS_PREBUILT_URL`, and `NODE_EDITOR_SYS_PACKAGE_DIR`. Missing or incompatible selections stop the build.
  The artifact must match the target, CRT, binding/source identity, and shared-core
  feature profile. In automatic mode, `build-from-source` forces source compilation even when `prebuilt` is also enabled; explicit prebuilt and bindings-only modes reject source-only requirements.
- `wasm32` is intentionally unsupported in the first integration phase.

The `0.22` release train requires Rust 1.95 or newer. Upgrade all workspace crates together and review the [native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy) before carrying environment settings forward from 0.21.0.

## WebAssembly Scope

`dear-node-editor-sys` is native-only in this integration phase. This is not
because `imgui-node-editor` can never be built for WebAssembly; it is because
this workspace's WASM support uses an import-style provider module
(`imgui-sys-v1`) with explicitly generated imports and provider exports.

To support `dear-node-editor` on wasm, the workspace would need a complete
provider integration for `cimnodes_editor` / `imgui-node-editor`: wasm
pregenerated bindings, provider export generation, Emscripten source wiring,
texture/input smoke coverage, and web examples. Until that exists, use
`dear-imnodes`, which is already part of the current wasm-capable provider path.

## License Notes

This Rust crate is dual-licensed under the workspace `MIT OR Apache-2.0` terms.
The vendored native `imgui-node-editor` sources are MIT-licensed; see
`third-party/cimnodes_editor/imgui-node-editor/LICENSE`.
