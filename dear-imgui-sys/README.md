# dear-imgui-sys

Low-level Rust bindings for Dear ImGui via cimgui (C API) + bindgen.

## Overview

This crate provides unsafe Rust bindings to Dear ImGui v1.92.6 (docking branch) using the [cimgui](https://github.com/cimgui/cimgui) C API. By using cimgui's C interface instead of directly binding to the C++ API, we avoid C++ ABI compatibility issues for the core `ig*` API while maintaining full access to Dear ImGui's functionality.

## Key Features

- **cimgui C API**: Clean C interface eliminates C++ ABI issues across platforms and compilers
- **Docking Support**: Full support for docking and multi-viewport features (multi-viewport WIP)
- **Modern Dear ImGui**: Based on Dear ImGui v1.92.6 docking branch
- **Cross-platform**: Consistent builds on Windows (MSVC/MinGW), Linux, macOS, and WebAssembly
- **Prebuilt Binaries**: Optional prebuilt static libraries for faster builds
- **Offline-friendly**: Pregenerated bindings for docs.rs and offline environments
- **Optional backend shim ABI**: Shared low-level backend shim modules for downstream backend crates and engine integrations

## Build Strategies

This crate supports multiple build strategies to fit different development workflows:

### 1. Prebuilt Static Libraries (Recommended)

The fastest way to get started. Use prebuilt static libraries instead of compiling from source:

```bash
# Option A: Point to a local directory containing the static library
export IMGUI_SYS_LIB_DIR=/path/to/lib/dir  # Contains dear_imgui.lib (Windows) or libdear_imgui.a (Unix)

# Option B: Use a local file path (library or .tar.gz archive)
export IMGUI_SYS_PREBUILT_URL=/path/to/dear_imgui.lib

# Option C: Enable HTTP(S) downloads / auto-download from GitHub releases
cargo build -p dear-imgui-sys --features prebuilt
# optional (requires the feature above)
export IMGUI_SYS_USE_PREBUILT=1
```

### 2. Build from Source

Compile Dear ImGui and cimgui from the vendored source code:

```bash
# Windows (automatically uses CMake if available)
cargo build -p dear-imgui-sys

# Force CMake on other platforms
export IMGUI_SYS_USE_CMAKE=1
cargo build -p dear-imgui-sys

# Use cc crate (default on non-Windows)
cargo build -p dear-imgui-sys
```

**Requirements by platform:**

- **Windows**: Visual Studio Build Tools or Visual Studio with C++ support
- **Linux**: `build-essential`, `pkg-config`, `llvm-dev` (for bindgen)
  ```bash
  sudo apt-get install build-essential pkg-config llvm-dev clang
  ```
- **macOS**: Xcode Command Line Tools
  ```bash
  xcode-select --install
  ```

### 3. Development Mode

Skip C/C++ compilation for faster Rust-only iteration:

```bash
export IMGUI_SYS_SKIP_CC=1
cargo build -p dear-imgui-sys
```

This uses pregenerated bindings and skips native compilation, useful when working on higher-level Rust code.

## Offline Builds & docs.rs

This crate supports offline builds and docs.rs compilation through pregenerated bindings:

### docs.rs Support

When building on docs.rs (`DOCS_RS=1`), the build script:

- Uses pregenerated bindings from `src/bindings_pregenerated.rs` if available
- Falls back to generating bindings from vendored cimgui headers (no network required)
- Skips native C/C++ compilation entirely

### Updating Pregenerated Bindings

To refresh the pregenerated bindings file:

```bash
# Generate new bindings without C++ compilation
IMGUI_SYS_SKIP_CC=1 cargo build -p dear-imgui-sys

# Copy generated bindings to source tree
cp target/debug/build/dear-imgui-sys-*/out/bindings.rs dear-imgui-sys/src/bindings_pregenerated.rs
```

Or use the provided update script:

```bash
python3 tools/update_submodule_and_bindings.py --branch docking_inter
```

## WebAssembly Support

WebAssembly support for Dear ImGui in this workspace follows the same **import-style** design used by the high-level `dear-imgui-rs` crate:

- Rust code links against a WASM import module named `imgui-sys-v0` that provides the cimgui (C API) implementation.
- The main application (Rust + winit + wgpu) targets `wasm32-unknown-unknown` and uses `wasm-bindgen`.
- A separate provider module (`imgui-sys-v0`) is built once (currently via Emscripten) and contains Dear ImGui + cimgui and, optionally, selected extensions.

The `dear-imgui-sys` crate participates in this flow via its `wasm` feature, but end users typically interact with it indirectly through:

- `dear-imgui-rs` with the `wasm` feature enabled.
- The `xtask` commands (`wasm-bindgen`, `web-demo`, `build-cimgui-provider`) that wire the main module and provider together.

For a complete, up-to-date guide (including required tools, commands, and troubleshooting), see:

- `docs/WASM.md` in this repository.
- The `examples-wasm` crate (`examples-wasm/dear-imgui-web-demo`), which demonstrates the web demo setup.

## Basic Usage

This is a low-level sys crate providing unsafe FFI bindings. Most users should use the higher-level [`dear-imgui-rs`](https://crates.io/crates/dear-imgui-rs) crate instead, which provides safe Rust wrappers.

```toml
[dependencies]
dear-imgui-sys = "0.10"

# Enable features as needed
dear-imgui-sys = { version = "0.10", features = ["freetype", "wasm"] }
```

### Direct FFI Usage (Advanced)

```rust
use dear_imgui_sys::*;

unsafe {
    let ctx = igCreateContext(std::ptr::null_mut());
    igSetCurrentContext(ctx);

    // Configure ImGui...
    let io = igGetIO();
    (*io).DisplaySize = ImVec2 { x: 800.0, y: 600.0 };

    // Main loop
    igNewFrame();
    igText(b"Hello from Dear ImGui!\0".as_ptr() as *const std::os::raw::c_char);
    igRender();

    // Clean up
    igDestroyContext(ctx);
}
```

## Backend Shim Features (Advanced)

For backend crates, engine integrations, and low-level users, `dear-imgui-sys`
can expose optional backend shim modules behind `backend-shim-*` features:

```toml
[dependencies]
dear-imgui-sys = { version = "0.10", features = ["backend-shim-opengl3"] }
```

These features expose modules such as:

- `dear_imgui_sys::backend_shim::win32`
- `dear_imgui_sys::backend_shim::dx11`
- `dear_imgui_sys::backend_shim::android`
- `dear_imgui_sys::backend_shim::opengl3`

Important scope note:

- `backend-shim-*` exposes the repository-owned C shim ABI, not the original
  upstream C++ backend symbol names
- self-contained official backends may be compiled by `dear-imgui-sys` behind
  these features
- this does not mean `dear-imgui-rs` already provides a safe wrapper for those
  backends

### Why Shim ABI Matters

The core `ig*` API comes from cimgui, so it is a normal C ABI boundary.

The official Dear ImGui backend entry points (`imgui_impl_win32.cpp`,
`imgui_impl_dx11.cpp`, `imgui_impl_opengl3.cpp`, etc.) are different:

- they are implemented as C++ backend code
- their upstream symbol names are not the portable Rust-facing ABI
- Rust should call a deliberate C shim boundary instead

`dear-imgui-sys` therefore exposes a backend shim ABI for self-contained
official backends instead of pretending the upstream `imgui_impl_*` names are a
stable C interface.

### Typical Downstream Pattern

There are two supported low-level patterns.

1. For self-contained official backends such as `opengl3`, `android`, `win32`,
   and `dx11`, enable the matching `backend-shim-*` feature and call the shim
   module directly from Rust.
2. For framework-specific integrations such as SDL3, keep the framework build
   logic in the backend crate, optionally reuse upstream backend sources exported
   by `dear-imgui-sys`, and define crate-local wrappers where needed.

`dear-imgui-sys` exports both upstream backend sources and repository-owned shim
sources to dependents as cargo metadata:

```rust
// build.rs
use std::env;
use std::path::PathBuf;

let imgui_backends = PathBuf::from(
    env::var("DEP_DEAR_IMGUI_IMGUI_BACKENDS_PATH")
        .expect("dear-imgui-sys did not export IMGUI_BACKENDS_PATH"),
);
let backend_shims = PathBuf::from(
    env::var("DEP_DEAR_IMGUI_IMGUI_BACKEND_SHIMS_PATH")
        .expect("dear-imgui-sys did not export IMGUI_BACKEND_SHIMS_PATH"),
);
let imgui_root = imgui_backends
    .parent()
    .expect("IMGUI_BACKENDS_PATH should point to imgui/backends");
```

This remains useful for backend crates such as `dear-imgui-sdl3`, which still
own SDL3-specific build logic even though `dear-imgui-sys` now provides shared
shims for self-contained backends such as OpenGL3.

### Cargo Metadata for Backend Authors

Backend and engine integration crates can consume these cargo metadata exports
from `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`: upstream Dear ImGui include root
- `DEP_DEAR_IMGUI_IMGUI_BACKENDS_PATH`: upstream `imgui/backends` directory
- `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`: cimgui include root
- `DEP_DEAR_IMGUI_IMGUI_BACKEND_SHIMS_PATH`: repository-owned `backend-shims`
  directory

Preferred use:

- use the Rust `backend_shim::*` modules directly when `dear-imgui-sys` already
  provides the low-level ABI you need
- use `IMGUI_BACKENDS_PATH` when your crate still owns framework-specific
  compilation such as SDL3/GLFW glue
- use `IMGUI_BACKEND_SHIMS_PATH` only when you intentionally need access to the
  repository-owned shim sources from a downstream build script

### Android Integration Recipes

There are two first-class Android directions.

1. Custom Android backend without a dedicated first-party crate yet:

   ```toml
   [dependencies]
   dear-imgui-rs = "0.10"
   dear-imgui-sys = { version = "0.10", features = ["backend-shim-android", "backend-shim-opengl3"] }
   ```

   Use `dear-imgui-rs` for the safe core (`Context`, IO, frame lifecycle,
   textures, render snapshots) and call
   `dear_imgui_sys::backend_shim::{android, opengl3}` for the low-level official
   backend pieces.

   A concrete repository template for this route lives at
   `examples-android/dear-imgui-android-smoke/`. It is intentionally kept
   outside the main workspace build so we can document and validate the Android
   path without changing the normal desktop/web CI matrix.

   The repository currently uses this template as the concrete proof that the
   low-level Android route is viable before any dedicated first-party Android
   convenience crate exists: it is cross-compiled in isolation, carries the
   minimal `cargo-apk2` metadata needed to build a `NativeActivity` APK without
   introducing a new published crate, and now also owns a minimal EGL / GLES3
   render loop that renders actual Dear ImGui UI on-device.

   Important nuance: if your Android app uses `android-activity`, its input API
   wraps raw `AInputEvent*` values. In that setup you will typically translate
   input into `dear-imgui-rs::Io` manually, or choose a lower-level glue path
   that gives direct access to raw Android input events before delegating to
   `backend_shim::android`.

2. SDL3-based Android integration:

   Depend on `dear-imgui-sdl3` for the SDL3 backend wrapper, but keep SDL3
   acquisition, NDK setup, and Android packaging owned by the application. The
   application may either provide SDL3 headers via `SDL3_INCLUDE_DIR` or add a
   direct `sdl3` dependency with `features = ["build-from-source"]` so Cargo
   feature unification makes `sdl3-sys` export `DEP_SDL3_OUT_DIR`. When using
   the build-from-source route, the application still needs to provide the
   Android ABI/toolchain contract expected by SDL3's CMake build
   (`ANDROID_ABI` / `CMAKE_ANDROID_ARCH_ABI`, toolchain file, generator, etc.),
   typically via `cargo-ndk`, Gradle+CMake, or an equivalent app-owned build
   system.

This is the intended ownership split: `dear-imgui-sys` owns reusable low-level
building blocks; framework- and application-specific Android integration remains
outside the core crates.

## Technical Details

### cimgui Integration

This crate uses [cimgui](https://github.com/cimgui/cimgui) as the C API layer:

- **No C++ ABI Issues**: cimgui provides a pure C interface, eliminating cross-platform ABI compatibility problems
- **Complete API Coverage**: All Dear ImGui functions are available through the C API
- **Consistent Naming**: Functions follow the `ig*` naming convention (e.g., `igText`, `igButton`)
- **Automatic Generation**: Bindings are generated via bindgen from cimgui headers

### Version Information

- **Dear ImGui Version**: v1.92.6 (docking branch)
- **cimgui Version**: Latest compatible with Dear ImGui v1.92.6
- **Supported Features**: Docking, multi-viewport (WIP), FreeType font rendering

### Environment Variables

Control build behavior with these environment variables:

| Variable | Description |
|----------|-------------|
| `IMGUI_SYS_LIB_DIR` | Path to directory containing prebuilt static library |
| `IMGUI_SYS_PREBUILT_URL` | Local file path or direct URL to a prebuilt library/archive (HTTP(S) and `.tar.gz` extraction require feature `prebuilt`) |
| `IMGUI_SYS_USE_PREBUILT` | Enable automatic download from GitHub releases (`1`, requires feature `prebuilt`) |
| `IMGUI_SYS_USE_CMAKE` | Force CMake build instead of cc crate (`1`) |
| `IMGUI_SYS_SKIP_CC` | Skip C/C++ compilation, use pregenerated bindings only (`1`) |
| `IMGUI_SYS_FORCE_BUILD` | Force build from source, ignore prebuilt options (`1`) |

## Related Crates

This crate is part of the `dear-imgui-rs` ecosystem:

- **[dear-imgui-rs](https://crates.io/crates/dear-imgui-rs)** - Safe, high-level Rust API
- **[dear-imgui-wgpu](https://crates.io/crates/dear-imgui-wgpu)** - WGPU renderer backend
- **[dear-imgui-glow](https://crates.io/crates/dear-imgui-glow)** - OpenGL renderer backend
- **[dear-imgui-winit](https://crates.io/crates/dear-imgui-winit)** - Winit platform backend

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
