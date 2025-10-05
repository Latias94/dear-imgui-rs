ImGuIZMO.quat – Quaternion/3D gizmo helpers for Dear ImGui (safe Rust bindings).

This crate wraps the C API from cimgui/cimguizmo_quat via `dear-imguizmo-quat-sys` and integrates with `dear-imgui`.

Quick sketch (builder):

```
let ui: &dear_imgui_rs::Ui = ...;
let mut rot = glam::Quat::IDENTITY; // or [f32; 4]
let used = ui
    .gizmo_quat()
    .builder()
    .size(220.0)
    .mode(dear_imguizmo_quat::Mode::MODE_DUAL | dear_imguizmo_quat::Mode::CUBE_AT_ORIGIN)
    .quat("##rot", &mut rot);
```
