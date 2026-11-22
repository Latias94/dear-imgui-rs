# WebAssembly support

Dear ImGui uses an import-provider architecture on WebAssembly. The Rust
application targets `wasm32-unknown-unknown`; all cimgui functions are imported
from one Emscripten-built provider named `imgui-sys-v1`.

The provider name is part of the ABI. It is not configurable. Core and
extension binding commands reject provider arguments so a locally generated
artifact cannot silently use a different module name.

`imgui-sys-v1` is the first provider ABI that guarantees the checked numeric
formatting and parsing contract used by the safe Rust API. An
`imgui-sys-v0` provider is incompatible: rebuild it from the same repository
revision instead of renaming its files or remapping it under the v1 module name.

## Feature contract

WASM support is explicit and target-specific. The only supported Rust target is `wasm32-unknown-unknown`, and every dependency path to the core crate must enable the `wasm` feature. The examples below target the `0.22` release train and require Rust 1.95 or newer; upgrade all workspace crates together. Use matching requirements:

```toml
[dependencies]
dear-imgui-rs = { version = "0.22", features = ["wasm"] }
```

For Bevy, enable `wasm` alongside the features needed by the application and take both packages from the same release train. The Bevy backend requires Bevy exactly 0.20.1 and Rust 1.97.1 or newer:

```toml
[dependencies]
bevy = "=0.20.1"
dear-imgui-bevy = { version = "0.22", features = ["render", "wasm"] }
dear-imgui-rs = { version = "0.22", features = ["wasm"] }
```

Six published safe native extensions expose the same `wasm` feature and forward it to
both `dear-imgui-rs` and their matching sys crate:

| Safe crate | Sys crate | WASM import bindings |
| --- | --- | --- |
| `dear-implot` | `dear-implot-sys` | yes |
| `dear-implot3d` | `dear-implot3d-sys` | yes |
| `dear-imnodes` | `dear-imnodes-sys` | yes |
| `dear-imguizmo` | `dear-imguizmo-sys` | yes |
| `dear-imguizmo-quat` | `dear-imguizmo-quat-sys` | yes |
| `dear-imgui-cte` | `dear-imgui-cte-sys` | yes |

Enable the feature on the highest safe crate you depend on; direct sys users
enable it on the extension sys crate, which forwards the same core dependency
path. The published native safe extension `dear-node-editor` intentionally has no
WASM route. Dear ImGui Test Engine is also native-only and source-only.

The workspace web demo's `cte` feature also enables the experimental managed
font-atlas path needed to install CTE's bundled DejaVu font before renderer
initialization.

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
cargo check -p dear-implot --target wasm32-unknown-unknown \
  --no-default-features --features wasm
cargo check -p dear-implot3d --target wasm32-unknown-unknown \
  --no-default-features --features wasm
cargo check -p dear-imnodes --target wasm32-unknown-unknown \
  --no-default-features --features wasm
cargo check -p dear-imguizmo --target wasm32-unknown-unknown \
  --no-default-features --features wasm
cargo check -p dear-imguizmo-quat --target wasm32-unknown-unknown \
  --no-default-features --features wasm
cargo check -p dear-imgui-cte --target wasm32-unknown-unknown \
  --no-default-features --features wasm

# Expected to fail with stable feature-contract diagnostics.
cargo check -p dear-imgui-sys --target wasm32-unknown-unknown \
  --features wasm,prebuilt
cargo check -p dear-imgui-sys --target wasm32-unknown-unknown \
  --features wasm,test-engine
cargo check -p dear-imgui-sys --features test-engine,package-bin
```

The following paths remain native-only:

- `stack-layout` and `dear-node-editor/blueprints`
- `test-engine`, which requires a native source build
- `prebuilt`, whose archives contain native static libraries
- `dear-node-editor`, which exposes no `wasm` forwarding feature
- native Winit, SDL3, WGPU, and Ash multi-viewport routes

These features may coexist with the `wasm` feature when Cargo unifies features
for a native target; the build selects behavior from the actual target, not the
feature name. On `wasm32-unknown-unknown`, `stack-layout`, `test-engine`, and
`prebuilt` are rejected. `test-engine + package-bin` is rejected on every
target because Test Engine artifacts are never part of prebuilt packaging. Use
`dear-imnodes` for the current WASM-capable node editor. Browser integrations
render one main canvas and do not install native platform-window callbacks.

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

CTE's native and WASM profiles are maintained by the repository-wide
`verify-bindings` command; there is no separate `wasm-bindgen-cte` contract.

## Local demo

Prerequisites:

- `rustup target add wasm32-unknown-unknown`
- a `wasm-bindgen-cli` version matching `Cargo.lock` (`0.2.130` for the 0.22.0 lockfile)
- Emscripten SDK for the provider build (CI uses 7.0.1)

Re-read the lockfile after dependency updates before installing `wasm-bindgen-cli`; the lockfile is the version source of truth rather than a separately maintained script constant.

Build the Rust application, then the provider:

```bash
# Core only, or pass a comma-separated extension list.
cargo run -p xtask -- web-demo
cargo run -p xtask -- web-demo implot,imnodes

# This enables the experimental managed font-atlas path required by CTE fonts.
cargo run -p xtask -- web-demo cte

# Requires emcc/em++ on PATH, or EMSDK set to the SDK root.
cargo run -p xtask -- build-cimgui-provider

python3 -m http.server -d target/web-demo 8080
```

Open `http://127.0.0.1:8080`. With `web-demo cte`, click in the upper source
editor and type a character. The next frames show the edited text and a higher
`transaction events` count. C++ calls the registered Rust closure while handling
the edit. The lower diff pane and the autocomplete list use the same renderer.

To repeat the browser regression with Playwright CLI, serve a build with `cte`
as above and run these commands from the workspace root. The check also keeps
a native allocation live while Rust reallocates an aligned buffer to 72 MiB.

```sh
playwright-cli -s=imgui-provider-abi open http://127.0.0.1:8080/
playwright-cli -s=imgui-provider-abi run-code --filename=examples-wasm/tests/provider-abi.js
playwright-cli -s=imgui-provider-abi close
```

The WASM CI job opens `/?webgl2` in headless Chrome with the software GL configuration in `examples-wasm/tests/chrome-software-gl.json`. The test verifies the fallback backend and edits the CTE document through the shared provider. With `@playwright/cli` 0.1.14, use `--json run-code` and check `.result == "true"`. The CLI can return exit code zero for a browser assertion failure.

## Runtime model

The supported browser runtime pairs one Rust application with one instance of
the repository's Emscripten provider on a single thread. Data pointers refer to
their common linear memory. C function pointers in either direction are indices
in their common `funcref` table. wasm-bindgen keeps its JavaScript `externref`
table private to Rust.

| Resource | Layout used by the provider and `xtask web-demo` |
| --- | --- |
| Linear memory | One fixed 256 MiB `WebAssembly.Memory`. Rust's stack, statics and linker-defined `System` arena fit below 64 MiB. The provider places its statics at 64 MiB, followed by its stack and `emmalloc` heap. |
| Ordinary allocations | The application selects `WasmProviderAllocator`. Rust `Box`, `Vec`, wasm-bindgen and provider C/C++ allocations then use the same `emmalloc` heap. The wrapper exports `dear_imgui_wasm_memalign` and `dear_imgui_wasm_free` for this purpose. |
| Function pointers | Slots `0..4096` (end exclusive) are reserved for the provider, including the null slot. Rust's first table element is at 4096. Before Rust starts, the wrapper checks the native table fits, then grows it to 16,384 entries. This leaves 12,288 slots for the application. |

The fixed memory size matters even with a global allocator. Rust's standard
library can use `std::alloc::System` directly for thread metadata. Its wasm
allocator first uses the linker's `__heap_base..__heap_end` arena, then tries
`memory.grow` when that arena is full. A fixed memory makes that request fail
and keeps `System` out of the provider heap. A larger Rust data segment or stack
fails at link time. A Rust table import requiring more slots fails at instantiation.

### Using the provider in another application

1. Enable the target features above. Select the allocator in the final Rust
   application so it also covers ordinary allocations in dependencies.

   ```rust
   #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
   #[global_allocator]
   static ALLOCATOR: dear_imgui_rs::sys::WasmProviderAllocator =
       dear_imgui_rs::sys::WasmProviderAllocator;
   ```

2. Apply the demo's linker flags to the final `cdylib`, then run the matching
   wasm-bindgen CLI. For a browser application named `my_app`, run the first two
   commands from the application crate.

   ```sh
   cargo rustc --lib --target wasm32-unknown-unknown --release -- \
     -C link-arg=--import-memory -C link-arg=--export-memory \
     -C link-arg=--initial-memory=67108864 -C link-arg=--max-memory=268435456 \
     -C link-arg=--import-table -C link-arg=--growable-table -C link-arg=--table-base=4096
   wasm-bindgen --target web --out-dir dist target/wasm32-unknown-unknown/release/my_app.wasm
   ```

   From the matching dear-imgui-rs checkout, build the provider into the same
   directory. Ship its `.js`, `.wasm` and `-wrapper.js` alongside the wasm-bindgen output.

   ```sh
   cargo run -p xtask -- build-cimgui-provider --out-dir /path/to/my_app/dist
   ```

3. Resolve both module names to the **same wrapper URL**. The wrapper creates
   the memory, instantiates C++ and exports its live table under the linker's
   `env.__indirect_function_table` name. Pass its memory to wasm-bindgen.

   ```html
   <script type="importmap">
     { "imports": {
       "env": "./imgui-sys-v1-wrapper.js",
       "imgui-sys-v1": "./imgui-sys-v1-wrapper.js"
     } }
   </script>
   <script type="module">
     import init from './my_app.js';
     import { memory } from 'imgui-sys-v1';
     await init({ memory });
   </script>
   ```

**Migration from the memory-only wrapper.** The raw cimgui import name remains
`imgui-sys-v1`. Earlier v1 bundles used private function tables and independent
growable heaps. Rebuild and deploy the Rust module, provider and page together.
Remove the old `globalThis.__imgui_shared_memory` bootstrap. The new wrapper
rejects that bootstrap, and the new Rust module requires allocator and table
exports that older wrappers lack. Additional Rust modules, workers sharing this
instance and custom provider layouts are outside this runtime contract.

## Troubleshooting

- `wasm32-unknown-unknown requires the explicit wasm feature`: enable `wasm` at
  the highest workspace crate in the dependency path; Cargo feature forwarding
  must reach `dear-imgui-sys/wasm`.
- `unsupported Dear ImGui WASM target`: switch to `wasm32-unknown-unknown`.
  WASI and Emscripten targets require different imports/runtime contracts and
  are intentionally not aliases for this provider ABI.
- An unresolved `env` or `imgui-sys-v1` import means the page needs both import
  map entries above. For memory or callable import errors, rebuild both modules
  from the same checkout and pass the wrapper's memory to wasm-bindgen.
- Browser module MIME errors: serve `.js` as JavaScript and `.wasm` as
  `application/wasm`.
- Stale binding errors: run `xtask verify-bindings`; only use `--update` after
  confirming the source revisions and binding specification changed together.

The provider currently disables filesystem functions and does not include
FreeType. Browser multi-viewport and native blueprint stack layout are not
supported.
