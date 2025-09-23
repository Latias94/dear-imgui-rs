# Dear ImGuizmo - Rust Bindings

High-level Rust bindings for [ImGuizmo](https://github.com/CedricGuillemet/ImGuizmo), built on top of `cimguizmo` (C API) and integrated with `dear-imgui`.

## Quick Start

```
[dependencies]
dear-imgui = "0.1"
dear-imguizmo = { path = "path/to/extensions/dear-imguizmo" }
```

Basic usage:

```rust
use dear_imgui as imgui;
use dear_imguizmo as guizmo;

let mut ctx = imgui::Context::create_or_panic();
let ui = ctx.frame();

// Begin per-frame ImGuizmo drawing
guizmo::begin_frame(&ui);

// Configure the drawing rectangle to match the current window or viewport
let [x, y] = [0.0, 0.0];
let [w, h] = ui.io().display_size;
guizmo::set_rect(x, y, w, h);

// Manipulate a model matrix using the current camera view/projection
let mut model = [1.0_f32; 16];
let view = [1.0_f32; 16];
let proj = [1.0_f32; 16];
let used = guizmo::manipulate(
    &view, &proj,
    guizmo::Operation::Translate,
    guizmo::Mode::Local,
    &mut model,
    None,
);
```

See the crate docs for more helpers.
