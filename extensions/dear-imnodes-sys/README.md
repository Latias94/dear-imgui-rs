# dear-imnodes-sys

Low-level FFI bindings for ImNodes via the cimnodes C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes `imnodes_*` functions/types for higher-level crates (`dear-imnodes`).

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (or when `IMNODES_SYS_USE_PREBUILT=1`).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `freetype`: passthrough to `dear-imgui-sys/freetype` to enable FreeType in the workspace.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

This crate supports three ways to obtain the native `dear_imnodes` static library:

- Source build (default)
  - Compiles `cimnodes.cpp` and upstream `imnodes/imnodes.cpp` with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide.
- Docs.rs
  - Generates Rust bindings only; native code is not compiled.

## Environment Variables

- `IMNODES_SYS_LIB_DIR`
  - Directory containing the prebuilt static library.
  - Expected names: `dear_imnodes.lib` (Windows/MSVC), `libdear_imnodes.a` (Unix).
- `IMNODES_SYS_PREBUILT_URL`
  - Direct URL to the prebuilt static library file.
  - The file is downloaded to `OUT_DIR/prebuilt/` and reused on subsequent builds.
- `IMNODES_SYS_SKIP_CC`
  - If set, skips native C/C++ compilation. Typically used with one of the above.

The build script also consumes the include paths and defines exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*`

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds generate bindings only and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-imnodes/`. See that crate and `examples/imnodes_basic.rs` for usage.

