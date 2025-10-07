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
| Crate                  | 0.4.x   |
| dear-imgui-rs          | 0.4.x   |
| dear-imguizmo-quat-sys | 0.4.x   |

See also: docs/COMPATIBILITY.md in the workspace for the full matrix.

## Features

- glam (default): pass and mutate `glam::Quat`, `glam::Vec3`, `glam::Vec4` directly
- mint (optional): interop via `mint::{Quaternion, Vector3, Vector4}`
- freetype: passthrough to `dear-imgui-sys/freetype` (enable from any crate once)

All math parameters are generic over lightweight traits so you can also use plain arrays: `[f32; 4]` for quaternions, `[f32; 3|4]` for vectors.

## Quick Start

```toml
[dependencies]
dear-imgui-rs = "0.4"
dear-imguizmo-quat = "0.4"
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
