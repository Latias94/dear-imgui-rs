use super::super::Viewport;
use super::registry::{abort_if_panicked, load_cb};
use super::set::*;
use crate::sys;
use core::ffi::c_char;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

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

    let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
    if vp.is_null() {
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB) {
        abort_if_panicked(
            "Platform_GetWindowPos",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp, &mut pos) })),
        );
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_POS_CB) {
        abort_if_panicked(
            "Platform_GetWindowPos",
            catch_unwind(AssertUnwindSafe(|| unsafe {
                cb(vp as *mut Viewport, &mut pos)
            })),
        );
    }

    unsafe { *out_pos = pos };
}
pub unsafe extern "C" fn platform_get_window_size_out(
    vp: *mut sys::ImGuiViewport,
    out_size: *mut sys::ImVec2,
) {
    if out_size.is_null() {
        return;
    }

    let mut size = sys::ImVec2 { x: 0.0, y: 0.0 };
    if vp.is_null() {
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB) {
        abort_if_panicked(
            "Platform_GetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp, &mut size) })),
        );
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_SIZE_CB) {
        abort_if_panicked(
            "Platform_GetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe {
                cb(vp as *mut Viewport, &mut size)
            })),
        );
    }

    unsafe { *out_size = size };
}
pub unsafe extern "C" fn platform_get_window_framebuffer_scale_out(
    vp: *mut sys::ImGuiViewport,
    out_scale: *mut sys::ImVec2,
) {
    if out_scale.is_null() {
        return;
    }

    let mut scale = sys::ImVec2 { x: 1.0, y: 1.0 };
    if vp.is_null() {
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB) {
        abort_if_panicked(
            "Platform_GetWindowFramebufferScale",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp, &mut scale) })),
        );
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB) {
        abort_if_panicked(
            "Platform_GetWindowFramebufferScale",
            catch_unwind(AssertUnwindSafe(|| unsafe {
                cb(vp as *mut Viewport, &mut scale)
            })),
        );
    }

    unsafe { *out_scale = scale };
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
pub unsafe extern "C" fn platform_get_window_work_area_insets_out(
    vp: *mut sys::ImGuiViewport,
    out_insets: *mut sys::ImVec4,
) {
    if out_insets.is_null() {
        return;
    }

    let mut insets = sys::ImVec4::zero();
    if vp.is_null() {
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB) {
        abort_if_panicked(
            "Platform_GetWindowWorkAreaInsets",
            catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp, &mut insets) })),
        );
    } else if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB) {
        abort_if_panicked(
            "Platform_GetWindowWorkAreaInsets",
            catch_unwind(AssertUnwindSafe(|| unsafe {
                cb(vp as *mut Viewport, &mut insets)
            })),
        );
    }

    unsafe { *out_insets = insets };
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
