# Dear ImGuizmo - Rust Bindings

High-level Rust bindings for [ImGuizmo](https://github.com/CedricGuillemet/ImGuizmo), built on top of the C API (`cimguizmo`) and integrated with `dear-imgui`.

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

// Build per-frame ImGuizmo UI bound to the current ImGui context
let gizmo_ui = GuizmoContext::new().get_ui(&ui);

// Set the drawing rectangle to the full viewport
let [w, h] = ui.io().display_size;
gizmo_ui.set_rect(0.0, 0.0, w, h);

// Matrices (column-major arrays)
let mut model = Mat4::IDENTITY;
let view = Mat4::IDENTITY;
let proj = Mat4::IDENTITY;

// Manipulate the model matrix
let used = gizmo_ui
    .manipulate_with_options(
        &ui.get_window_draw_list(),
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
