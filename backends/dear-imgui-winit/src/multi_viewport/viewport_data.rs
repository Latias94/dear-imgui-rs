use super::*;
use crate::sanitize;
use std::ffi::c_void;

/// Helper structure stored in the void* PlatformUserData field of each ImGuiViewport
/// to easily retrieve our backend data. Following official ImGui backend pattern.
#[repr(C)]
pub(super) struct ViewportData {
    pub window: *mut Window, // Stored in ImGuiViewport::PlatformHandle
    pub window_owned: bool,  // Set to false for main window
    pub ignore_window_pos_event_frame: i32,
    pub ignore_window_size_event_frame: i32,
    // Last framebuffer scale we logged for this viewport (debug only).
    pub last_log_fb_scale: f32,
}

impl Default for ViewportData {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportData {
    pub(super) fn new() -> Self {
        Self {
            window: std::ptr::null_mut(),
            window_owned: false,
            ignore_window_pos_event_frame: -1,
            ignore_window_size_event_frame: -1,
            last_log_fb_scale: 0.0,
        }
    }
}

// Convert client-area logical coordinates to screen coordinates (logical), per-window
pub(crate) fn client_to_screen_pos(window: &Window, logical: [f64; 2]) -> Option<[f32; 2]> {
    let logical = sanitize::finite_vec2_f64_to_f32(logical)?;
    let scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
    // Cross-platform: absolute screen = client top-left (logical) + client offset (logical)
    let base = window
        .inner_position()
        .ok()
        .map(|p| p.to_logical::<f64>(scale))
        .or_else(|| {
            window
                .outer_position()
                .ok()
                .map(|p| p.to_logical::<f64>(scale))
        });
    if let Some(base) = base {
        sanitize::finite_vec2_f64_to_f32([base.x + logical[0] as f64, base.y + logical[1] as f64])
    } else {
        Some(logical)
    }
}

/// Compute the decoration offset in logical pixels: inner_position - outer_position.
///
/// This lets us translate between ImGui's platform coordinate space (client origin)
/// and winit's outer-position APIs. Returns None if either position is unavailable.
pub(super) fn decoration_offset_logical(window: &Window) -> Option<(f64, f64)> {
    let scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
    let inner_phys = window.inner_position().ok()?;
    let outer_phys = window.outer_position().ok()?;
    let inner_log = inner_phys.to_logical::<f64>(scale);
    let outer_log = outer_phys.to_logical::<f64>(scale);
    sanitize::finite_vec2_f64_to_f32([inner_log.x - outer_log.x, inner_log.y - outer_log.y])?;
    Some((inner_log.x - outer_log.x, inner_log.y - outer_log.y))
}

pub(super) fn init_main_viewport(ctx: &mut Context, main_window: &Window) {
    let _context_guard = unsafe { CurrentContextGuard::bind(ctx.as_raw()) };

    unsafe {
        let main_viewport = dear_imgui_rs::sys::igGetMainViewport();
        if main_viewport.is_null() {
            return;
        }

        let existing = (*main_viewport).PlatformUserData as *mut ViewportData;
        let vd = if existing.is_null() {
            let vd = Box::into_raw(Box::new(ViewportData::new()));
            register_viewport_data(vd);
            vd
        } else if is_winit_viewport_data(existing) {
            existing
        } else {
            panic!("main viewport PlatformUserData is already owned by another platform backend");
        };

        (*vd).window = main_window as *const Window as *mut Window;
        (*vd).window_owned = false; // Main window is owned by the application

        (*main_viewport).PlatformUserData = vd as *mut c_void;
        (*main_viewport).PlatformHandle = main_window as *const Window as *mut c_void;
    }
}

pub(super) unsafe fn clear_main_viewport_data_for_current_context() {
    unsafe {
        let main_viewport = dear_imgui_rs::sys::igGetMainViewport();
        if !main_viewport.is_null() {
            let vp = &mut *main_viewport;
            let vd_ptr = vp.PlatformUserData as *mut ViewportData;
            let owned_by_winit = is_winit_viewport_data(vd_ptr);
            if owned_by_winit {
                // Main window not owned by us: do not free the window itself
                (*vd_ptr).window = std::ptr::null_mut();
                drop_viewport_data(vd_ptr);
                vp.PlatformUserData = std::ptr::null_mut();
            }
            // Clear handle to avoid dangling pointer
            if vd_ptr.is_null() || owned_by_winit {
                vp.PlatformHandle = std::ptr::null_mut();
            }
        }
    }
}
