//! Keep UI sizes in SDL window coordinates and backing pixels in the platform backend.

use dear_imgui_rs::{ConfigFlags, Context, Style};
use sdl3::video::Window;

pub struct DpiStyle {
    baseline: Style,
    scale: f32,
}

impl DpiStyle {
    /// Capture the application theme before applying the window's content scale.
    pub fn new(context: &mut Context, window: &Window) -> Self {
        let mut style = Self {
            baseline: context.style().clone(),
            scale: 0.0,
        };
        style.update(context, window);
        style
    }

    /// Call after the platform's `new_frame`, before opening an ImGui frame.
    pub fn update(&mut self, context: &mut Context, window: &Window) {
        // Only native multi-viewport has monitor positions suitable for core DPI selection.
        // In single-window mode the main viewport sits at (0, 0), so retain our window DPI.
        let viewports = context
            .io()
            .config_flags()
            .contains(ConfigFlags::VIEWPORTS_ENABLE);
        context.io_mut().set_config_dpi_scale_fonts(viewports);
        context.io_mut().set_config_dpi_scale_viewports(viewports);
        // SDL's framebuffer scale already accounts for Retina/Wayland pixel density.
        let scale = window.display_scale() / window.pixel_density();
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        if scale != self.scale {
            // Restore the unscaled theme to avoid accumulating rounding on monitor changes.
            *context.style_mut() = self.baseline.clone();
            context.style_mut().scale_all_sizes(scale);
            context.style_mut().set_font_scale_dpi(scale);
            self.scale = scale;
        }
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }
}
