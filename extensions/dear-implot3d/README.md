# dear-implot3d

High-level Rust bindings for ImPlot3D, integrating with `dear-imgui-rs`.

This crate sits on top of `dear-implot3d-sys` (FFI to `cimplot3d`) and mirrors
the ergonomics of `dear-implot`.

## Quick start

```rust
use dear_imgui_rs as imgui;
use dear_implot3d as implot3d;

let mut ctx = imgui::Context::create();
let plot3d = implot3d::Plot3DContext::create(&ctx);

let ui = ctx.frame();
let plot_ui = plot3d.get_plot_ui(&ui);

if let Some(_token) = plot_ui.begin_plot("3D Demo").size([600.0, 400.0]).build() {
    let xs = [0.0, 1.0, 2.0];
    let ys = [0.0, 1.0, 0.0];
    let zs = [0.0, 0.5, 1.0];
    plot_ui.plot_line_f32("line", &xs, &ys, &zs, implot3d::Line3DFlags::NONE);
}
```

## Features

- Context management and Ui facade
- Builder for `begin_plot()` with flags and size
- Line/Scatter helpers (f32/f64) + optional `mint` inputs
- Style helpers (push/pop colors/vars) and presets

## Notes

- The underlying C API comes from `cimplot3d` which depends on `implot3d`.
  Ensure git submodules are initialized with `--recursive`.

