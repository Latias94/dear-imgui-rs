# dear-imgui-test-engine-sys

Low-level FFI bindings for Dear ImGui Test Engine via a small C shim over the upstream C++ API.

This crate pairs with `dear-imgui-sys` and is intended for advanced users. Most applications should use `dear-imgui-test-engine`.

- Upstream: https://github.com/ocornut/imgui_test_engine
- Submodule path: `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine`

## ABI Contract

- Every shim operation returns `ImGuiTestEngineStatus`; values are written through validated output
  pointers only after the operation succeeds.
- The shim catches C++ exceptions at every exported boundary. No exception is allowed to unwind into
  Rust. Call `imgui_test_engine_get_last_error()` immediately after a failing operation to copy its
  thread-local diagnostic.
- Engine and script handles are non-thread-safe and must follow the documented create, transfer,
  unbind, and destroy lifecycle. A successfully destroyed raw handle is invalid immediately.
- Prefer the safe `dear-imgui-test-engine` crate when Context identity, frame scope, or retryable
  teardown matters.

## Features

- `freetype`: passthrough to `dear-imgui-sys/freetype`.
- `capture` (default): enable screenshot/video capture helpers (`IMGUI_TEST_ENGINE_ENABLE_CAPTURE=1`).

## Build Modes

- Native source build (the only runtime route)
  - Compiles Dear ImGui Test Engine sources + crate shim using `cc`.
  - Inherits include paths/defines from `dear-imgui-sys`.
- Docs.rs
  - Uses pregenerated Rust bindings and skips native C/C++ compilation.

There is deliberately no Test Engine prebuilt or WASM feature. The paired
`dear-imgui-sys/test-engine` feature forces the core native artifact to build
from source because the hooks change that artifact. WASM targets and
prebuilt-package generation reject this combination before linking. Workspace
`--all-features` is therefore not a supported Test Engine build command.

The checked-in bindings are part of the repository-wide canonical binding
contract. Their crate-local profile records the exact upstream Test Engine
revision, wrapper/header shim, native target assumptions, generator settings,
and deterministic specification hash. Reproduce and compare them with:

```bash
cargo run -p xtask -- verify-bindings
```

After an intentional source or binding-specification change, maintainers use
`cargo run -p xtask -- verify-bindings --update --allow-dirty` and review the
source metadata and generated diff together. Test Engine provenance is kept
separate from native prebuilt artifact manifests because no Test Engine
prebuilt is published.

## Environment Variables

- `IMGUI_TEST_ENGINE_SYS_SKIP_CC`
  - If set, skip native C/C++ compilation and use pregenerated bindings.
  - Useful only for docs/binding checks and constrained CI jobs; it does not
    provide a linkable Test Engine runtime.

The build script also consumes values exported by `dear-imgui-sys`:

- `DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH`, `DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH`
- `DEP_DEAR_IMGUI_DEFINE_*` (including `IMGUITEST` from the `test-engine` feature)

## Notes

- This crate requires `dear-imgui-sys` to be compiled with `IMGUI_ENABLE_TEST_ENGINE` (enabled automatically through dependency features).
- Linking of the base ImGui static library is handled by `dear-imgui-sys`.
- A small built-in demo test set is bundled for validating integration via `imgui_test_engine_register_default_tests()`.
- Upstream Dear ImGui Test Engine has custom license terms. Review `LICENSE.txt` (this crate) and
  `third-party/imgui_test_engine/imgui_test_engine/LICENSE.txt` (upstream) for usage conditions.

Feature consistency:

- If you enable `dear-imgui-sys/test-engine`, the compiled ImGui objects reference hook symbols (e.g. `ImGuiTestEngineHook_*`).
  When that feature is enabled, `dear-imgui-sys` provides the hook symbols so workspace feature-unification won't cause linker errors.
- To actually run tests, link a binary with `dear-imgui-test-engine(-sys)`, which registers the real hook implementations at runtime.
