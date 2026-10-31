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

`IMGUIZMO_QUAT_SYS_BUILD_MODE` accepts `auto` (default), `source`, `prebuilt`, or `bindings-only`. In the 0.22 release train, explicit `IMGUIZMO_QUAT_SYS_LIB_DIR`, `IMGUIZMO_QUAT_SYS_PREBUILT_URL`, and `IMGUIZMO_QUAT_SYS_PACKAGE_DIR` selectors are mutually exclusive; missing or incompatible selections stop the build. Strict prebuilt mode never falls back to source. In automatic mode, `build-from-source` or `IMGUIZMO_QUAT_SYS_FORCE_BUILD=1` forces source compilation; explicit prebuilt and bindings-only modes reject source-only requirements. See the [native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy).

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

- `IMGUIZMO_QUAT_SYS_BUILD_MODE`
  - Select `auto`, `source`, `prebuilt`, or `bindings-only`. Do not combine this with enabled legacy `SKIP_CC` or `FORCE_BUILD` flags.
- `IMGUIZMO_QUAT_SYS_LIB_DIR`
  - Directory containing the matching static library and its packaged manifest.
  - Expected names: `dear_imguizmo_quat.lib` (Windows/MSVC), `libdear_imguizmo_quat.a` (Unix).
- `IMGUIZMO_QUAT_SYS_PREBUILT_URL`
  - Local path or URL to a matching `.tar.gz` package containing the library and manifest (HTTP(S) and archive extraction require `prebuilt`). A local bare library path is accepted only when the matching manifest is beside it.
  - Downloads/extractions are reused from the prebuilt cache; `IMGUIZMO_QUAT_SYS_CACHE_DIR` overrides its root.
- `IMGUIZMO_QUAT_SYS_USE_PREBUILT`
  - Automatic release lookup requires the `prebuilt` feature. This environment toggle does not enable downloads by itself.
- `IMGUIZMO_QUAT_SYS_SKIP_CC`
  - Legacy boolean: `1` selects bindings-only without artifact intent, or strict prebuilt when an explicit selector or feature `prebuilt` is present. It conflicts with source requirements. False values disable the flag.
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
