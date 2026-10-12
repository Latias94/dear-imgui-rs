# dear-imguizmo-quat-sys

Low-level FFI bindings for ImGuIZMO.quat via the `cimguizmo_quat` C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes raw functions/types used by the high-level `dear-imguizmo-quat` crate.

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (the env toggle `IMGUIZMO_QUAT_SYS_USE_PREBUILT=1` requires this feature).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `freetype`: passthrough to `dear-imgui-sys/freetype` to enable FreeType in the workspace.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

Prebuilt artifacts must match the release train, target, CRT, source revisions,
binding identity, feature profile, and shared Dear ImGui core recorded in their
manifest. Bare remote `.lib`/`.a` downloads do not supply that manifest.
`build-from-source` (or `IMGUIZMO_QUAT_SYS_FORCE_BUILD`) forces source compilation.
Choose one prebuilt route at a time: clear `IMGUIZMO_QUAT_SYS_LIB_DIR` before using
`IMGUIZMO_QUAT_SYS_PREBUILT_URL`; a local directory override takes precedence.
Source builds are the default and do not require libclang unless binding
regeneration is explicitly requested. `wasm` uses import bindings for the fixed
`imgui-sys-v1` provider and skips C++ compilation in Cargo; build the provider
separately with Emscripten.

This crate supports multiple ways to obtain the native `dear_imguizmo_quat` static library:

- Source build (default)
  - Compiles the C wrapper and upstream sources with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide (see env vars below).
- Docs.rs
  - Uses validated checked-in Rust bindings; native code is not compiled.

## Environment Variables

- `IMGUIZMO_QUAT_SYS_LIB_DIR`
  - Directory containing the matching static library and its packaged manifest.
  - Expected names: `dear_imguizmo_quat.lib` (Windows/MSVC), `libdear_imguizmo_quat.a` (Unix).
- `IMGUIZMO_QUAT_SYS_PREBUILT_URL`
  - Local path or URL to a matching `.tar.gz` package containing the library and manifest (HTTP(S) and archive extraction require `prebuilt`). A local bare library path is accepted only when the matching manifest is beside it.
  - Downloads/extractions are reused from the prebuilt cache; `IMGUIZMO_QUAT_SYS_CACHE_DIR` overrides its root.
- `IMGUIZMO_QUAT_SYS_USE_PREBUILT`
  - Automatic release lookup requires the `prebuilt` feature. This environment toggle does not enable downloads by itself.
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
$env:IMGUIZMO_QUAT_SYS_LIB_DIR = "C:\prebuilt\imguizmo_quat"
cargo build -p dear-imguizmo-quat-sys
```

- System/prebuilt (Unix):
```
export IMGUIZMO_QUAT_SYS_LIB_DIR=/opt/imguizmo_quat/lib
cargo build -p dear-imguizmo-quat-sys
```

- Remote prebuilt download (choose the block for your shell):

```powershell
$env:IMGUIZMO_QUAT_SYS_LIB_DIR = $null
$env:IMGUIZMO_QUAT_SYS_PREBUILT_URL = "https://example.com/dear_imguizmo_quat-matching-profile.tar.gz"
cargo build -p dear-imguizmo-quat-sys --features prebuilt
```

```sh
unset IMGUIZMO_QUAT_SYS_LIB_DIR
export IMGUIZMO_QUAT_SYS_PREBUILT_URL=https://example.com/dear_imguizmo_quat-matching-profile.tar.gz
cargo build -p dear-imguizmo-quat-sys --features prebuilt
```

Replace the illustrative URL with an archive for your exact profile. Clear the
URL override before switching back to automatic release lookup or source builds.

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds use checked-in bindings and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-imguizmo-quat/`. See that crate and `examples/03-features/imguizmo_quat_minimal.rs` for a copy-ready integration.
