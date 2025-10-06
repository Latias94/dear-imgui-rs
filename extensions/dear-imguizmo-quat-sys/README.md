# dear-imguizmo-quat-sys

Low-level FFI bindings for ImGuIZMO.quat via the `cimguizmo_quat` C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes raw functions/types used by the high-level `dear-imguizmo-quat` crate.

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (or when `IMGUIZMO_QUAT_SYS_USE_PREBUILT=1`).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `freetype`: passthrough to `dear-imgui-sys/freetype` to enable FreeType in the workspace.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

This crate supports multiple ways to obtain the native `dear_imguizmo_quat` static library:

- Source build (default)
  - Compiles the C wrapper and upstream sources with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide (see env vars below).
- Docs.rs
  - Generates Rust bindings only; native code is not compiled.

## Environment Variables

- `IMGUIZMO_QUAT_SYS_LIB_DIR`
  - Directory containing the prebuilt static library.
  - Expected names: `dear_imguizmo_quat.lib` (Windows/MSVC), `libdear_imguizmo_quat.a` (Unix).
- `IMGUIZMO_QUAT_SYS_PREBUILT_URL`
  - Direct URL to the prebuilt static library file, or to a `.tar.gz` package produced by our packager.
  - Downloaded to `OUT_DIR/prebuilt/` and reused on subsequent builds.
- `IMGUIZMO_QUAT_SYS_USE_PREBUILT`
  - If set to `1` or the `prebuilt` feature is enabled, the build script may auto-download a release asset.
- `IMGUIZMO_QUAT_SYS_SKIP_CC`
  - If set, skips native C/C++ compilation. Typically used with one of the above.
- `IMGUIZMO_QUAT_SYS_FORCE_BUILD`
  - Force building from source even if a prebuilt could be used.
- `IMGUIZMO_QUAT_SYS_PACKAGE_DIR`, `IMGUIZMO_QUAT_SYS_CACHE_DIR`
  - Overrides for local package discovery and cache locations.

The build script also consumes include paths and defines exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*`

## Examples

- Default (source build):
```
cargo build -p dear-imguizmo-quat-sys -p dear-imguizmo-quat
```

- System/prebuilt (Windows):
```
$env:IMGUIZMO_QUAT_SYS_LIB_DIR = "C:\\prebuilt\\imguizmo_quat"
cargo build -p dear-imguizmo-quat-sys
```

- System/prebuilt (Unix):
```
export IMGUIZMO_QUAT_SYS_LIB_DIR=/opt/imguizmo_quat/lib
cargo build -p dear-imguizmo-quat-sys
```

- Remote prebuilt download:
```
# Windows: URL must point to dear_imguizmo_quat.lib or a .tar.gz package
$env:IMGUIZMO_QUAT_SYS_PREBUILT_URL = "https://example.com/dear_imguizmo_quat.lib"

# Unix: URL must point to libdear_imguizmo_quat.a or a .tar.gz package
export IMGUIZMO_QUAT_SYS_PREBUILT_URL=https://example.com/libdear_imguizmo_quat.a

cargo build -p dear-imguizmo-quat-sys
```

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds generate bindings only and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-imguizmo-quat/`. See that crate and `examples/imguizmo_quat_basic.rs` for usage.

