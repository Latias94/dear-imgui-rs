# Dear ImGui Extensions

Extensions in this workspace build on top of `dear-imgui-sys` (cimgui C API) and `dear-imgui-rs` to provide extra functionality like plotting, 3D gizmos, and reflection-based UI helpers.

Use `version = "0.23"` for the core and extensions. All 29 publishable packages share the `0.23.0` release. When building from Git, select the same revision for all `dear-*` crates. The native source links below identify the revisions used to build this release.

| Extension | Description | Status | Rust crate | Native sources or implementation |
| --- | --- | --- | --- | --- |
| ImPlot | Scientific plotting | Complete | [dear-implot](dear-implot/README.md) | [cimplot `c809ff29c8`](https://github.com/cimgui/cimplot/tree/c809ff29c84813fe1cdc8a0c069145ef2285f22a) / [ImPlot 1.2 WIP `72de27832d`](https://github.com/epezent/implot/tree/72de27832d1e6299bcc930ce01073d899d420392) |
| ImPlot3D | 3D scientific plotting | Complete | [dear-implot3d](dear-implot3d/README.md) | [cimplot3d `c7b527f318`](https://github.com/cimgui/cimplot3d/tree/c7b527f318c1e18cfe4c05b3cd666ce44175779e) / [ImPlot3D 0.6 WIP `c54525f8d4`](https://github.com/brenocq/implot3d/tree/c54525f8d46cece384cc2618761a2c55016b06db) |
| ImGuizmo | 3D transform gizmos | Complete | [dear-imguizmo](dear-imguizmo/README.md) | [cimguizmo `566ff53a4a`](https://github.com/cimgui/cimguizmo/tree/566ff53a4ae6ab148312ffc41eef7ba03b312560) / [ImGuizmo `105e4fde45`](https://github.com/CedricGuillemet/ImGuizmo/tree/105e4fde45e4e74aae8973d98750820ef7aca7e2) |
| ImGuIZMO.quat | Quaternion + 3D gizmo | Complete | [dear-imguizmo-quat](dear-imguizmo-quat/README.md) | [cimguizmo_quat `91499ef659`](https://github.com/cimgui/cimguizmo_quat/tree/91499ef65938d050c4a87150284e2b085db003dd) / [imGuIZMO.quat `df0927ea22`](https://github.com/sonoro1234/imGuIZMO.quat/tree/df0927ea22031b82f073304e15509fc2a868f228) |
| ImNodes | Node editor widgets | Complete | [dear-imnodes](dear-imnodes/README.md) | [cimnodes `712f574082`](https://github.com/cimgui/cimnodes/tree/712f574082945810dc2f79e022995c86b3b128b9) / [ImNodes `2d0c280b07`](https://github.com/Nelarius/imnodes/tree/2d0c280b07b2f62b120034811cc6e41251673961) |
| Node Editor | Native node editor + blueprints | Complete | [dear-node-editor](dear-node-editor/README.md) | [cimnodes_editor `917728f588`](https://github.com/cimgui/cimnodes_editor/tree/917728f58869efafb67386099a1c77e153284f49) / [imgui-node-editor `1e0e494f1a`](https://github.com/thedmd/imgui-node-editor/tree/1e0e494f1a86885eb332ca518543108ccd88d13e) |
| ImGuiColorTextEdit | Syntax-aware editing, text diff, and autocomplete | Preview | [dear-imgui-cte](dear-imgui-cte/README.md) | [cimCTE `dbf2abe484`](https://github.com/cimgui/cimCTE/tree/dbf2abe4841b9dd8a3f1f1864038ac9d60155c5e) / [ImGuiColorTextEdit `7515d1c765`](https://github.com/goossens/ImGuiColorTextEdit/tree/7515d1c765f8f9d47cd704fd3fbd20eb03fcc304) |
| ImGui Test Engine | UI automation and test runner | Preview | [dear-imgui-test-engine](dear-imgui-test-engine/README.md) | [Test Engine `10a5c08e78`](https://github.com/ocornut/imgui_test_engine/tree/10a5c08e78619274e3c943d69a5765ac2132b7a4) |
| File Browser | File dialogs + in-UI browser | Preview | [dear-file-browser](dear-file-browser/README.md) | Pure ImGui UI + rfd (native) |
| ImGui Reflect | Reflection-based UI from types | Preview | [dear-imgui-reflect](dear-imgui-reflect/README.md) | C++ ImReflect (reference only; pure Rust implementation) |

## Architecture

Most extensions use C bindings plus checked-in pregenerated bindgen output (no C++ bindgen):

```
Core:        dear-imgui-sys (cimgui C API)  ->  dear-imgui-rs (safe Rust)
Extensions:  dear-xxx-sys (C API + pregenerated bindings) ->  dear-xxx (safe Rust)
Pure-Rust:   dear-file-browser / dear-imgui-reflect build directly on dear-imgui-rs
```

Key points:
- `*-sys` crates bind to C APIs (cimgui/cimplot/cimguizmo/cimCTE) with committed bindgen output by default.
- ImPlot and ImPlot3D lend separate setup and drawing handles inside a plot. ImNodes uses `with_nodes` for its canvas, and Node Editor uses `with_node_editor`. These closures pair native calls in their original window and table cell.
- Linking of the base ImGui static library is unified by `dear-imgui-sys` -> extensions should not duplicate link flags for it.
- `dear-imgui-rs` draw APIs accept `Into<ImVec2>`, so arrays, tuples, `mint::Vector2<f32>`, and `ImVec2` all work out of the box.

## Build Modes

The core and seven prebuilt-capable extension sys crates follow the shared [0.22 native build policy](../docs/COMPATIBILITY.md#0220-native-build-policy). Test Engine is source-only. Build custom native archives and browser providers from the matching 0.23.0 sources. Each `-sys` README describes its artifact requirements.

- Source build (default): compile upstream C/C++ sources with `cc`.
- Matching prebuilt: select one compatible library directory, archive, or package directory. Explicit selectors are mutually exclusive and authoritative; missing or incompatible artifacts stop the build.
- Automatic prebuilt discovery: enable feature `prebuilt` to allow release lookup and cached acquisition. Automatic misses may fall back to source; strict `BUILD_MODE=prebuilt` never does.
- Bindings-only: select `BUILD_MODE=bindings-only` for declaration-only checks; it does not provide native symbols for runnable applications.

Environment variables:

- ImPlot: `IMPLOT_SYS_LIB_DIR`, `IMPLOT_SYS_PREBUILT_URL`, `IMPLOT_SYS_SKIP_CC`.
- ImPlot3D: `IMPLOT3D_SYS_LIB_DIR`, `IMPLOT3D_SYS_PREBUILT_URL`, `IMPLOT3D_SYS_SKIP_CC`.
- ImGuizmo: `IMGUIZMO_SYS_LIB_DIR`, `IMGUIZMO_SYS_PREBUILT_URL`, `IMGUIZMO_SYS_SKIP_CC`.
- ImNodes: `IMNODES_SYS_LIB_DIR`, `IMNODES_SYS_PREBUILT_URL`, `IMNODES_SYS_SKIP_CC`.
- Node Editor: `NODE_EDITOR_SYS_LIB_DIR`, `NODE_EDITOR_SYS_PREBUILT_URL`, `NODE_EDITOR_SYS_SKIP_CC`.
- ImGuIZMO.quat: `IMGUIZMO_QUAT_SYS_LIB_DIR`, `IMGUIZMO_QUAT_SYS_PREBUILT_URL`, `IMGUIZMO_QUAT_SYS_SKIP_CC`.
- ImGuiColorTextEdit: `CTE_SYS_LIB_DIR`, `CTE_SYS_PREBUILT_URL`, `CTE_SYS_SKIP_CC`.
- ImGui Test Engine: source build only (`IMGUI_TEST_ENGINE_SYS_SKIP_CC` supported for pregenerated-bindings check/docs workflows).

Each prebuilt-capable prefix also accepts `*_SYS_BUILD_MODE=auto|source|prebuilt|bindings-only` and `*_SYS_PACKAGE_DIR`. Legacy `*_SYS_SKIP_CC=1` alone is check-only, but with an artifact selector or feature `prebuilt` it requires a compatible prebuilt. Do not combine `BUILD_MODE` with enabled legacy `SKIP_CC` or `FORCE_BUILD` flags. Source requirements win in automatic mode; explicit prebuilt and bindings-only modes reject them. Test Engine retains its documented dual-skip check route instead of adding these prebuilt modes.

Optional toggles:

- Auto-download prebuilt archives: feature `prebuilt` (the env toggle `*_SYS_USE_PREBUILT=1` requires the feature)
- Force build from sources: feature `build-from-source` or `*_SYS_FORCE_BUILD=1`

The sys crates document their individual build routes.

- [ImPlot](dear-implot-sys/README.md)
- [ImPlot3D](dear-implot3d-sys/README.md)
- [ImGuizmo](dear-imguizmo-sys/README.md)
- [ImGuIZMO.quat](dear-imguizmo-quat-sys/README.md)
- [ImNodes](dear-imnodes-sys/README.md)
- [Node Editor](dear-node-editor-sys/README.md)
- [ImGuiColorTextEdit](dear-imgui-cte-sys/README.md)
- [ImGui Test Engine](dear-imgui-test-engine-sys/README.md)

## Submodules

Ensure third-party sources are available:

```
git submodule update --init --recursive
```

## Best Practices

For native scope ownership, build scripts, and math type interoperability, see the [extension authoring guide](BEST_PRACTICES.md).

## Examples

Examples are in the top-level `examples/` crate and are feature-gated per extension. Start
with a `*_minimal` target for a copy-ready integration, then use the matching `*_showcase`
target to explore the complete API surface:

- `implot_minimal` / `implot_showcase` -> `--features implot`
- `implot3d_minimal` / `implot3d_showcase` -> `--features implot3d`
- `imguizmo_minimal` / `imguizmo_showcase` -> `--features imguizmo`
- `imguizmo_quat_minimal` / `imguizmo_quat_showcase` -> `--features imguizmo-quat`
- `imnodes_minimal` / `imnodes_showcase` -> `--features imnodes`
- `node_editor_minimal` -> `--features node-editor`
- `node_editor_showcase` -> `--features node-editor-blueprints` (native only)
- `cte_minimal` / `cte_showcase` -> `--features cte`
- `reflect_demo` -> `--features reflect`
- `file_dialog_native` / `file_browser_imgui` -> `--features file-browser`
- `test_engine_integration` -> `--features test-engine`

Run:

```bash
cargo run -p dear-imgui-examples --bin implot_minimal --features implot
cargo run -p dear-imgui-examples --bin implot_showcase --features implot
cargo run -p dear-imgui-examples --bin implot3d_minimal --features implot3d
cargo run -p dear-imgui-examples --bin implot3d_showcase --features implot3d
cargo run -p dear-imgui-examples --bin imguizmo_minimal --features imguizmo
cargo run -p dear-imgui-examples --bin imguizmo_showcase --features imguizmo
cargo run -p dear-imgui-examples --bin imguizmo_quat_minimal --features imguizmo-quat
cargo run -p dear-imgui-examples --bin imguizmo_quat_showcase --features imguizmo-quat
cargo run -p dear-imgui-examples --bin imnodes_minimal --features imnodes
cargo run -p dear-imgui-examples --bin imnodes_showcase --features imnodes
cargo run -p dear-imgui-examples --bin node_editor_minimal --features node-editor
cargo run -p dear-imgui-examples --bin node_editor_showcase --features node-editor-blueprints
cargo run -p dear-imgui-examples --bin cte_minimal --features cte
cargo run -p dear-imgui-examples --bin cte_showcase --features cte
cargo run -p dear-imgui-examples --bin reflect_demo --features reflect
cargo run -p dear-imgui-examples --bin test_engine_integration --features test-engine

# File Browser (new)
# Native dialog (rfd):
cargo run -p dear-imgui-examples --bin file_dialog_native --features file-browser
# ImGui browser (pure UI):
cargo run -p dear-imgui-examples --bin file_browser_imgui --features file-browser
```
