# Dear ImGuizmo - Rust Bindings

High-level Rust bindings for [ImGuizmo](https://github.com/CedricGuillemet/ImGuizmo), built on top of the C API (`cimguizmo`) and integrated with `dear-imgui`.

## Features

- `glam` (default): Use `glam::Mat4` seamlessly in the high-level API.
- `mint` (optional): Use `mint::ColumnMatrix4<f32>` seamlessly.
- Without those features, you can always pass `[f32; 16]` (column-major) matrices.

All matrix arguments in the API are generic over a `Mat4Like` trait, implemented for `[f32; 16]`, and when enabled, for `glam::Mat4` and `mint::ColumnMatrix4<f32>`.

## Quick Start

```
[dependencies]
dear-imgui = "0.1"
dear-imguizmo = { path = "../../extensions/dear-imguizmo" }
```

Minimal usage:

```rust
use dear_imgui::Context;
use dear_imguizmo::{GuizmoContext, Operation, Mode};
use glam::Mat4;

let mut ctx = Context::create_or_panic();
let ui = ctx.frame();

// Begin ImGuizmo for this frame, bound to the current ImGui context
let gizmo_ui = GuizmoContext::new().begin_frame(&ui);

// Set the drawing rectangle to the full viewport
let ds = ui.io().display_size();
gizmo_ui.set_rect(0.0, 0.0, ds[0], ds[1]);

// Choose where to draw (window/background/foreground)
gizmo_ui.set_drawlist_window();

// Matrices (column-major). With the default `glam` feature, use `glam::Mat4`.
let mut model = Mat4::IDENTITY;
let view = Mat4::IDENTITY;
let proj = Mat4::IDENTITY;

// Manipulate the model matrix
let used: bool = gizmo_ui
    .manipulate(
        &view,
        &proj,
        Operation::TRANSLATE,
        Mode::Local,
        &mut model,
        None,
        None,
        None,
        None,
    )
    .unwrap_or(false);

if used { /* object moved */ }
```

See `examples/imguizmo_basic.rs` for a full demo with camera controls, snapping, bounds and helpers.

## Notes

- This crate depends on `dear-imguizmo-sys` (C API + bindgen). Linking to the base ImGui static library is provided by `dear-imgui-sys`; you do not need to configure it here.
