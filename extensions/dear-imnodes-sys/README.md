# dear-imnodes-sys

Low-level FFI bindings for ImNodes via the cimnodes C API.
This crate pairs with `dear-imgui-sys` (cimgui C API) and exposes `imnodes_*`
functions/types for higher-level crates (`dear-imnodes`).

- Upstream (C++): ImNodes — https://github.com/Nelarius/imnodes
- C API we bind to: cimnodes — https://github.com/cimgui/cimnodes

Notes:
- We intentionally bind the C API (cimnodes) rather than the C++ library directly
  to avoid C++ ABI/toolchain issues and to align with `dear-imgui-sys` (which
  binds Dear ImGui via cimgui).
- The cimnodes submodule lives under `extensions/dear-imnodes-sys/third-party/cimnodes`.

## Features

- `prebuilt`: allow the build script to auto-download a release archive when available (the env toggle `IMNODES_SYS_USE_PREBUILT=1` requires this feature).
- `build-from-source`: force building native sources with `cc` even if a prebuilt could be linked.
- `freetype`: passthrough to `dear-imgui-sys/freetype` to enable FreeType in the workspace.
- `package-bin`: enable an internal `bin/package` helper to produce release artifacts.

## Build Modes

Prebuilt artifacts must match the release train, target, CRT, source revisions,
binding identity, feature profile, and shared Dear ImGui core recorded in their
manifest. Bare remote `.lib`/`.a` downloads do not supply that manifest.

`IMNODES_SYS_BUILD_MODE` accepts `auto` (default), `source`, `prebuilt`, or `bindings-only`. In the 0.22 release train, explicit `IMNODES_SYS_LIB_DIR`, `IMNODES_SYS_PREBUILT_URL`, and `IMNODES_SYS_PACKAGE_DIR` selectors are mutually exclusive; missing or incompatible selections stop the build. Strict prebuilt mode never falls back to source. In automatic mode, `build-from-source` or `IMNODES_SYS_FORCE_BUILD=1` forces source compilation; explicit prebuilt and bindings-only modes reject source-only requirements. See the [native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy).

Source builds are the default and do not require libclang unless binding
regeneration is explicitly requested. `wasm` uses import bindings for the fixed
`imgui-sys-v1` provider and skips C++ compilation in Cargo; build the provider
separately with Emscripten.

Native and WebAssembly providers share the `imnodes-imgui-193-v1` adaptation for
Dear ImGui 1.93 corner flags and its file-handle API. Source-drift checks reject
unexpected upstream changes, and artifact identity includes this adaptation.

This crate supports three ways to obtain the native `dear_imnodes` static library:

- Source build (default)
  - Compiles `cimnodes.cpp` and upstream `imnodes/imnodes.cpp` with `cc`.
  - Inherits include paths and preprocessor defines from `dear-imgui-sys`.
- System/prebuilt library
  - Links an existing static library from a directory you provide.
- Docs.rs
  - Uses validated checked-in Rust bindings; native code is not compiled.

## Environment Variables

- `IMNODES_SYS_BUILD_MODE`
  - Select `auto`, `source`, `prebuilt`, or `bindings-only`. Do not combine this with enabled legacy `SKIP_CC` or `FORCE_BUILD` flags.
- `IMNODES_SYS_LIB_DIR`
  - Directory containing the matching static library and its packaged manifest.
  - Expected names: `dear_imnodes.lib` (Windows/MSVC), `libdear_imnodes.a` (Unix).
- `IMNODES_SYS_PREBUILT_URL`
  - Local path or URL to a matching `.tar.gz` package containing the library and manifest (HTTP(S) and archive extraction require `prebuilt`). A local bare library path is accepted only when the matching manifest is beside it.
  - Downloaded files are cached and reused on subsequent builds.
- `IMNODES_SYS_SKIP_CC`
  - Legacy boolean: `1` selects bindings-only without artifact intent, or strict prebuilt when an explicit selector or feature `prebuilt` is present. It conflicts with source requirements. False values disable the flag.

The build script also consumes the include paths and defines exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*`

## Notes

- Linking to the base ImGui static library is provided by `dear-imgui-sys`; this crate does not duplicate `cargo:rustc-link-lib` for it.
- MSVC (Windows) builds align CRT and exception flags with `dear-imgui-sys`.
- `docs.rs` builds use checked-in bindings and export include paths for downstream crates.
- Higher-level Rust APIs live in `extensions/dear-imnodes/`. See that crate and `examples/03-features/imnodes_minimal.rs` for a copy-ready integration.
