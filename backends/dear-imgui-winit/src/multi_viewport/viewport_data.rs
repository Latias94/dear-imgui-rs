use std::cell::Cell;
use std::ffi::c_void;
use std::sync::Arc;

use winit::window::Window;

use super::WinitPlatformError;
use super::registry::{insert_viewport_data, owns_viewport_data};
use super::runtime::RuntimeControl;
use crate::sanitize;

/// Runtime-owned sidecar stored in `ImGuiViewport::PlatformUserData`.
#[repr(C)]
pub(super) struct ViewportData {
    window: Arc<Window>,
    main: bool,
    pub(super) ignore_window_pos_event_frame: Cell<i32>,
    pub(super) ignore_window_size_event_frame: Cell<i32>,
    pub(super) last_log_fb_scale: Cell<f32>,
}

impl ViewportData {
    pub(super) fn new(window: Arc<Window>, main: bool) -> Self {
        Self {
            window,
            main,
            ignore_window_pos_event_frame: Cell::new(-1),
            ignore_window_size_event_frame: Cell::new(-1),
            last_log_fb_scale: Cell::new(0.0),
        }
    }

    pub(super) fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub(super) fn window_ptr(&self) -> *const Window {
        Arc::as_ptr(&self.window)
    }

    pub(super) fn is_main(&self) -> bool {
        self.main
    }
}

// Convert client-area logical coordinates to screen coordinates (logical), per-window.
pub(crate) fn client_to_screen_pos(window: &Window, logical: [f64; 2]) -> Option<[f32; 2]> {
    let logical = sanitize::finite_vec2_f64_to_f32(logical)?;
    let scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
    let base = window
        .inner_position()
        .ok()
        .map(|position| position.to_logical::<f64>(scale))
        .or_else(|| {
            window
                .outer_position()
                .ok()
                .map(|position| position.to_logical::<f64>(scale))
        });
    if let Some(base) = base {
        sanitize::finite_vec2_f64_to_f32([base.x + logical[0] as f64, base.y + logical[1] as f64])
    } else {
        Some(logical)
    }
}

/// Compute `inner_position - outer_position` in logical pixels.
pub(super) fn decoration_offset_logical(window: &Window) -> Option<(f64, f64)> {
    let scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
    let inner = window.inner_position().ok()?.to_logical::<f64>(scale);
    let outer = window.outer_position().ok()?.to_logical::<f64>(scale);
    sanitize::finite_vec2_f64_to_f32([inner.x - outer.x, inner.y - outer.y])?;
    Some((inner.x - outer.x, inner.y - outer.y))
}

pub(super) fn preflight_main_viewport(
    context: &dear_imgui_rs::Context,
) -> Result<(), WinitPlatformError> {
    let binding = context.binding();
    binding.with_bound_context(|| unsafe {
        let viewport = dear_imgui_rs::sys::igGetMainViewport();
        if viewport.is_null()
            || ((*viewport).PlatformUserData.is_null() && (*viewport).PlatformHandle.is_null())
        {
            Ok(())
        } else {
            Err(WinitPlatformError::ForeignPlatformUserData)
        }
    })
}

pub(super) fn init_main_viewport(
    control: &RuntimeControl,
    main_window: Arc<Window>,
) -> Result<(), WinitPlatformError> {
    control.binding().try_with_bound_context(|| unsafe {
        let viewport = dear_imgui_rs::sys::igGetMainViewport();
        if viewport.is_null()
            || !(*viewport).PlatformUserData.is_null()
            || !(*viewport).PlatformHandle.is_null()
        {
            return Err(WinitPlatformError::ForeignPlatformUserData);
        }

        let data = insert_viewport_data(
            control,
            viewport,
            ViewportData::new(Arc::clone(&main_window), true),
        )?;
        (*viewport).PlatformUserData = data.cast::<c_void>();
        (*viewport).PlatformHandle = Arc::as_ptr(&main_window).cast_mut().cast();
        Ok(())
    })?
}

pub(super) fn viewport_data_is_owned(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    if viewport.is_null() {
        return false;
    }
    // SAFETY: callers invoke this only for a live viewport in the current Context.
    let data = unsafe { (*viewport).PlatformUserData.cast::<ViewportData>() };
    owns_viewport_data(control, viewport, data)
}
