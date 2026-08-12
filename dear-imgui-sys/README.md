# dear-imgui-sys

Low-level Rust bindings for Dear ImGui via cimgui (C API) and checked-in pregenerated bindings.

## Overview

This crate provides unsafe Rust bindings to Dear ImGui v1.92.9b (docking branch) using the [cimgui](https://github.com/cimgui/cimgui) C API. The core `ig*` API crosses a C ABI boundary. C++ backend integration and callback-bearing platform APIs use explicit repository-owned shims because their compiler ABI still matters, especially on MSVC.

## Key Features

- **cimgui C API**: A deliberate C boundary for the core `ig*` API
- **Docking Support**: Full docking support; PlatformIO primitives for backend-specific native multi-viewport routes
- **Modern Dear ImGui**: Based on Dear ImGui v1.92.9b docking branch
- **Cross-platform**: Consistent builds on Windows (MSVC, MinGW-GCC, and GNU/LLVM), Linux, macOS, and WebAssembly
- **Prebuilt Binaries**: Optional prebuilt static libraries for faster builds
- **Offline-friendly**: Pregenerated bindings for normal builds, docs.rs, and offline environments
- **Optional backend shim ABI**: Shared low-level self-contained backend shim modules for downstream backend crates and engine integrations
- **Optional stack layout artifact**: The native-only `stack-layout` feature enables a patched
  core and repository-owned C ABI for blueprint-style layout helpers

## Build Strategies

This crate supports multiple build strategies to fit different development workflows:

### 1. Prebuilt Static Libraries (Recommended)

The fastest way to get started is to use prebuilt static libraries instead of compiling from source.
`0.16.0-alpha.3` archives are published for the supported release targets and profiles.

```bash
# Option A: Point to the library directory inside an extracted core artifact.
# The strict manifest.txt may be in this directory or its parent artifact root.
export IMGUI_SYS_LIB_DIR=/path/to/extracted/dear-imgui-artifact/lib

# Option B: Use a package-tool-generated local archive or HTTP(S) URL.
export IMGUI_SYS_PREBUILT_URL=/path/to/dear-imgui-prebuilt-0.16.0-alpha.3-<target>-static.tar.gz
cargo build -p dear-imgui-sys --features prebuilt

# Option C: Enable HTTP(S) downloads / auto-download from GitHub releases
export IMGUI_SYS_USE_PREBUILT=1
cargo build -p dear-imgui-sys --features prebuilt
```

### 2. Build from Source

Compile Dear ImGui and cimgui from the vendored source code:

```bash
cargo build -p dear-imgui-sys
```

When building from a repository checkout, the vendored cimgui source comes from
Git submodules. For a fresh checkout, clone with `--recursive`; for an existing
checkout, run this inside the repository before building:

```bash
git submodule update --init --recursive
```

If Cargo reports a missing header such as
`dear-imgui-sys/third-party/cimgui/imgui/imgui.h`, the cimgui submodule is not
initialized.

Source builds use the `cc` crate on every platform. There is no alternate CMake
core build route.

Normal source builds use the checked-in pregenerated Rust bindings and do not require libclang.
Bindgen is only needed when regenerating bindings.

**Requirements by platform:**

- **Windows MSVC**: Visual Studio Build Tools or Visual Studio with C++ support
  - Optional `freetype` source builds can use vcpkg:
    `vcpkg install freetype:x64-windows-static-md`
- **Windows GNU/LLVM (`*-pc-windows-gnullvm`)**: llvm-mingw with its target-prefixed Clang drivers and `llvm-ar`; Visual Studio is not required
- **Linux**: `build-essential`, `pkg-config`
  ```bash
  sudo apt-get install build-essential pkg-config
  ```
- **macOS**: Xcode Command Line Tools
  ```bash
  xcode-select --install
  ```

For an x64 gnullvm source build, point Cargo and `cc` at the same llvm-mingw installation:

```powershell
$llvmMingw = 'C:\llvm-mingw'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = "$llvmMingw\bin\x86_64-w64-mingw32-clang.exe"
$env:CC_x86_64_pc_windows_gnullvm = "$llvmMingw\bin\x86_64-w64-mingw32-clang.exe"
$env:CXX_x86_64_pc_windows_gnullvm = "$llvmMingw\bin\x86_64-w64-mingw32-clang++.exe"
$env:AR_x86_64_pc_windows_gnullvm = "$llvmMingw\bin\llvm-ar.exe"
cargo build -p dear-imgui-sys --target x86_64-pc-windows-gnullvm
```

For `aarch64-pc-windows-gnullvm`, use the equivalent `aarch64_pc_windows_gnullvm` environment-variable suffix and the `aarch64-w64-mingw32-clang[++]` drivers. Both gnullvm targets link libc++ statically. Default Rust CRT mode still requires llvm-mingw's `libunwind.dll` at runtime; `-C target-feature=+crt-static` removes that dynamic unwind dependency on x64. CI executes and ABI-tests x64 in both modes, but only cross-links and inspects the Arm64 target.

Official gnullvm prebuilts are not published. This source-build contract covers the default native core and maintained C++ extension paths; optional FreeType and the SDL3 backend's external native dependency remain separate support work.

When the `freetype` feature is enabled, `dear-imgui-sys` must find real
FreeType development files. The build script tries `pkg-config freetype2` first
and vcpkg's `freetype` port next. On Windows/MSVC, install the vcpkg triplet
that matches vcpkg-rs' selection, for example `x64-windows-static-md` for the
default Rust CRT mode, or set `VCPKGRS_TRIPLET` explicitly. If you use a dynamic
vcpkg triplet such as `x64-windows`, also set `VCPKGRS_DYNAMIC=1`.

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

- Selects the checked-in native ABI profile for the docs.rs target
- Falls back to the same shared binding specification only when the profile file
  is unavailable and the `bindgen` feature is present
- Skips native C/C++ compilation entirely

### Updating Pregenerated Bindings

Core bindings are generated as Windows, non-Windows, and WASM profiles from one
shared specification. To reproduce and compare all checked-in profiles:

```bash
cargo run -p xtask -- verify-bindings
```

After an intentional source or specification change, update all profiles and
then verify them:

```bash
cargo run -p xtask -- verify-bindings --update --allow-dirty
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys --submodules auto --wasm
```

Canonical generation rejects `BINDGEN_EXTRA_CLANG_ARGS*`. The target profile,
generator policy, header shims, enum normalization, formatter, and WASM provider
all participate in the binding-spec hash.

## WebAssembly Support

WebAssembly support for Dear ImGui in this workspace follows the same **import-style** design used by the high-level `dear-imgui-rs` crate:

- Rust code links against a WASM import module named `imgui-sys-v1` that provides the cimgui (C API) implementation.
- The main application (Rust + winit + wgpu) targets `wasm32-unknown-unknown` and uses `wasm-bindgen`.
- A separate provider module (`imgui-sys-v1`) is built once (currently via Emscripten) and contains Dear ImGui + cimgui and, optionally, selected extensions.

Provider ABI v1 includes the repository's checked numeric formatting and
parsing source transform. Older v0 providers are not compatible and must be
rebuilt; renaming or remapping a v0 artifact does not upgrade its semantics.

The `wasm` feature is mandatory for `wasm32-unknown-unknown`, the only supported
WASM target. WASI (`wasip1`/`wasip2`) and Emscripten targets are rejected even
with the feature because their runtime ABI cannot consume these import bindings.
The provider name is fixed; generation commands do not accept an alternate
import module. Enabling `wasm` on a native target is allowed and does not select
the WASM binding profile.

End users typically interact with the flow indirectly through:

- `dear-imgui-rs` with the `wasm` feature enabled.
- The `xtask` commands (`wasm-bindgen`, `web-demo`, `build-cimgui-provider`) that wire the main module and provider together.

For a complete, up-to-date guide (including required tools, commands, and troubleshooting), see:

- `docs/WASM.md` in this repository.
- The `examples-wasm` crate (`examples-wasm/dear-imgui-web-demo`), which demonstrates the web demo setup.

## Basic Usage

This is a low-level sys crate providing unsafe FFI bindings. Most users should use the higher-level [`dear-imgui-rs`](https://crates.io/crates/dear-imgui-rs) crate instead, which provides safe Rust wrappers.

Use the exact prerelease requirement:

```toml
[dependencies]
dear-imgui-sys = "=0.16.0-alpha.3"

# Enable features as needed
dear-imgui-sys = { version = "=0.16.0-alpha.3", features = ["freetype", "wasm"] }
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
dear-imgui-sys = { version = "=0.16.0-alpha.3", features = ["backend-shim-opengl3"] }
```

These features expose self-contained modules such as:

- `dear_imgui_sys::backend_shim::win32`
- `dear_imgui_sys::backend_shim::dx11`
- `dear_imgui_sys::backend_shim::android`
- `dear_imgui_sys::backend_shim::opengl3`

SDLRenderer3 and SDLGPU3 renderer shims are owned by `dear-imgui-sdl3`, not
`dear-imgui-sys`. Use `dear-imgui-sdl3` with feature `sdlrenderer3-renderer`
or `sdlgpu3-renderer` for those renderer integrations.

Important scope note:

- `backend-shim-*` exposes the repository-owned C shim ABI, not the original
  upstream C++ backend symbol names
- self-contained official backends may be compiled by `dear-imgui-sys` behind
  these features
- SDL3 renderer shims are framework-specific and are compiled by `dear-imgui-sdl3`
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

## Stack Layout Compatibility Shim

With feature `stack-layout`, `dear-imgui-sys` builds a repository-owned stack layout shim that
backs the safe `dear-imgui-rs` helpers named `begin_horizontal`,
`begin_vertical`, and `spring`.

```bash
cargo build -p dear-imgui-sys --features stack-layout
```

Scope notes:

- Dear ImGui itself does not ship `BeginHorizontal`, `BeginVertical`, or
  `Spring` as official public APIs.
- The shim is provided so Rust examples can follow the blueprint-style
  `imgui-node-editor` examples without patching the Dear ImGui submodule.
- Native source builds patch only the generated `OUT_DIR` copy of `imgui.cpp`
  to add the `ItemSize()` / `ItemAdd()` hooks that the upstream stack layout
  extension needs for regular ImGui widgets to be measured correctly.
- The implementation is derived from the MIT-licensed stack layout extension
  vendored by `imgui-node-editor`; see
  [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
- The Rust-facing ABI uses `dear_imgui_stack_*` symbols and is owned by this
  crate. Downstream code should prefer the safe `dear-imgui-rs` wrappers.
- Normal native builds compile the original Dear ImGui core and do not export the shim symbols.
- Official release prebuilt profiles match exactly: stack-layout artifacts use a `-stack-layout`
  archive suffix, or `-stack-layout-freetype` when FreeType is also enabled. Their manifests declare
  the same feature set, so neither can substitute for a normal or FreeType-only artifact.
- `stack-layout` is native-only and cannot be combined with the WASM feature or target.

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
   dear-imgui-rs = "=0.16.0-alpha.3"
   dear-imgui-sys = { version = "=0.16.0-alpha.3", features = ["backend-shim-android", "backend-shim-opengl3"] }
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
   application may provide SDL3 headers via `SDL3_INCLUDE_DIR`, rely on
   pkg-config/vcpkg discovery, or add a direct `sdl3` dependency with
   `features = ["build-from-source"]` so Cargo feature unification makes
   `sdl3-sys` export `DEP_SDL3_OUT_DIR`. When using the build-from-source route,
   the application still needs to provide the
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

- **Core C ABI**: cimgui exposes the core `ig*` calls through C; backend shims
  and callback signatures retain explicit platform/compiler ABI contracts
- **Complete API Coverage**: All Dear ImGui functions are available through the C API
- **Consistent Naming**: Functions follow the `ig*` naming convention (e.g., `igText`, `igButton`)
- **Pregenerated by default**: Checked-in bindings are copied into `OUT_DIR` for normal builds
- **Explicit regeneration**: Set `DEAR_IMGUI_RS_REGEN_BINDINGS=1` to run bindgen from cimgui headers

Two native binding files are selected by target facts rather than by the host
that published the crate:

- `bindings_pregenerated_windows.rs` for supported 64-bit Windows MSVC/GNU ABIs
- `bindings_pregenerated.rs` for supported Linux, Android, macOS, and iOS ABIs

WASM uses `wasm_bindings_pregenerated.rs` and imports the fixed
`imgui-sys-v1` provider. `ImGuiDockNode` is intentionally opaque and pointer-only;
C/C++ `va_list` APIs are omitted because neither has one portable Rust layout.

The packaged Cargo manifest records exact cimgui and nested Dear ImGui revisions
under `[package.metadata.dear-imgui-sources]`. Builds and artifact packaging use
that metadata without requiring a `.git` directory. Update and release checks
require both source submodules to be clean and to match the recorded revisions.

### Version Information

- **Dear ImGui Version**: v1.92.9b (docking branch)
- **cimgui Version**: Pinned to a revision generated against Dear ImGui v1.92.9b
- **Supported Features**: Docking, FreeType font rendering, and low-level PlatformIO/multi-viewport primitives; end-to-end status is documented per backend route

### Environment Variables

Control build behavior with these environment variables:

| Variable | Description |
|----------|-------------|
| `IMGUI_SYS_LIB_DIR` | Directory containing the core static library; a matching strict `manifest.txt` must be in that directory or its parent |
| `IMGUI_SYS_PREBUILT_URL` | Local path or direct URL to a package-tool-generated core archive; HTTP(S) and `.tar.gz` extraction require feature `prebuilt` |
| `IMGUI_SYS_USE_PREBUILT` | Enable automatic download from GitHub releases (`1`, requires feature `prebuilt`) |
| `IMGUI_SYS_SKIP_CC` | Skip C/C++ compilation, use pregenerated bindings only (`1`) |
| `IMGUI_SYS_FORCE_BUILD` | Force build from source, ignore prebuilt options (`1`) |
| `DEAR_IMGUI_RS_REGEN_BINDINGS` | Regenerate Rust bindings with bindgen (`1`; requires `--features bindgen` and libclang) |

Bare `.a`/`.lib` inputs are not trusted core artifacts. An explicit library is
accepted only when its directory or parent artifact root also contains the
complete matching `manifest.txt`; using the packaged `.tar.gz` is recommended.

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
