# dear-implot3d-sys

Low-level FFI bindings for ImPlot3D via the `cimplot3d` C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes raw functions/types used by the high-level `dear-implot3d` crate.

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (the env toggle `IMPLOT3D_SYS_USE_PREBUILT=1` requires this feature).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

This crate supports multiple ways to obtain the native `dear_implot3d` static library:

- Source build (default)
  - Compiles `cimplot3d.cpp` and upstream `implot3d/*.cpp` with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide (see env vars below).
- Docs.rs
  - Generates Rust bindings only; native code is not compiled.

## Environment Variables

- `IMPLOT3D_SYS_LIB_DIR`
  - Directory containing the prebuilt static library.
  - Expected names: `dear_implot3d.lib` (Windows/MSVC), `libdear_implot3d.a` (Unix).
- `IMPLOT3D_SYS_PREBUILT_URL`
  - Local file path or direct URL to the prebuilt static library file, or to a `.tar.gz` package produced by our packager (HTTP(S) and `.tar.gz` extraction require feature `prebuilt`).
  - Downloaded to `OUT_DIR/prebuilt/` and reused on subsequent builds.
- `IMPLOT3D_SYS_USE_PREBUILT`
  - If set to `1` or the `prebuilt` feature is enabled, the build script may auto-download a release asset.
- `IMPLOT3D_SYS_SKIP_CC`
  - If set, skips native C/C++ compilation. Typically used with one of the above.
- `IMPLOT3D_SYS_FORCE_BUILD`
  - Force building from source even if a prebuilt could be used.
- `IMPLOT3D_SYS_PACKAGE_DIR`, `IMPLOT3D_SYS_CACHE_DIR`
  - Overrides for local package discovery and cache locations.

The build script also consumes include paths and defines exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*`

## Examples

- Default (source build):
```
cargo build -p dear-implot3d-sys -p dear-implot3d
```

- System/prebuilt (Windows):
```
$env:IMPLOT3D_SYS_LIB_DIR = "C:\\prebuilt\\implot3d"
cargo build -p dear-implot3d-sys
```

- System/prebuilt (Unix):
```
export IMPLOT3D_SYS_LIB_DIR=/opt/implot3d/lib
cargo build -p dear-implot3d-sys
```

- Remote prebuilt download:
```
# Windows: URL must point to dear_implot3d.lib or a .tar.gz package
$env:IMPLOT3D_SYS_PREBUILT_URL = "https://example.com/dear_implot3d.lib"

# Unix: URL must point to libdear_implot3d.a or a .tar.gz package
export IMPLOT3D_SYS_PREBUILT_URL=https://example.com/libdear_implot3d.a

cargo build -p dear-implot3d-sys
```

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds generate bindings only and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-implot3d/`. See that crate and `examples/implot3d_basic.rs` for usage.
