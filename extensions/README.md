# Dear ImGui Extensions

Extensions in this workspace build on top of `dear-imgui-sys` (cimgui C API) to provide extra functionality like plotting and 3D gizmos.

| Extension | Description            | Status    | Rust Crate                       | Upstream C API                                      |
|-----------|------------------------|-----------|----------------------------------|-----------------------------------------------------|
| ImPlot    | Scientific plotting    | Complete  | [dear-implot](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-implot)     | [cimgui/cimplot](https://github.com/cimgui/cimplot) |
| ImGuizmo  | 3D transform gizmos    | Complete  | [dear-imguizmo](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imguizmo) | [cimgui/cimguizmo](https://github.com/cimgui/cimguizmo) |
| ImNodes   | Node editor widgets    | Complete  | [dear-imnodes](https://github.com/Latias94/dear-imgui-rs/tree/main/extensions/dear-imnodes)   | [cimgui/cimnodes](https://github.com/cimgui/cimnodes)  |

## Architecture

All extensions use C bindings + bindgen (no C++ bindgen):

```
Core:        dear-imgui-sys (cimgui C API)  ->  dear-imgui (safe Rust)
Extensions:  dear-xxx-sys (C API + bindgen) ->  dear-xxx (safe Rust)
```

Key points:
- `*-sys` crates bind to C APIs (cimgui/cimplot/cimguizmo) with bindgen.
- High-level crates wrap C APIs with RAII tokens and builder-style ergonomics.
- Linking of the base ImGui static library is unified by `dear-imgui-sys` -> extensions should not duplicate link flags for it.
 - dear-imgui draw APIs accept `Into<ImVec2>`, so arrays, tuples, `mint::Vector2<f32>`, and `ImVec2` all work out of the box.

## Build Modes

Each `*-sys` crate supports multiple ways to obtain its own native static library (see each `-sys` README for details):

- Source build (default): compile upstream C/C++ sources with `cc`.
- System/prebuilt: set a directory env var so Cargo can find the static lib.
- Remote prebuilt: set a direct URL; the file is downloaded into `OUT_DIR/prebuilt/`.

Environment variables:

- ImPlot: `IMPLOT_SYS_LIB_DIR`, `IMPLOT_SYS_PREBUILT_URL`, `IMPLOT_SYS_SKIP_CC`.
- ImGuizmo: `IMGUIZMO_SYS_LIB_DIR`, `IMGUIZMO_SYS_PREBUILT_URL`, `IMGUIZMO_SYS_SKIP_CC`.

- ImNodes: `IMNODES_SYS_LIB_DIR`, `IMNODES_SYS_PREBUILT_URL`, `IMNODES_SYS_SKIP_CC`.

Optional toggles:

- Auto-download prebuilt archives: feature `prebuilt` or `*_SYS_USE_PREBUILT=1`
- Force build from sources: feature `build-from-source` or `*_SYS_FORCE_BUILD=1`

See also:
- ImPlot details: `extensions/dear-implot-sys/README.md`.
- ImGuizmo details: `extensions/dear-imguizmo-sys/README.md`.
- ImNodes details: `extensions/dear-imnodes-sys/README.md`.

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

- `implot_basic` -> `--features dear-implot`
- `imguizmo_basic` -> `--features dear-imguizmo`

 - `imnodes_basic` -> `--features dear-imnodes`

Run:

```
cargo run --bin implot_basic --features dear-implot
cargo run --bin imguizmo_basic --features dear-imguizmo
cargo run --bin imnodes_basic --features dear-imnodes
```

 


