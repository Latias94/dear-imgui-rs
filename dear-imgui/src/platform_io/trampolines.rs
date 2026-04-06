use super::Viewport;
use crate::sys;
use core::ffi::c_char;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

// Platform callbacks
pub static PLATFORM_CREATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static PLATFORM_DESTROY_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static PLATFORM_SHOW_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static PLATFORM_SET_WINDOW_POS_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
> = Mutex::new(None);
pub static PLATFORM_GET_WINDOW_POS_RAW_CB: Mutex<
    Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2>,
> = Mutex::new(None);
pub static PLATFORM_GET_WINDOW_POS_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
> = Mutex::new(None);
pub static PLATFORM_SET_WINDOW_SIZE_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
> = Mutex::new(None);
pub static PLATFORM_GET_WINDOW_SIZE_RAW_CB: Mutex<
    Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2>,
> = Mutex::new(None);
pub static PLATFORM_GET_WINDOW_SIZE_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
> = Mutex::new(None);
pub static PLATFORM_SET_WINDOW_FOCUS_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static PLATFORM_GET_WINDOW_DPI_SCALE_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport) -> f32>,
> = Mutex::new(None);
pub static PLATFORM_GET_WINDOW_FOCUS_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
> = Mutex::new(None);
pub static PLATFORM_GET_WINDOW_MINIMIZED_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
> = Mutex::new(None);
pub static PLATFORM_ON_CHANGED_VIEWPORT_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static PLATFORM_SET_WINDOW_TITLE_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, *const c_char)>,
> = Mutex::new(None);
pub static PLATFORM_SET_WINDOW_ALPHA_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport, f32)>> =
    Mutex::new(None);
pub static PLATFORM_UPDATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static PLATFORM_RENDER_WINDOW_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
> = Mutex::new(None);
pub static PLATFORM_SWAP_BUFFERS_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
> = Mutex::new(None);

// Renderer callbacks
pub static RENDERER_CREATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static RENDERER_DESTROY_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
    Mutex::new(None);
pub static RENDERER_SET_WINDOW_SIZE_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
> = Mutex::new(None);
pub static RENDERER_RENDER_WINDOW_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
> = Mutex::new(None);
pub static RENDERER_SWAP_BUFFERS_CB: Mutex<
    Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
> = Mutex::new(None);

#[inline]
fn load_cb<T: Copy>(m: &Mutex<T>) -> T {
    match m.lock() {
        Ok(g) => *g,
        Err(poison) => *poison.into_inner(),
    }
}

#[inline]
pub(super) fn store_cb<T: Copy>(m: &Mutex<Option<T>>, cb: Option<T>) {
    let mut g = match m.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    };
    *g = cb;
}

#[inline]
fn abort_if_panicked<T>(ctx: &str, res: Result<T, Box<dyn std::any::Any + Send>>) -> T {
    match res {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-imgui-rs: panic in PlatformIO callback ({ctx})");
            std::process::abort();
        }
    }
}

// Trampolines for platform callbacks
pub unsafe extern "C" fn platform_create_window(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_CREATE_WINDOW_CB) {
        abort_if_panicked(
            "Platform_CreateWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn platform_destroy_window(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_DESTROY_WINDOW_CB) {
        abort_if_panicked(
            "Platform_DestroyWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn platform_show_window(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SHOW_WINDOW_CB) {
        abort_if_panicked(
            "Platform_ShowWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn platform_set_window_pos(vp: *mut sys::ImGuiViewport, p: sys::ImVec2) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_POS_CB) {
        abort_if_panicked(
            "Platform_SetWindowPos",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, p) })),
        );
    }
}
pub unsafe extern "C" fn platform_set_window_size(vp: *mut sys::ImGuiViewport, s: sys::ImVec2) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_SIZE_CB) {
        abort_if_panicked(
            "Platform_SetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, s) })),
        );
    }
}
pub unsafe extern "C" fn platform_get_window_pos_out(
    vp: *mut sys::ImGuiViewport,
    out_pos: *mut sys::ImVec2,
) {
    if out_pos.is_null() {
        return;
    }

    let pos = if vp.is_null() {
        sys::ImVec2 { x: 0.0, y: 0.0 }
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB) {
        abort_if_panicked(
            "Platform_GetWindowPos",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp) })),
        )
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_POS_CB) {
        abort_if_panicked(
            "Platform_GetWindowPos",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        )
    } else {
        sys::ImVec2 { x: 0.0, y: 0.0 }
    };

    unsafe { *out_pos = pos };
}
pub unsafe extern "C" fn platform_get_window_size_out(
    vp: *mut sys::ImGuiViewport,
    out_size: *mut sys::ImVec2,
) {
    if out_size.is_null() {
        return;
    }

    let size = if vp.is_null() {
        sys::ImVec2 { x: 0.0, y: 0.0 }
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB) {
        abort_if_panicked(
            "Platform_GetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp) })),
        )
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_SIZE_CB) {
        abort_if_panicked(
            "Platform_GetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        )
    } else {
        sys::ImVec2 { x: 0.0, y: 0.0 }
    };

    unsafe { *out_size = size };
}
pub unsafe extern "C" fn platform_set_window_focus(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_FOCUS_CB) {
        abort_if_panicked(
            "Platform_SetWindowFocus",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn platform_get_window_focus(vp: *mut sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }
    if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_FOCUS_CB) {
        return abort_if_panicked(
            "Platform_GetWindowFocus",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
    false
}
pub unsafe extern "C" fn platform_get_window_dpi_scale(vp: *mut sys::ImGuiViewport) -> f32 {
    if vp.is_null() {
        return 0.0;
    }
    if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_DPI_SCALE_CB) {
        return abort_if_panicked(
            "Platform_GetWindowDpiScale",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
    0.0
}
pub unsafe extern "C" fn platform_get_window_minimized(vp: *mut sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }
    if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_MINIMIZED_CB) {
        return abort_if_panicked(
            "Platform_GetWindowMinimized",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
    false
}
pub unsafe extern "C" fn platform_on_changed_viewport(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_ON_CHANGED_VIEWPORT_CB) {
        abort_if_panicked(
            "Platform_OnChangedViewport",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn platform_set_window_title(
    vp: *mut sys::ImGuiViewport,
    title: *const c_char,
) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_TITLE_CB) {
        abort_if_panicked(
            "Platform_SetWindowTitle",
            catch_unwind(AssertUnwindSafe(|| unsafe {
                cb(vp as *mut Viewport, title)
            })),
        );
    }
}
pub unsafe extern "C" fn platform_set_window_alpha(vp: *mut sys::ImGuiViewport, a: f32) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_ALPHA_CB) {
        abort_if_panicked(
            "Platform_SetWindowAlpha",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, a) })),
        );
    }
}
pub unsafe extern "C" fn platform_update_window(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_UPDATE_WINDOW_CB) {
        abort_if_panicked(
            "Platform_UpdateWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn platform_render_window(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_RENDER_WINDOW_CB) {
        abort_if_panicked(
            "Platform_RenderWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
        );
    }
}
pub unsafe extern "C" fn platform_swap_buffers(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&PLATFORM_SWAP_BUFFERS_CB) {
        abort_if_panicked(
            "Platform_SwapBuffers",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
        );
    }
}

// Trampolines for renderer callbacks
pub unsafe extern "C" fn renderer_create_window(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&RENDERER_CREATE_WINDOW_CB) {
        abort_if_panicked(
            "Renderer_CreateWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn renderer_destroy_window(vp: *mut sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&RENDERER_DESTROY_WINDOW_CB) {
        abort_if_panicked(
            "Renderer_DestroyWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
        );
    }
}
pub unsafe extern "C" fn renderer_set_window_size(vp: *mut sys::ImGuiViewport, s: sys::ImVec2) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&RENDERER_SET_WINDOW_SIZE_CB) {
        abort_if_panicked(
            "Renderer_SetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, s) })),
        );
    }
}
pub unsafe extern "C" fn renderer_render_window(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&RENDERER_RENDER_WINDOW_CB) {
        abort_if_panicked(
            "Renderer_RenderWindow",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
        );
    }
}
pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
    if vp.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&RENDERER_SWAP_BUFFERS_CB) {
        abort_if_panicked(
            "Renderer_SwapBuffers",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
        );
    }
}
