use std::cell::Cell;
use std::ffi::c_void;
use std::sync::Arc;

use winit::window::Window;

use super::WinitPlatformError;
use super::registry::{insert_viewport_data, owns_viewport_data};
use super::runtime::RuntimeControl;
use crate::sanitize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WindowEventEcho {
    targets: [[i64; 2]; 2],
    target_count: u8,
    ignore_until_frame: i32,
}

impl WindowEventEcho {
    pub(super) const fn new(target: [i64; 2], ignore_until_frame: i32) -> Self {
        Self {
            targets: [target, target],
            target_count: 1,
            ignore_until_frame,
        }
    }

    pub(super) const fn with_alternate(mut self, target: [i64; 2]) -> Self {
        self.targets[1] = target;
        self.target_count = 2;
        self
    }
}

pub(super) fn classify_window_event_echo(
    current_frame: i32,
    pending: Option<WindowEventEcho>,
    actual: [i64; 2],
) -> (bool, Option<WindowEventEcho>) {
    let Some(pending) = pending else {
        return (true, None);
    };
    let matches_target = pending.targets[..usize::from(pending.target_count)]
        .iter()
        .any(|target| {
            target
                .iter()
                .copied()
                .zip(actual)
                .all(|(expected, actual)| expected.abs_diff(actual) <= 1)
        });
    if current_frame <= pending.ignore_until_frame && matches_target {
        (false, Some(pending))
    } else {
        (true, None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewportWindowPolicy {
    pub(super) decorations: bool,
    pub(super) top_most: bool,
    pub(super) skip_taskbar: bool,
    pub(super) cursor_hittest: bool,
    pub(super) no_focus_on_appearing: bool,
    pub(super) no_focus_on_click: bool,
}

impl Default for ViewportWindowPolicy {
    fn default() -> Self {
        Self {
            decorations: true,
            top_most: false,
            skip_taskbar: false,
            cursor_hittest: true,
            no_focus_on_appearing: false,
            no_focus_on_click: false,
        }
    }
}

impl ViewportWindowPolicy {
    pub(super) fn from_flags(flags: dear_imgui_rs::sys::ImGuiViewportFlags) -> Self {
        Self {
            decorations: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoDecoration == 0,
            top_most: flags & dear_imgui_rs::sys::ImGuiViewportFlags_TopMost != 0,
            skip_taskbar: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoTaskBarIcon != 0,
            cursor_hittest: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoInputs == 0,
            no_focus_on_appearing: flags
                & dear_imgui_rs::sys::ImGuiViewportFlags_NoFocusOnAppearing
                != 0,
            no_focus_on_click: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoFocusOnClick != 0,
        }
    }
}

/// Runtime-owned sidecar stored in `ImGuiViewport::PlatformUserData`.
#[repr(C)]
pub(super) struct ViewportData {
    window: Arc<Window>,
    main: bool,
    pub(super) pending_window_pos_echo: Cell<Option<WindowEventEcho>>,
    pub(super) pending_window_size_echo: Cell<Option<WindowEventEcho>>,
    pub(super) window_policy: Cell<ViewportWindowPolicy>,
    pub(super) last_log_fb_scale: Cell<f32>,
}

impl ViewportData {
    pub(super) fn new(window: Arc<Window>, main: bool) -> Self {
        Self {
            window,
            main,
            pending_window_pos_echo: Cell::new(None),
            pending_window_size_echo: Cell::new(None),
            window_policy: Cell::new(ViewportWindowPolicy::default()),
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
    let scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
    let base = window
        .inner_position()
        .ok()
        .map(|position| position.to_logical::<f64>(scale));
    client_to_screen_logical(logical, base.map(|base| [base.x, base.y]))
}

fn client_to_screen_logical(
    logical: [f64; 2],
    screen_origin: Option<[f64; 2]>,
) -> Option<[f32; 2]> {
    let logical = sanitize::finite_vec2_f64_to_f32(logical)?;
    let screen_origin = screen_origin?;
    sanitize::finite_vec2_f64_to_f32([
        screen_origin[0] + f64::from(logical[0]),
        screen_origin[1] + f64::from(logical[1]),
    ])
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
        if viewport.is_null() {
            Err(WinitPlatformError::ContextMismatch)
        } else if (*viewport).PlatformUserData.is_null()
            && (*viewport).PlatformHandle.is_null()
            && (*viewport).PlatformHandleRaw.is_null()
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
            || !(*viewport).PlatformHandleRaw.is_null()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_coordinates_add_the_window_screen_origin() {
        assert_eq!(
            client_to_screen_logical([12.5, 7.0], Some([300.0, 200.0])),
            Some([312.5, 207.0])
        );
        assert_eq!(client_to_screen_logical([12.5, 7.0], None), None);
    }

    #[test]
    fn matching_geometry_echo_is_suppressed_but_user_input_is_reported() {
        let pending = WindowEventEcho::new([400, 240], 11);

        assert_eq!(
            classify_window_event_echo(10, Some(pending), [400, 240]),
            (false, Some(pending))
        );
        assert_eq!(
            classify_window_event_echo(10, Some(pending), [425, 240]),
            (true, None)
        );
        assert_eq!(
            classify_window_event_echo(12, Some(pending), [400, 240]),
            (true, None)
        );
    }
}
