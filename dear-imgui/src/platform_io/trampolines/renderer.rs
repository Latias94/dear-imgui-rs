use super::super::Viewport;
use super::registry::{abort_if_panicked, load_cb};
use super::set::*;
use crate::sys;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

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
/// Forwards Dear ImGui's renderer-window size callback into Rust.
///
/// # Safety
///
/// `vp` must identify a viewport owned by the active Dear ImGui context. `size` must be non-null,
/// properly aligned, and point to an `ImVec2` that remains readable for the duration of this call.
/// The repository-owned C++ thunk satisfies this contract by passing the address of its
/// callback-scoped aggregate value.
pub unsafe extern "C" fn renderer_set_window_size(
    vp: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    if vp.is_null() || size.is_null() {
        return;
    }
    if let Some(cb) = load_cb(&RENDERER_SET_WINDOW_SIZE_CB) {
        // SAFETY: The null check above and the C++ thunk contract guarantee that `size` is aligned
        // and readable for this callback only. Copying the value prevents Rust from retaining it.
        let size = unsafe { *size };
        abort_if_panicked(
            "Renderer_SetWindowSize",
            catch_unwind(AssertUnwindSafe(|| unsafe {
                cb(vp as *mut Viewport, size)
            })),
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
