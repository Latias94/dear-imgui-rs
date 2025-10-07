# Dear ImGuizmo - Rust Bindings

[![Crates.io](https://img.shields.io/crates/v/dear-imguizmo.svg)](https://crates.io/crates/dear-imguizmo)
[![Documentation](https://docs.rs/dear-imguizmo/badge.svg)](https://docs.rs/dear-imguizmo)

High-level Rust bindings for [ImGuizmo](https://github.com/CedricGuillemet/ImGuizmo), built on the C API ([cimguizmo](https://github.com/cimgui/cimguizmo)) and integrated with `dear-imgui-rs`.

This project is a Rust wrapper around the C shim (cimguizmo), not a direct C++ binding.

<p align="center">
  <img src="https://github.com/user-attachments/assets/6fee92e9-d77a-4c47-aca9-9992243f26de" alt="ImGuizmo" width="75%"/>
  <br/>
</p>

## Links

- Upstream: https://github.com/CedricGuillemet/ImGuizmo
- C API: https://github.com/cimgui/cimguizmo

## Crate Layout

- `dear-imguizmo` (this crate): safe, idiomatic wrapper integrated with `dear-imgui-rs`.
- `dear-imguizmo-sys`: low-level FFI generated from the C API (`cimguizmo`). Prefer not using it directly unless you need raw bindings.

## Compatibility

| Item              | Version |
|-------------------|---------|
| Crate             | 0.4.x   |
| dear-imgui-rs     | 0.4.x   |
| dear-imguizmo-sys | 0.4.x   |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Features

- `glam` (default): Use `glam::Mat4` seamlessly in the high-level API.
- `mint` (optional): Use `mint::ColumnMatrix4<f32>` seamlessly.
- Without those features, you can always pass `[f32; 16]` (column-major) matrices.

All matrix arguments in the API are generic over a `Mat4Like` trait, implemented for `[f32; 16]`, and when enabled, for `glam::Mat4` and `mint::ColumnMatrix4<f32>`.

## Quick Start

```toml
[dependencies]
dear-imgui-rs = "0.4"
dear-imguizmo = "0.4"
```

Minimal usage (dear-imgui-style API):

```rust
use dear_imgui_rs::Context;
use dear_imguizmo::{Operation, Mode, GuizmoExt};
use glam::Mat4;

let mut ctx = Context::create();
let ui = ctx.frame();

// Begin ImGuizmo for this frame via Ui extension
let giz = ui.guizmo();

// Set the drawing rectangle to the full viewport
let ds = ui.io().display_size();
giz.set_rect(0.0, 0.0, ds[0], ds[1]);

// Choose where to draw (window/background/foreground)
giz.set_drawlist_window();

// Matrices (column-major). With the default `glam` feature, use `glam::Mat4`.
let mut model = Mat4::IDENTITY;
let view = Mat4::IDENTITY;
let proj = Mat4::IDENTITY;

// Manipulate the model matrix
let used: bool = giz
    .manipulate_config(&view, &proj, &mut model)
    .operation(Operation::TRANSLATE)
    .mode(Mode::Local)
    //.snap([1.0, 1.0, 1.0])
    //.bounds([minx,miny,minz], [maxx,maxy,maxz])
    .build();

if used { /* object moved */ }
```

See `examples/imguizmo_basic.rs` for a full demo with camera controls, snapping, bounds and helpers.

### Per-frame pattern

Typical usage order within a single frame:

```rust
let giz = ui.guizmo(); // call once per frame

// 1) Configure drawing area and destination
let ds = ui.io().display_size();
giz.set_rect(0.0, 0.0, ds[0], ds[1]);
giz.set_drawlist_window(); // or Background/Foreground

// 2) Manipulate
let used = giz
    .manipulate_config(&view, &proj, &mut model)
    .operation(Operation::UNIVERSAL)
    .mode(Mode::Local)
    .build();
```

Notes:
- Call `ui.guizmo()` exactly once per frame, then set `set_rect(...)` and a draw target before calling `manipulate`/`draw_*`.
- You can select draw target with `giz.set_drawlist(DrawListTarget::Window|Background|Foreground)`.
- Use RAII ID helpers: `let _id = giz.push_id(i);`.
- Style access: `let mut st = giz.style(); st.set_color(Color::Selection, [1.0,0.2,0.2,1.0]);`.
  More fields: `translation_line_arrow_size()`, `rotation_outer_line_thickness()`,
  `scale_line_circle_size()`, `hatched_axis_line_thickness()`, etc.
 - `view_manipulate`/`view_manipulate_with_camera` accept any `Into<[f32;2]>` for position/size; `set_rect_pos_size` is available as a convenience.
 - Matrices are column-major; `Mat4Like` supports `[f32;16]`, and (with features) `glam::Mat4`, `mint::ColumnMatrix4<f32>`.

Builder extras:
- `.drawlist(DrawListTarget::...)`, `.rect(x,y,w,h)`, `.orthographic(bool)`, `.gizmo_size_clip_space(f32)`
- `.axis_mask(AxisMask::X | AxisMask::Z)`
- `.translate_snap([f32;3])`, `.rotate_snap_deg(f32)`, `.scale_snap([f32;3])` (also accepts `(f32,f32,f32)`, `glam::Vec3`, `mint::Vector3<f32>`)

### Extra helpers

Simple utility methods you may find handy:

```rust
// Hover test near a world-space position (in pixels)
// Call after you've established view/projection for the frame
// via manipulate/draw_grid/draw_cubes.
let hovered = giz.is_over_at([0.0, 0.0, 0.0], 8.0);

// Compute an ImGuizmo hashed ID from a pointer
let id_from_ptr = giz.get_id_ptr(&model as *const _);
```

### Draw helpers

```rust
// Draw a grid aligned with a model transform (size in world units)
giz.draw_grid(&view, &proj, &model, 10.0);

// Draw multiple cubes from a list of model matrices
let matrices = vec![model, other_model];
giz.draw_cubes(&view, &proj, &matrices);
```

## Notes

- This crate depends on `dear-imguizmo-sys` (C API + bindgen). Linking to the base ImGui static library is provided by `dear-imgui-sys`; you do not need to configure it here.

### Graph Editor (pure Rust, optional)

We provide a minimal, pure-Rust graph editor aligned with dear-imgui style, under `dear_imguizmo::graph`. It aims to mirror ImGuizmo's GraphEditor UX without relying on the C++ API.

```rust
use dear_imguizmo::graph::{Graph, GraphView};
use dear_imguizmo::graph::GraphEditorExt; // Ui extension

let mut graph = Graph::new();
let mut view = GraphView::default();
// populate graph.nodes/links ...

ui.window("Graph")
  .size([600.0, 400.0], dear_imgui_rs::Condition::FirstUseEver)
  .build(|| {
      // Simple draw with defaults
      let _resp = ui.graph_editor().draw(&mut graph, &mut view);

      // Or configure style via builder
      let _resp = ui
          .graph_editor_config()
          .graph(&mut graph)
          .view(&mut view)
          .grid_visible(true)
          .grid_spacing(28.0)
          .link_thickness(2.0)
          .build();
  });
```

Current features:
- Grid (toggle visibility, custom spacing/color)
- Panning (MMB), node dragging (LMB)
- Selection (click, Ctrl multi-select, box select)
- Link creation (drag from output to input), link selection
- Link reconnection (drag near a link endpoint onto another pin)
- Deletion via Delete key or helper `delete_selected(&mut Graph, &mut GraphView)`
- Fit helpers: `fit_all_nodes(...)`, `fit_selected_nodes(...)`

Notes:
- Pin hover, node/link hover outlines are shown using `GraphStyle` colors.
- Simple 2D interop: `graph::Vec2Like` supports `(f32,f32)`, `[f32;2]`, `mint::Vector2<f32>`, and (with `glam`) `glam::Vec2`.
- This module is pure Rust and independent of the C++ GraphEditor; improvements are welcome.

