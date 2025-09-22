//! Gradient editor (ImGradient) - simplified Rust port

use dear_imgui::{StyleVar, Ui};

/// Delegate trait mirroring ImGradient::Delegate
pub trait GradientDelegate {
    /// Return number of control points
    fn get_point_count(&self) -> usize;
    /// Return slice of points as [r,g,b,position] where position in [0,1]
    fn get_points(&self) -> &[[f32; 4]];
    /// Edit point value; return true if modified
    fn edit_point(&mut self, index: usize, value: [f32; 4]) -> bool;
    /// Sample color at t in [0,1]
    fn get_point(&self, t: f32) -> [f32; 4];
    /// Add a new point
    fn add_point(&mut self, value: [f32; 4]);
}

/// Edit a gradient inside a child window of given `size` (width, height)
pub fn edit(
    ui: &Ui,
    delegate: &mut dyn GradientDelegate,
    size: [f32; 2],
    selection: &mut i32,
) -> bool {
    let mut modified = false;
    let _style = ui.push_style_var(StyleVar::FramePadding([0.0, 0.0]));
    ui.child_window("ImGradient")
        .size(size)
        .border(true)
        .build(ui, || {
            let draw_list = ui.get_window_draw_list();
            let offset = ui.cursor_screen_pos();

            let points = delegate.get_points();
            static mut MOVING_PT: i32 = -1;

            // Draw segments
            if points.len() > 0 {
                let w = size[0];
                let h = size[1];
                let steps = 64.0f32;
                let step = 1.0 / steps;
                let mut t = 0.0f32;
                let mut prev = delegate.get_point(0.0);
                while t <= 1.0 {
                    let next_t = (t + step).min(1.0);
                    let next = delegate.get_point(next_t);
                    let x0 = offset[0] + t * w;
                    let x1 = offset[0] + next_t * w;
                    // Fill small rect
                    let c = lerp_color(prev, next, 0.5);
                    draw_list
                        .add_rect([x0, offset[1]], [x1, offset[1] + h], rgba_to_u32(c))
                        .filled(true)
                        .build();
                    prev = next;
                    t = next_t;
                }
            }

            // Draw points
            for (i, p) in points.iter().enumerate() {
                let pos = [offset[0] + p[3].clamp(0.0, 1.0) * size[0], offset[1] + 3.0];
                let sz = 12.0;
                let rc_min = [pos[0] - sz * 0.5, pos[1]];
                let rc_max = [pos[0] + sz * 0.5, pos[1] + sz];
                let hovered = point_in_rect(ui.io().mouse_pos(), rc_min, rc_max);
                let active = unsafe { MOVING_PT == i as i32 };
                let col = if active {
                    0xFFFFFFFF
                } else if hovered {
                    0xAAFFFFFF
                } else {
                    0x77FFFFFF
                };
                draw_list.add_rect(rc_min, rc_max, col).filled(true).build();
                draw_list.add_rect(rc_min, rc_max, 0xFF000000).build();

                if hovered && ui.is_mouse_clicked(dear_imgui::MouseButton::Left) {
                    *selection = i as i32;
                    modified = true;
                    unsafe {
                        MOVING_PT = i as i32;
                    }
                }
            }

            // Move selected
            if *selection >= 0 {
                let sel = *selection as usize;
                if ui.is_mouse_down(dear_imgui::MouseButton::Left) {
                    let mouse = ui.io().mouse_pos();
                    let t = ((mouse[0] - offset[0]) / size[0]).clamp(0.0, 1.0);
                    let mut val = delegate.get_points()[sel];
                    val[3] = t;
                    modified |= delegate.edit_point(sel, val);
                } else {
                    unsafe {
                        MOVING_PT = -1;
                    }
                }
            }

            // Double-click to add
            let mouse = ui.io().mouse_pos();
            if ui.is_mouse_double_clicked(dear_imgui::MouseButton::Left)
                && mouse[0] >= offset[0]
                && mouse[0] <= offset[0] + size[0]
                && mouse[1] >= offset[1]
                && mouse[1] <= offset[1] + size[1]
            {
                let t = ((mouse[0] - offset[0]) / size[0]).clamp(0.0, 1.0);
                delegate.add_point(delegate.get_point(t));
                modified = true;
            }
        });
    modified
}

fn point_in_rect(mouse: [f32; 2], min: [f32; 2], max: [f32; 2]) -> bool {
    mouse[0] >= min[0] && mouse[0] <= max[0] && mouse[1] >= min[1] && mouse[1] <= max[1]
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn rgba_to_u32(c: [f32; 4]) -> u32 {
    let r = (c[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (c[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (c[2].clamp(0.0, 1.0) * 255.0) as u32;
    let a = (c[3].clamp(0.0, 1.0) * 255.0) as u32;
    (a << 24) | (b << 16) | (g << 8) | r
}
