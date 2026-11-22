#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use dear_imgui_rs::{Context, sys};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
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
    /// coordinate space. An attached single window applies the new scale at its next size or DPI
    /// event, or during [`Self::prepare_frame`].
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
        self.control.hidpi_factor.get()
    }

    pub(super) fn update_single_window_geometry(
        &self,
        context: &mut Context,
        size: PhysicalSize<u32>,
        hidpi: f64,
    ) {
        let previous = self.control.hidpi_factor.replace(hidpi);
        if previous != hidpi {
            reproject_local_mouse(
                context,
                previous,
                hidpi,
                self.control.last_local_pointer.get(),
            );
        }
        let io = context.io_mut();
        io.set_display_size(sanitize::finite_non_negative_size(size.to_logical(hidpi)));
        io.set_display_framebuffer_scale(sanitize::framebuffer_scale(hidpi, 1.0));
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
                self.control.hidpi_factor.set(hidpi);

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

        if !runtime_owns_desktop_cursor {
            let hidpi = self.hidpi_factor_for_window(window);
            if hidpi != self.hidpi_factor() {
                self.update_single_window_geometry(imgui_ctx, window.inner_size(), hidpi);
            }
        }

        // If backend supports setting mouse pos and ImGui requests it, honor it. Winit cannot set
        // a global desktop pointer, so a live multi-viewport runtime intentionally skips this.
        if imgui_ctx.io().want_set_mouse_pos()
            && !runtime_owns_desktop_cursor
            && !self.control.pointer_controls_window(window.id())
        {
            let pos = imgui_ctx.io().mouse_pos();
            let logical_pos = self
                .scale_pos_for_winit(window, LogicalPosition::new(pos[0] as f64, pos[1] as f64));
            if let Some(pos) = sanitize::finite_position(logical_pos) {
                let position = LogicalPosition::new(pos[0], pos[1]);
                if window.set_cursor_position(position).is_ok() {
                    self.control.last_local_pointer.set(Some(
                        position
                            .to_physical(sanitize::positive_finite_or(window.scale_factor(), 1.0)),
                    ));
                }
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
                .to_logical(self.hidpi_factor()),
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
                .to_logical(self.hidpi_factor()),
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
                .to_physical::<f64>(self.hidpi_factor())
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

fn reproject_local_mouse(
    context: &mut Context,
    old_scale: f64,
    new_scale: f64,
    last_physical: Option<PhysicalPosition<f64>>,
) {
    // Dear ImGui floors queued positions. Preserve the physical precision of the last Winit
    // sample, including a stationary cursor when the new scale reveals a fractional pixel.
    let last = last_physical.and_then(|physical| {
        let at_scale = |scale| {
            sanitize::finite_position(physical.to_logical(scale)).map(|p| p.map(f32::floor))
        };
        Some((at_scale(old_scale)?, at_scale(new_scale)?))
    });
    let reproject = |x: &mut f32, y: &mut f32| {
        if *x == -f32::MAX || *y == -f32::MAX {
            return;
        }
        let pos = [*x, *y];
        [*x, *y] = if let Some((old, new)) = last
            && pos == old
        {
            new
        } else {
            sanitize::finite_vec2_f64_to_f32(pos.map(|v| f64::from(v) * old_scale / new_scale))
                .map(|p| p.map(f32::floor))
                .unwrap_or([-f32::MAX; 2])
        };
    };
    // SAFETY: the exclusively borrowed Context owns IO and its queue. Only MousePos events
    // initialize this union member. No native calls or queue reallocations occur here.
    unsafe {
        let raw = &mut *context.as_raw();
        let io = &mut raw.IO;
        for pos in [&mut io.MousePos, &mut io.MousePosPrev]
            .into_iter()
            .chain(&mut io.MouseClickedPos)
        {
            reproject(&mut pos.x, &mut pos.y);
        }
        // A button can make Dear ImGui defer the next move to another frame. Each earlier
        // position must be in the current display space when its associated click is consumed.
        let queue = dear_imgui_rs::internal::imvector_cast_mut::<sys::ImGuiInputEvent, _>(
            &mut raw.InputEventsQueue,
        );
        for event in queue.as_slice_mut() {
            if event.Type == sys::ImGuiInputEventType_MousePos {
                let pos = &mut event.__bindgen_anon_1.MousePos;
                reproject(&mut pos.PosX, &mut pos.PosY);
            }
        }
    }
}
