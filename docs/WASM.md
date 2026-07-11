# WebAssembly support

Dear ImGui uses an import-provider architecture on WebAssembly. The Rust
application targets `wasm32-unknown-unknown`; all cimgui functions are imported
from one Emscripten-built provider named `imgui-sys-v0`.

The provider name is part of the ABI. It is not configurable. Core and
extension binding commands reject provider arguments so a locally generated
artifact cannot silently use a different module name.

## Feature contract

WASM support is explicit and target-specific. The only supported Rust target is
`wasm32-unknown-unknown`, and every dependency path to the core crate must
enable the `wasm` feature:

```toml
[dependencies]
dear-imgui-rs = { version = "0.16.0", features = ["wasm"] }
```

For Bevy, enable `wasm` alongside the features needed by the application:

```toml
[dependencies]
dear-imgui-bevy = { version = "0.16.0", features = ["render", "wasm"] }
```

A `wasm32-unknown-unknown` build without this feature fails at compile time
instead of selecting a native binding artifact. Other wasm32 families, including
`wasm32-wasip1`, `wasm32-wasip2`, and `wasm32-unknown-emscripten`, are rejected
even when the feature is enabled; they cannot consume the unknown-unknown import
ABI. `dear_imgui_rs::HAS_WASM` is true only for the exact supported target plus
feature. Enabling the feature on a native target remains valid and leaves
`HAS_WASM` false.

Use these checks when changing feature forwarding:

```bash
# Expected to fail: the target alone is insufficient.
cargo check -p dear-imgui-sys --target wasm32-unknown-unknown

# Expected to pass.
cargo check -p dear-imgui-rs --target wasm32-unknown-unknown --features wasm
cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown \
  --no-default-features --features wasm
cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown \
  --features render,wasm
```

The following paths remain native-only:

- `stack-layout` and `dear-node-editor/blueprints`
- `dear-node-editor` in its current integration
- native Winit, SDL3, WGPU, and Ash multi-viewport routes

`stack-layout` and `wasm` are rejected together. Use `dear-imnodes` for the
current WASM-capable node editor. Browser integrations render one main canvas
and do not install native platform-window callbacks.

## Binding artifacts

Core generation is owned by the shared binding specification. One command
regenerates and cross-checks the two native ABI profiles and the WASM import
profile:

```bash
# Reproduce and compare all checked-in artifacts.
cargo run -p xtask -- verify-bindings

# Maintainer-only update after an intentional source/spec change.
cargo run -p xtask -- verify-bindings --update --allow-dirty
```

The compatibility matrix includes Windows MSVC/GNU and supported Linux,
Android, macOS, and iOS clang targets. Canonical generation rejects every
`BINDGEN_EXTRA_CLANG_ARGS*` override. This keeps checked-in output tied to the
reviewed generator contract instead of a maintainer's shell environment.

The update workflow also synchronizes the exact cimgui and nested Dear ImGui
revisions in `dear-imgui-sys/Cargo.toml` and refuses dirty source submodules:

```bash
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys --submodules auto --wasm
```

Optional extension import bindings use the same fixed provider:

```bash
cargo run -p xtask -- wasm-bindgen-implot
cargo run -p xtask -- wasm-bindgen-implot3d
cargo run -p xtask -- wasm-bindgen-imnodes
cargo run -p xtask -- wasm-bindgen-imguizmo
cargo run -p xtask -- wasm-bindgen-imguizmo-quat
```

## Local demo

Prerequisites:

- `rustup target add wasm32-unknown-unknown`
- a `wasm-bindgen-cli` version matching `Cargo.lock`
- `wasm-tools`
- Emscripten SDK for the provider build

Re-read the lockfile after dependency updates before installing
`wasm-bindgen-cli`; do not maintain a second version constant in scripts or
documentation.

Build the Rust application, then the provider:

```bash
# Core only, or pass a comma-separated extension list.
cargo run -p xtask -- web-demo
cargo run -p xtask -- web-demo implot,imnodes

# Requires emcc/em++ on PATH, or EMSDK set to the SDK root.
cargo run -p xtask -- build-cimgui-provider

python3 -m http.server -d target/web-demo 8080
```

Open `http://127.0.0.1:8080`. The provider command emits
`imgui-sys-v0.wasm`, its JavaScript loader, a shared-memory wrapper, and the
import map used by the web demo.

## Runtime model

The Rust and C++ modules share one `WebAssembly.Memory`:

1. The page creates `globalThis.__imgui_shared_memory`.
2. `xtask web-demo` patches the wasm-bindgen module to import and re-export
   `env.memory`.
3. `xtask build-cimgui-provider` builds cimgui with imported memory and passes
   the same object to the Emscripten module.
4. The browser resolves all generated cimgui imports through `imgui-sys-v0`.

This is why the provider cannot be replaced by arbitrary Rust-side imports or
by compiling native C++ through Cargo on a WASM target. A custom single-module
design would require a separate reviewed ABI, memory, startup, and symbol-export
contract; it is not a supported fallback.

## Troubleshooting

- `wasm32-unknown-unknown requires the explicit wasm feature`: enable `wasm` at
  the highest workspace crate in the dependency path; Cargo feature forwarding
  must reach `dear-imgui-sys/wasm`.
- `unsupported Dear ImGui WASM target`: switch to `wasm32-unknown-unknown`.
  WASI and Emscripten targets require different imports/runtime contracts and
  are intentionally not aliases for this provider ABI.
- `Failed to resolve import imgui-sys-v0`: build the provider and verify the
  generated import map points to `imgui-sys-v0-wrapper.js`.
- `env.memory` or callable import errors: install `wasm-tools`, rebuild the web
  demo, then rebuild the provider so both artifacts use the same memory wiring.
- Browser module MIME errors: serve `.js` as JavaScript and `.wasm` as
  `application/wasm`.
- Stale binding errors: run `xtask verify-bindings`; only use `--update` after
  confirming the source revisions and binding specification changed together.

The provider currently disables filesystem functions and does not include
FreeType. Browser multi-viewport and native blueprint stack layout are not
supported.
