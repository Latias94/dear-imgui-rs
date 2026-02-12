# Dear ImGuIZMO.quat – Rust Bindings

[![Crates.io](https://img.shields.io/crates/v/dear-imguizmo-quat.svg)](https://crates.io/crates/dear-imguizmo-quat)
[![Documentation](https://docs.rs/dear-imguizmo-quat/badge.svg)](https://docs.rs/dear-imguizmo-quat)

Safe, idiomatic Rust bindings for ImGuIZMO.quat (quaternion + 3D gizmo helpers) built on the C API wrapper `cimguizmo_quat`, integrated with `dear-imgui-rs`.

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imguizmo-quat-basic.png" alt="ImGuIZMO.quat" width="75%"/>
  <br/>
</p>

## Links

- Upstream: https://github.com/BrutPitt/imGuIZMO.quat
- C API: https://github.com/cimgui/cimguizmo_quat

## Compatibility

| Item                   | Version |
|------------------------|---------|
| Crate                  | 0.9.x   |
| dear-imgui-rs          | 0.9.x   |
| dear-imguizmo-quat-sys | 0.9.x   |

See also: docs/COMPATIBILITY.md in the workspace for the full matrix.

### WASM (WebAssembly) support

This crate has **experimental** support for `wasm32-unknown-unknown` targets via the same import-style design used by the core ImGui bindings:

- `dear-imguizmo-quat` + `dear-imguizmo-quat-sys` expose a `wasm` feature which:
  - Enables import-style FFI that links against the shared `imgui-sys-v0` provider module.
  - Avoids compiling C/C++ during the Rust build for wasm.
- The provider module (`imgui-sys-v0`) is built once using Emscripten and contains:
  - Dear ImGui + cimgui (from `dear-imgui-sys`)
  - ImGuizmo + cimguizmo (from `dear-imguizmo-sys`)
  - ImGuIZMO.quat + cimguizmo_quat (from `dear-imguizmo-quat-sys`)

To try the web demo with ImGuIZMO.quat enabled:

```bash
# 1) Generate pregenerated wasm bindings (Dear ImGui core + ImGuIZMO.quat)
cargo run -p xtask -- wasm-bindgen imgui-sys-v0
cargo run -p xtask -- wasm-bindgen-imguizmo-quat imgui-sys-v0

# 2) Build the main wasm + JS (includes an "ImGuIZMO.quat (Web)" window)
cargo run -p xtask -- web-demo imguizmo-quat

# 3) Build the provider (Emscripten imgui-sys-v0 with ImGui + ImGuIZMO.quat)
cargo run -p xtask -- build-cimgui-provider

# 4) Serve and open in a browser
python -m http.server -d target/web-demo 8080
```

Notes:
- The `dear-imgui-web-demo` crate in `examples-wasm` can enable the `imguizmo-quat` feature; when present, an “ImGuIZMO.quat (Web)” window is shown if bindings + provider are available.
- This is an early, experimental path; API and build steps may evolve in future releases. For production use, pin to a specific `0.6.x` release and follow changes in `docs/WASM.md`.

## Features

- glam (default): pass and mutate `glam::Quat`, `glam::Vec3`, `glam::Vec4` directly
- mint (optional): interop via `mint::{Quaternion, Vector3, Vector4}`
- freetype: passthrough to `dear-imgui-sys/freetype` (enable from any crate once)

All math parameters are generic over lightweight traits so you can also use plain arrays: `[f32; 4]` for quaternions, `[f32; 3|4]` for vectors.

## Quick Start

```toml
[dependencies]
dear-imgui-rs = "0.9"
dear-imguizmo-quat = "0.9"
```

Minimal usage with the Ui extension and builder API:

```rust
use dear_imgui_rs::Ui;
use dear_imguizmo_quat::{GizmoQuatExt, Mode};

fn draw(ui: &Ui) {
    let mut rot = glam::Quat::IDENTITY; // or [f32; 4]

    let used = ui
        .gizmo_quat()
        .builder()
        .size(220.0)
        .mode(Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN)
        .quat("##rot", &mut rot);

    if used {
        // rotation changed this frame
    }
}
```

Pan+Dolly + quaternion + light direction variant:

```rust
use dear_imguizmo_quat::{GizmoQuatExt, Mode};

let mut pan_dolly = glam::Vec3::ZERO;
let mut rot = glam::Quat::IDENTITY;
let mut light_dir = glam::Vec3::new(1.0, 0.0, 0.0);

ui.gizmo_quat()
    .builder()
    .mode(Mode::MODE_PAN_DOLLY | Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN)
    .pan_dolly_quat_light_vec3("##gizmo", &mut pan_dolly, &mut rot, &mut light_dir);
```

## Notes

- Begin once per frame with `ui.gizmo_quat()` and call into the builder; no extra context is required.
- RAII and ID usage follow dear-imgui patterns; use unique labels or `##suffix` to avoid ID clashes in loops.
- See `examples/imguizmo_quat_basic.rs` for a complete WGPU demo (cube + view controls).
