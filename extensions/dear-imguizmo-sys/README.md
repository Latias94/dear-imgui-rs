# dear-imguizmo-sys

Low-level FFI bindings for ImGuizmo via the cimguizmo C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes `ImGuizmo_*` functions/types for higher-level crates (`dear-imguizmo`).

## Build Modes

This crate supports three ways to obtain the native `dear_imguizmo` static library:

- Source build (default)
  - Compiles `cimguizmo.cpp` and upstream `ImGuizmo/ImGuizmo.cpp` with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide (see env vars below).
- Docs.rs
  - Generates Rust bindings only; native code is not compiled.

## Environment Variables

- `IMGUIZMO_SYS_LIB_DIR`
  - Directory containing the prebuilt static library.
  - Expected names: `dear_imguizmo.lib` (Windows/MSVC), `libdear_imguizmo.a` (Unix).
- `IMGUIZMO_SYS_PREBUILT_URL`
  - Direct URL to the prebuilt static library file.
  - The file is downloaded to `OUT_DIR/prebuilt/` and reused on subsequent builds.
- `IMGUIZMO_SYS_SKIP_CC`
  - If set, skips native C/C++ compilation. Typically used with one of the above.

The build script also consumes include paths and defines exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*`

## Examples

- Default (source build):
```
cargo build -p dear-imguizmo-sys -p dear-imguizmo
```

- System/prebuilt (Windows):
```
$env:IMGUIZMO_SYS_LIB_DIR = "C:\\prebuilt\\imguizmo"
cargo build -p dear-imguizmo-sys
```

- System/prebuilt (Unix):
```
export IMGUIZMO_SYS_LIB_DIR=/opt/imguizmo/lib
cargo build -p dear-imguizmo-sys
```

- Remote prebuilt download:
```
# Windows: URL must point to dear_imguizmo.lib
$env:IMGUIZMO_SYS_PREBUILT_URL = "https://example.com/dear_imguizmo.lib"

# Unix: URL must point to libdear_imguizmo.a
export IMGUIZMO_SYS_PREBUILT_URL=https://example.com/libdear_imguizmo.a

cargo build -p dear-imguizmo-sys
```

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds generate bindings only and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-imguizmo/`. See that crate and `examples/imguizmo_basic.rs` for usage.
