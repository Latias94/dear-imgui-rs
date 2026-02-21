# dear-imgui-test-engine-sys

Low-level FFI bindings for Dear ImGui Test Engine via a small C shim over the upstream C++ API.

This crate pairs with `dear-imgui-sys` and is intended for advanced users. Most applications should use `dear-imgui-test-engine`.

- Upstream: https://github.com/ocornut/imgui_test_engine
- Submodule path: `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine`

## Features

- `freetype`: passthrough to `dear-imgui-sys/freetype`.
- `capture` (default): enable screenshot/video capture helpers (`IMGUI_TEST_ENGINE_ENABLE_CAPTURE=1`).

## Build Modes

- Source build (default)
  - Compiles Dear ImGui Test Engine sources + crate shim using `cc`.
  - Inherits include paths/defines from `dear-imgui-sys`.
- Docs.rs
  - Uses pregenerated Rust bindings and skips native C/C++ compilation.

## Environment Variables

- `IMGUI_TEST_ENGINE_SYS_SKIP_CC`
  - If set, skip native C/C++ compilation and use pregenerated bindings.
  - Useful for cross-target `cargo check` or constrained CI jobs.

The build script also consumes values exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*` (including `IMGUITEST` from the `test-engine` feature)

## Notes

- This crate requires `dear-imgui-sys` to be compiled with `IMGUI_ENABLE_TEST_ENGINE` (enabled automatically through dependency features).
- Linking of the base ImGui static library is handled by `dear-imgui-sys`.
- A small built-in demo test set is bundled for validating integration via `imgui_test_engine_register_default_tests()`.
- Upstream Dear ImGui Test Engine has custom license terms. Review `LICENSE.txt` (this crate) and
  `third-party/imgui_test_engine/imgui_test_engine/LICENSE.txt` (upstream) for usage conditions.
