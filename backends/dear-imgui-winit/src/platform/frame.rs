#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use dear_imgui_rs::Context;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{Window, WindowAttributes};

use crate::sanitize;

use super::WinitPlatformError;
use super::ownership::{WinitPlatform, WinitPlatformControl};

/// DPI scaling mode for the platform
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum HiDpiMode {
    /// Use the default DPI scaling
    #[default]
    Default,
    /// Use a custom scale factor
    Locked(f64),
    /// Round the scale factor to the nearest integer
    Rounded,
}

impl WinitPlatformControl {
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn refresh_runtime_state(
        &self,
        context: &mut Context,
    ) -> Result<(), WinitPlatformError> {
        let runtime = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .cloned();
        let Some(runtime) = runtime else {
            return Ok(());
        };
        runtime.reconcile_geometry_state();
        runtime.reconcile_input_state(context);
        let result = runtime.refresh_monitors(context);
        #[cfg(target_os = "windows")]
        let result = result.and_then(|()| runtime.refresh_native_mouse(context));
        if let Err(error) = result {
            self.fail_current_contract(error.clone());
            return Err(error);
        }
        Ok(())
    }
}

impl WinitPlatform {
    /// Set the DPI scaling mode.
    ///
    /// The mode is part of the primary-window coordinate mapping, so it cannot change while a
    /// multi-viewport runtime is attached. Secondary windows always use Winit's native desktop
    /// coordinate space.
    pub fn set_hidpi_mode(&mut self, hidpi_mode: HiDpiMode) -> Result<(), WinitPlatformError> {
        self.ensure_runtime_configuration_mutable()?;
        self.hidpi_mode = hidpi_mode;
        Ok(())
    }

    /// Return the configured DPI scaling mode.
    pub fn hidpi_mode(&self) -> HiDpiMode {
        self.hidpi_mode
    }

    /// Get the current DPI scaling factor
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    /// Update frame timing and platform state before calling [`Context::frame`].
    pub fn prepare_frame(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
    ) -> Result<(), WinitPlatformError> {
        self.control.validate_entry(imgui_ctx, window)?;
        let now = Instant::now();
        let delta = now - self.last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.last_frame = now;

        imgui_ctx.io_mut().set_delta_time(delta_s);

        // Keep the main viewport's native desktop coordinate unit and framebuffer relation in
        // sync while an owning multi-viewport runtime is attached.
        #[cfg(feature = "multi-viewport")]
        {
            if self.control.has_live_runtime() {
                self.control.refresh_runtime_state(imgui_ctx)?;
                let winit_scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
                let hidpi = self.hidpi_factor_for_scale(winit_scale);
                self.hidpi_factor = hidpi;

                let io = imgui_ctx.io_mut();
                io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                io.set_display_framebuffer_scale(
                    crate::multi_viewport::framebuffer_scale_for_window(window),
                );
            }
        }

        #[cfg(feature = "multi-viewport")]
        let runtime_owns_desktop_cursor = self.control.has_live_runtime();
        #[cfg(not(feature = "multi-viewport"))]
        let runtime_owns_desktop_cursor = false;

        // If backend supports setting mouse pos and ImGui requests it, honor it. Winit cannot set
        // a global desktop pointer, so a live multi-viewport runtime intentionally skips this.
        if imgui_ctx.io().want_set_mouse_pos() && !runtime_owns_desktop_cursor {
            let pos = imgui_ctx.io().mouse_pos();
            let logical_pos = self
                .scale_pos_for_winit(window, LogicalPosition::new(pos[0] as f64, pos[1] as f64));
            if let Some(pos) = sanitize::finite_position(logical_pos) {
                let _ = window.set_cursor_position(LogicalPosition::new(pos[0], pos[1]));
            }
        }
        // Cursor and IME state depend on the completed UI and are updated by `prepare_render`.
        Ok(())
    }

    /// Scale a logical size from winit to our active HiDPI mode
    pub fn scale_size_from_winit(
        &self,
        window: &Window,
        logical_size: LogicalSize<f64>,
    ) -> LogicalSize<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_size,
            // Convert to physical using winit scale, then back to logical with our factor
            _ => logical_size
                .to_physical::<f64>(sanitize::positive_finite_or(window.scale_factor(), 1.0))
                .to_logical(sanitize::positive_finite_or(self.hidpi_factor, 1.0)),
        }
    }

    /// Scale a logical position from winit to our active HiDPI mode
    pub fn scale_pos_from_winit(
        &self,
        window: &Window,
        logical_pos: LogicalPosition<f64>,
    ) -> LogicalPosition<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_pos,
            _ => logical_pos
                .to_physical::<f64>(sanitize::positive_finite_or(window.scale_factor(), 1.0))
                .to_logical(sanitize::positive_finite_or(self.hidpi_factor, 1.0)),
        }
    }

    /// Scale a logical position for winit based on our active HiDPI mode
    pub fn scale_pos_for_winit(
        &self,
        window: &Window,
        logical_pos: LogicalPosition<f64>,
    ) -> LogicalPosition<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_pos,
            _ => logical_pos
                .to_physical::<f64>(sanitize::positive_finite_or(self.hidpi_factor, 1.0))
                .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0)),
        }
    }

    pub(super) fn hidpi_factor_for_window(&self, window: &Window) -> f64 {
        self.hidpi_factor_for_scale(window.scale_factor())
    }

    pub(super) fn hidpi_factor_for_scale(&self, scale_factor: f64) -> f64 {
        let scale_factor = sanitize::positive_finite_or(scale_factor, 1.0);
        match self.hidpi_mode {
            HiDpiMode::Default => scale_factor,
            HiDpiMode::Locked(factor) => sanitize::positive_finite_or(factor, 1.0),
            HiDpiMode::Rounded => sanitize::positive_finite_or(scale_factor.round(), 1.0),
        }
    }

    /// Create window attributes with Dear ImGui defaults
    pub fn create_window_attributes() -> WindowAttributes {
        WindowAttributes::default()
            .with_title("Dear ImGui Window")
            .with_inner_size(LogicalSize::new(1024.0, 768.0))
    }
}
