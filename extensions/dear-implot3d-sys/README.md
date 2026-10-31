# dear-implot3d-sys

Low-level FFI bindings for ImPlot3D via the `cimplot3d` C API. This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes raw functions/types used by the high-level `dear-implot3d` crate.

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (the env toggle `IMPLOT3D_SYS_USE_PREBUILT=1` requires this feature).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

Prebuilt artifacts must match the release train, target, CRT, source revisions,
binding identity, feature profile, and shared Dear ImGui core recorded in their
manifest. Bare remote `.lib`/`.a` downloads do not supply that manifest.

`IMPLOT3D_SYS_BUILD_MODE` accepts `auto` (default), `source`, `prebuilt`, or `bindings-only`. In the 0.22 release train, explicit `IMPLOT3D_SYS_LIB_DIR`, `IMPLOT3D_SYS_PREBUILT_URL`, and `IMPLOT3D_SYS_PACKAGE_DIR` selectors are mutually exclusive; missing or incompatible selections stop the build. Strict prebuilt mode never falls back to source. In automatic mode, `build-from-source` or `IMPLOT3D_SYS_FORCE_BUILD=1` forces source compilation; explicit prebuilt and bindings-only modes reject source-only requirements. See the [native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy).

Source builds are the default and do not require libclang unless binding
regeneration is explicitly requested. `wasm` uses import bindings for the fixed
`imgui-sys-v1` provider and skips C++ compilation in Cargo; build the provider
separately with Emscripten.

Native and WebAssembly providers share an `implot3d-draw-flags-v2` source adaptation
for Dear ImGui 1.93. It updates the internal drawing flags and delegates line texture
selection to Dear ImGui, preserving fractional widths and framebuffer density while
avoiding atlas-index overflow on thick lines. Lines widened to one physical pixel
retain their original alpha coverage. The adaptation is part of artifact identity.
Legend scrolling also requires the plot's window to be hovered, so overlapping windows retain input.

This crate supports multiple ways to obtain the native `dear_implot3d` static library:

- Source build (default)
  - Compiles `cimplot3d.cpp` and upstream `implot3d/*.cpp` with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide (see env vars below).
- Docs.rs
  - Uses validated checked-in Rust bindings; native code is not compiled.

## Environment Variables

- `IMPLOT3D_SYS_BUILD_MODE`
  - Select `auto`, `source`, `prebuilt`, or `bindings-only`. Do not combine this with enabled legacy `SKIP_CC` or `FORCE_BUILD` flags.
- `IMPLOT3D_SYS_LIB_DIR`
  - Directory containing the matching static library and its packaged manifest.
  - Expected names: `dear_implot3d.lib` (Windows/MSVC), `libdear_implot3d.a` (Unix).
- `IMPLOT3D_SYS_PREBUILT_URL`
  - Local path or URL to a matching `.tar.gz` package containing the library and manifest (HTTP(S) and archive extraction require `prebuilt`). A local bare library path is accepted only when the matching manifest is beside it.
  - Downloads/extractions are reused from the prebuilt cache; `IMPLOT3D_SYS_CACHE_DIR` overrides its root.
- `IMPLOT3D_SYS_USE_PREBUILT`
  - Automatic release lookup requires the `prebuilt` feature. This environment toggle does not enable downloads by itself.
- `IMPLOT3D_SYS_SKIP_CC`
  - Legacy boolean: `1` selects bindings-only without artifact intent, or strict prebuilt when an explicit selector or feature `prebuilt` is present. It conflicts with source requirements. False values disable the flag.
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
$env:IMPLOT3D_SYS_LIB_DIR = "C:\prebuilt\implot3d"
cargo build -p dear-implot3d-sys
```

- System/prebuilt (Unix):
```
export IMPLOT3D_SYS_LIB_DIR=/opt/implot3d/lib
cargo build -p dear-implot3d-sys
```

- Remote prebuilt download (choose the block for your shell):

```powershell
$env:IMPLOT3D_SYS_LIB_DIR = $null
$env:IMPLOT3D_SYS_PREBUILT_URL = "https://example.com/dear_implot3d-matching-profile.tar.gz"
cargo build -p dear-implot3d-sys --features prebuilt
```

```sh
unset IMPLOT3D_SYS_LIB_DIR
export IMPLOT3D_SYS_PREBUILT_URL=https://example.com/dear_implot3d-matching-profile.tar.gz
cargo build -p dear-implot3d-sys --features prebuilt
```

Replace the illustrative URL with an archive for your exact profile. Clear the
URL override before switching back to automatic release lookup or source builds.

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds use checked-in bindings and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-implot3d/`. See that crate and `examples/03-features/implot3d_minimal.rs` for a copy-ready integration.
