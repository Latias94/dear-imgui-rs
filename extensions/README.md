# Dear ImGui Extensions

Extensions in this workspace build on top of `dear-imgui-sys` (cimgui C API) and `dear-imgui-rs` to provide extra functionality like plotting, 3D gizmos, and reflection-based UI helpers.

| Extension      | Description                     | Status    | Rust Crate                                                                 | Upstream C API / Reference                                   |
|----------------|---------------------------------|-----------|----------------------------------------------------------------------------|--------------------------------------------------------------|
| ImPlot         | Scientific plotting             | Complete  | [dear-implot](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-implot)         | [cimgui/cimplot](https://github.com/cimgui/cimplot)          |
| ImPlot3D       | 3D scientific plotting          | Complete  | [dear-implot3d](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-implot3d)     | [cimgui/cimplot3d](https://github.com/cimgui/cimplot3d)      |
| ImGuizmo       | 3D transform gizmos             | Complete  | [dear-imguizmo](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imguizmo)     | [cimgui/cimguizmo](https://github.com/cimgui/cimguizmo)      |
| ImGuIZMO.quat  | Quaternion + 3D gizmo           | Complete  | [dear-imguizmo-quat](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imguizmo-quat) | [cimgui/cimguizmo_quat](https://github.com/cimgui/cimguizmo_quat) |
| ImNodes        | Node editor widgets             | Complete  | [dear-imnodes](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imnodes)       | [cimgui/cimnodes](https://github.com/cimgui/cimnodes)        |
| Node Editor    | Native node editor + blueprints | Complete  | [dear-node-editor](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-node-editor) | [cimgui/cimnodes_editor](https://github.com/cimgui/cimnodes_editor) |
| ImGui Test Engine | UI automation and test runner | Preview | [dear-imgui-test-engine](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imgui-test-engine) | [ocornut/imgui_test_engine](https://github.com/ocornut/imgui_test_engine) |
| File Browser   | File dialogs + in-UI browser    | Preview   | [dear-file-browser](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-file-browser) | Pure ImGui UI + rfd (native)                                 |
| ImGui Reflect  | Reflection-based UI from types  | Preview   | [dear-imgui-reflect](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imgui-reflect) | C++ ImReflect (reference only; pure Rust implementation)     |

## Architecture

Most extensions use C bindings plus checked-in pregenerated bindgen output (no C++ bindgen):

```
Core:        dear-imgui-sys (cimgui C API)  ->  dear-imgui-rs (safe Rust)
Extensions:  dear-xxx-sys (C API + pregenerated bindings) ->  dear-xxx (safe Rust)
Pure-Rust:   dear-file-browser / dear-imgui-reflect build directly on dear-imgui-rs
```

Key points:
- `*-sys` crates bind to C APIs (cimgui/cimplot/cimguizmo) with committed bindgen output by default.
- High-level crates wrap C APIs with RAII tokens and builder-style ergonomics.
- Linking of the base ImGui static library is unified by `dear-imgui-sys` -> extensions should not duplicate link flags for it.
- `dear-imgui-rs` draw APIs accept `Into<ImVec2>`, so arrays, tuples, `mint::Vector2<f32>`, and `ImVec2` all work out of the box.

## Build Modes

Each `*-sys` crate supports multiple ways to obtain its own native static library (see each `-sys` README for details):

- Source build (default): compile upstream C/C++ sources with `cc`.
- System/prebuilt: set a directory env var so Cargo can find the static lib.
- Remote prebuilt: set a direct URL (requires feature `prebuilt`); the file is downloaded into `OUT_DIR/prebuilt/`.

Environment variables:

- ImPlot: `IMPLOT_SYS_LIB_DIR`, `IMPLOT_SYS_PREBUILT_URL`, `IMPLOT_SYS_SKIP_CC`.
- ImPlot3D: `IMPLOT3D_SYS_LIB_DIR`, `IMPLOT3D_SYS_PREBUILT_URL`, `IMPLOT3D_SYS_SKIP_CC`.
- ImGuizmo: `IMGUIZMO_SYS_LIB_DIR`, `IMGUIZMO_SYS_PREBUILT_URL`, `IMGUIZMO_SYS_SKIP_CC`.
- ImNodes: `IMNODES_SYS_LIB_DIR`, `IMNODES_SYS_PREBUILT_URL`, `IMNODES_SYS_SKIP_CC`.
- Node Editor: `NODE_EDITOR_SYS_LIB_DIR`, `NODE_EDITOR_SYS_PREBUILT_URL`, `NODE_EDITOR_SYS_SKIP_CC`.
- ImGuIZMO.quat: `IMGUIZMO_QUAT_SYS_LIB_DIR`, `IMGUIZMO_QUAT_SYS_PREBUILT_URL`, `IMGUIZMO_QUAT_SYS_SKIP_CC`.
- ImGui Test Engine: source build only (`IMGUI_TEST_ENGINE_SYS_SKIP_CC` supported for pregenerated-bindings check/docs workflows).

Optional toggles:

- Auto-download prebuilt archives: feature `prebuilt` (the env toggle `*_SYS_USE_PREBUILT=1` requires the feature)
- Force build from sources: feature `build-from-source` or `*_SYS_FORCE_BUILD=1`

See also:
- ImPlot details: `extensions/dear-implot-sys/README.md`.
- ImPlot3D details: `extensions/dear-implot3d-sys/README.md`.
- ImGuizmo details: `extensions/dear-imguizmo-sys/README.md`.
- ImNodes details: `extensions/dear-imnodes-sys/README.md`.
- Node Editor details: `extensions/dear-node-editor-sys/README.md`.
- ImGui Test Engine details: `extensions/dear-imgui-test-engine-sys/README.md`.

## Submodules

Ensure third-party sources are available:

```
git submodule update --init --recursive
```

## Best Practices

Guidance on build scripts, bitflags vs enums, and data interop (mint/glam):

- `extensions/BEST_PRACTICES.md`

## Examples

Examples are in the top-level `examples/` crate and are feature-gated per extension:

- `implot_basic` -> `--features implot`
- `implot3d_basic` -> `--features implot3d`
- `imguizmo_basic` -> `--features imguizmo`
- `imnodes_basic` -> `--features imnodes`
- `node_editor_basic` -> `--features node-editor`
- `node_editor_showcase` -> `--features node-editor-blueprints` (native only)
- `reflect_demo` -> `--features reflect`
- `file_dialog_native` / `file_browser_imgui` -> `--features file-browser`
- `imgui_test_engine_basic` -> `--features test-engine`

Run:

```bash
cargo run -p dear-imgui-examples --bin implot_basic --features implot
cargo run -p dear-imgui-examples --bin implot3d_basic --features implot3d
cargo run -p dear-imgui-examples --bin imguizmo_basic --features imguizmo
cargo run -p dear-imgui-examples --bin imnodes_basic --features imnodes
cargo run -p dear-imgui-examples --bin node_editor_basic --features node-editor
cargo run -p dear-imgui-examples --bin node_editor_showcase --features node-editor-blueprints
cargo run -p dear-imgui-examples --bin reflect_demo --features reflect
cargo run -p dear-imgui-examples --bin imgui_test_engine_basic --features test-engine

# File Browser (new)
# Native dialog (rfd):
cargo run -p dear-imgui-examples --bin file_dialog_native --features file-browser
# ImGui browser (pure UI):
cargo run -p dear-imgui-examples --bin file_browser_imgui --features file-browser
```
