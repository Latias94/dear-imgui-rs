# Dear ImGui Extensions

Extensions in this workspace build on top of `dear-imgui-sys` (cimgui C API) to provide extra functionality like plotting and 3D gizmos.

| Extension | Description            | Status    | Rust Crate                       | Upstream C API                                      |
|-----------|------------------------|-----------|----------------------------------|-----------------------------------------------------|
| ImPlot    | Scientific plotting    | Complete  | [dear-implot](./dear-implot)     | [cimgui/cimplot](https://github.com/cimgui/cimplot) |
| ImGuizmo  | 3D transform gizmos    | Complete  | [dear-imguizmo](./dear-imguizmo) | [cimgui/cimguizmo](https://github.com/cimgui/cimguizmo) |

## Architecture

All extensions use C bindings + bindgen (no C++ bindgen):

```
Core:        dear-imgui-sys (cimgui C API)  ->  dear-imgui (safe Rust)
Extensions:  dear-xxx-sys (C API + bindgen) ->  dear-xxx (safe Rust)
```

Key points:
- `*-sys` crates bind to C APIs (cimgui/cimplot/cimguizmo) with bindgen.
- High-level crates wrap C APIs with RAII tokens and builder-style ergonomics.
- `dear-imgui-sys` exports include paths and defines via `DEP_DEAR_IMGUI_*`; extensions inherit them for consistent builds.
- Linking of the base ImGui static library is unified by `dear-imgui-sys` — extensions should not duplicate link flags for it.

## Build Modes

Each `*-sys` crate supports multiple ways to obtain its own native static library (see each `-sys` README for details):

- Source build (default): compile upstream C/C++ sources with `cc`.
- System/prebuilt: set a directory env var so Cargo can find the static lib.
- Remote prebuilt: set a direct URL; the file is downloaded into `OUT_DIR/prebuilt/`.

Environment variables:

- ImPlot: `IMPLOT_SYS_LIB_DIR`, `IMPLOT_SYS_PREBUILT_URL`, `IMPLOT_SYS_SKIP_CC`.
- ImGuizmo: `IMGUIZMO_SYS_LIB_DIR`, `IMGUIZMO_SYS_PREBUILT_URL`, `IMGUIZMO_SYS_SKIP_CC`.

See also:
- ImPlot details: `extensions/dear-implot-sys/README.md`.
- ImGuizmo details: `extensions/dear-imguizmo-sys/README.md`.

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

- `implot_basic` → `--features dear-implot`
- `imguizmo_basic` → `--features dear-imguizmo`

Run:

```
cargo run --bin implot_basic --features dear-implot
cargo run --bin imguizmo_basic --features dear-imguizmo
```

### New: ImNodes

ImNodes (node editor widgets) is integrated similarly:

- Crates: `extensions/dear-imnodes-sys` (C API via cimnodes) and `extensions/dear-imnodes` (safe API)
- Example: `imnodes_basic` behind `--features dear-imnodes`

Run:

```
cargo run --bin imnodes_basic --features dear-imnodes
```
