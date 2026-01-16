# dear-implot-sys

Low-level FFI bindings for ImPlot via the cimplot C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes `ImPlot_*` functions/types for higher-level crates (`dear-implot`).

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (the env toggle `IMPLOT_SYS_USE_PREBUILT=1` requires this feature).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `freetype`: passthrough to `dear-imgui-sys/freetype` to enable FreeType in the workspace.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

This crate supports three ways to obtain the native `dear_implot` static library:

- Source build (default)
  - Compiles `cimplot.cpp` and embedded `implot/*.cpp` with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide.
- Docs.rs
  - Generates Rust bindings only; native code is not compiled.

## Environment Variables

- `IMPLOT_SYS_LIB_DIR`
  - Directory containing the prebuilt static library.
  - Expected names: `dear_implot.lib` (Windows/MSVC), `libdear_implot.a` (Unix).
- `IMPLOT_SYS_PREBUILT_URL`
  - Local file path or direct URL to the prebuilt static library file (HTTP(S) and `.tar.gz` extraction require feature `prebuilt`).
  - The file is downloaded to `OUT_DIR/prebuilt/` and reused on subsequent builds.
- `IMPLOT_SYS_SKIP_CC`
  - If set, skips native C/C++ compilation. Typically used with one of the above.

The build script also consumes the include paths and defines exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*`

## Examples

- Default (source build):
```
cargo build -p dear-implot-sys -p dear-implot
```

- System/prebuilt library (Windows):
```
$env:IMPLOT_SYS_LIB_DIR = "C:\\prebuilt\\implot"
cargo build -p dear-implot-sys
```

- System/prebuilt library (Unix):
```
export IMPLOT_SYS_LIB_DIR=/opt/implot/lib
cargo build -p dear-implot-sys
```

- Remote prebuilt download:
```
# Windows: URL must point to dear_implot.lib
$env:IMPLOT_SYS_PREBUILT_URL = "https://example.com/dear_implot.lib"

# Unix: URL must point to libdear_implot.a
export IMPLOT_SYS_PREBUILT_URL=https://example.com/libdear_implot.a

cargo build -p dear-implot-sys
```

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds generate bindings only and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-implot/`. See that crate for usage patterns and examples.
