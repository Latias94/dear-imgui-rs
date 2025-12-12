# dear-imgui-sys

Low-level Rust bindings for Dear ImGui via cimgui (C API) + bindgen.

## Overview

This crate provides unsafe Rust bindings to Dear ImGui v1.92.5 (docking branch) using the [cimgui](https://github.com/cimgui/cimgui) C API. By using cimgui's C interface instead of directly binding to the C++ API, we completely avoid C++ ABI compatibility issues while maintaining full access to Dear ImGui's functionality.

## Key Features

- **cimgui C API**: Clean C interface eliminates C++ ABI issues across platforms and compilers
- **Docking Support**: Full support for docking and multi-viewport features (multi-viewport WIP)
- **Modern Dear ImGui**: Based on Dear ImGui v1.92.5 docking branch
- **Cross-platform**: Consistent builds on Windows (MSVC/MinGW), Linux, macOS, and WebAssembly
- **Prebuilt Binaries**: Optional prebuilt static libraries for faster builds
- **Offline-friendly**: Pregenerated bindings for docs.rs and offline environments

## Build Strategies

This crate supports multiple build strategies to fit different development workflows:

### 1. Prebuilt Static Libraries (Recommended)

The fastest way to get started. Use prebuilt static libraries instead of compiling from source:

```bash
# Option A: Point to a local directory containing the static library
export IMGUI_SYS_LIB_DIR=/path/to/lib/dir  # Contains dear_imgui.lib (Windows) or libdear_imgui.a (Unix)

# Option B: Download from a direct URL
export IMGUI_SYS_PREBUILT_URL=https://example.com/dear_imgui.lib

# Option C: Enable automatic download from GitHub releases
cargo build --features prebuilt
# or
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
python tools/update_submodule_and_bindings.py --branch docking_inter
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
dear-imgui-sys = "0.6"

# Enable features as needed
dear-imgui-sys = { version = "0.6", features = ["freetype", "wasm"] }
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
    igText(b"Hello from Dear ImGui!\0".as_ptr() as *const i8);
    igRender();

    // Clean up
    igDestroyContext(ctx);
}
```

## Technical Details

### cimgui Integration

This crate uses [cimgui](https://github.com/cimgui/cimgui) as the C API layer:

- **No C++ ABI Issues**: cimgui provides a pure C interface, eliminating cross-platform ABI compatibility problems
- **Complete API Coverage**: All Dear ImGui functions are available through the C API
- **Consistent Naming**: Functions follow the `ig*` naming convention (e.g., `igText`, `igButton`)
- **Automatic Generation**: Bindings are generated via bindgen from cimgui headers

### Version Information

- **Dear ImGui Version**: v1.92.5 (docking branch)
- **cimgui Version**: Latest compatible with Dear ImGui v1.92.5
- **Supported Features**: Docking, multi-viewport (WIP), FreeType font rendering

### Environment Variables

Control build behavior with these environment variables:

| Variable | Description |
|----------|-------------|
| `IMGUI_SYS_LIB_DIR` | Path to directory containing prebuilt static library |
| `IMGUI_SYS_PREBUILT_URL` | Direct URL to download prebuilt library |
| `IMGUI_SYS_USE_PREBUILT` | Enable automatic download from GitHub releases (`1`) |
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
