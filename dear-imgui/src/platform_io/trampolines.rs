use super::Viewport;
use crate::sys;
use core::ffi::c_char;
use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

type ViewportCb = unsafe extern "C" fn(*mut Viewport);
type ViewportVec2Cb = unsafe extern "C" fn(*mut Viewport, sys::ImVec2);
type RawViewportVec2RetCb = unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2;
type ViewportVec2RetCb = unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2;
type ViewportF32RetCb = unsafe extern "C" fn(*mut Viewport) -> f32;
type ViewportBoolRetCb = unsafe extern "C" fn(*mut Viewport) -> bool;
type ViewportTitleCb = unsafe extern "C" fn(*mut Viewport, *const c_char);
type ViewportF32Cb = unsafe extern "C" fn(*mut Viewport, f32);
type ViewportRenderCb = unsafe extern "C" fn(*mut Viewport, *mut c_void);

#[derive(Clone, Copy, Default)]
struct CallbackSet {
    platform_create_window: Option<ViewportCb>,
    platform_destroy_window: Option<ViewportCb>,
    platform_show_window: Option<ViewportCb>,
    platform_set_window_pos: Option<ViewportVec2Cb>,
    platform_get_window_pos_raw: Option<RawViewportVec2RetCb>,
    platform_get_window_pos: Option<ViewportVec2RetCb>,
    platform_set_window_size: Option<ViewportVec2Cb>,
    platform_get_window_size_raw: Option<RawViewportVec2RetCb>,
    platform_get_window_size: Option<ViewportVec2RetCb>,
    platform_set_window_focus: Option<ViewportCb>,
    platform_get_window_dpi_scale: Option<ViewportF32RetCb>,
    platform_get_window_focus: Option<ViewportBoolRetCb>,
    platform_get_window_minimized: Option<ViewportBoolRetCb>,
    platform_on_changed_viewport: Option<ViewportCb>,
    platform_set_window_title: Option<ViewportTitleCb>,
    platform_set_window_alpha: Option<ViewportF32Cb>,
    platform_update_window: Option<ViewportCb>,
    platform_render_window: Option<ViewportRenderCb>,
    platform_swap_buffers: Option<ViewportRenderCb>,
    renderer_create_window: Option<ViewportCb>,
    renderer_destroy_window: Option<ViewportCb>,
    renderer_set_window_size: Option<ViewportVec2Cb>,
    renderer_render_window: Option<ViewportRenderCb>,
    renderer_swap_buffers: Option<ViewportRenderCb>,
}

struct ContextCallbacks {
    ctx: *mut sys::ImGuiContext,
    callbacks: CallbackSet,
}

pub(super) struct CallbackSlot<T: Copy> {
    get: fn(&CallbackSet) -> Option<T>,
    set: fn(&mut CallbackSet, Option<T>),
}

macro_rules! callback_slot {
    ($name:ident, $field:ident, $ty:ty, $getter:ident, $setter:ident) => {
        fn $getter(callbacks: &CallbackSet) -> Option<$ty> {
            callbacks.$field
        }

        fn $setter(callbacks: &mut CallbackSet, callback: Option<$ty>) {
            callbacks.$field = callback;
        }

        pub(super) const $name: CallbackSlot<$ty> = CallbackSlot {
            get: $getter,
            set: $setter,
        };
    };
}

callback_slot!(
    PLATFORM_CREATE_WINDOW_CB,
    platform_create_window,
    ViewportCb,
    get_platform_create_window,
    set_platform_create_window
);
callback_slot!(
    PLATFORM_DESTROY_WINDOW_CB,
    platform_destroy_window,
    ViewportCb,
    get_platform_destroy_window,
    set_platform_destroy_window
);
callback_slot!(
    PLATFORM_SHOW_WINDOW_CB,
    platform_show_window,
    ViewportCb,
    get_platform_show_window,
    set_platform_show_window
);
callback_slot!(
    PLATFORM_SET_WINDOW_POS_CB,
    platform_set_window_pos,
    ViewportVec2Cb,
    get_platform_set_window_pos,
    set_platform_set_window_pos
);
callback_slot!(
    PLATFORM_GET_WINDOW_POS_RAW_CB,
    platform_get_window_pos_raw,
    RawViewportVec2RetCb,
    get_platform_get_window_pos_raw,
    set_platform_get_window_pos_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_POS_CB,
    platform_get_window_pos,
    ViewportVec2RetCb,
    get_platform_get_window_pos,
    set_platform_get_window_pos
);
callback_slot!(
    PLATFORM_SET_WINDOW_SIZE_CB,
    platform_set_window_size,
    ViewportVec2Cb,
    get_platform_set_window_size,
    set_platform_set_window_size
);
callback_slot!(
    PLATFORM_GET_WINDOW_SIZE_RAW_CB,
    platform_get_window_size_raw,
    RawViewportVec2RetCb,
    get_platform_get_window_size_raw,
    set_platform_get_window_size_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_SIZE_CB,
    platform_get_window_size,
    ViewportVec2RetCb,
    get_platform_get_window_size,
    set_platform_get_window_size
);
callback_slot!(
    PLATFORM_SET_WINDOW_FOCUS_CB,
    platform_set_window_focus,
    ViewportCb,
    get_platform_set_window_focus,
    set_platform_set_window_focus
);
callback_slot!(
    PLATFORM_GET_WINDOW_DPI_SCALE_CB,
    platform_get_window_dpi_scale,
    ViewportF32RetCb,
    get_platform_get_window_dpi_scale,
    set_platform_get_window_dpi_scale
);
callback_slot!(
    PLATFORM_GET_WINDOW_FOCUS_CB,
    platform_get_window_focus,
    ViewportBoolRetCb,
    get_platform_get_window_focus,
    set_platform_get_window_focus
);
callback_slot!(
    PLATFORM_GET_WINDOW_MINIMIZED_CB,
    platform_get_window_minimized,
    ViewportBoolRetCb,
    get_platform_get_window_minimized,
    set_platform_get_window_minimized
);
callback_slot!(
    PLATFORM_ON_CHANGED_VIEWPORT_CB,
    platform_on_changed_viewport,
    ViewportCb,
    get_platform_on_changed_viewport,
    set_platform_on_changed_viewport
);
callback_slot!(
    PLATFORM_SET_WINDOW_TITLE_CB,
    platform_set_window_title,
    ViewportTitleCb,
    get_platform_set_window_title,
    set_platform_set_window_title
);
callback_slot!(
    PLATFORM_SET_WINDOW_ALPHA_CB,
    platform_set_window_alpha,
    ViewportF32Cb,
    get_platform_set_window_alpha,
    set_platform_set_window_alpha
);
callback_slot!(
    PLATFORM_UPDATE_WINDOW_CB,
    platform_update_window,
    ViewportCb,
    get_platform_update_window,
    set_platform_update_window
);
callback_slot!(
    PLATFORM_RENDER_WINDOW_CB,
    platform_render_window,
    ViewportRenderCb,
    get_platform_render_window,
    set_platform_render_window
);
callback_slot!(
    PLATFORM_SWAP_BUFFERS_CB,
    platform_swap_buffers,
    ViewportRenderCb,
    get_platform_swap_buffers,
    set_platform_swap_buffers
);
callback_slot!(
    RENDERER_CREATE_WINDOW_CB,
    renderer_create_window,
    ViewportCb,
    get_renderer_create_window,
    set_renderer_create_window
);
callback_slot!(
    RENDERER_DESTROY_WINDOW_CB,
    renderer_destroy_window,
    ViewportCb,
    get_renderer_destroy_window,
    set_renderer_destroy_window
);
callback_slot!(
    RENDERER_SET_WINDOW_SIZE_CB,
    renderer_set_window_size,
    ViewportVec2Cb,
    get_renderer_set_window_size,
    set_renderer_set_window_size
);
callback_slot!(
    RENDERER_RENDER_WINDOW_CB,
    renderer_render_window,
    ViewportRenderCb,
    get_renderer_render_window,
    set_renderer_render_window
);
callback_slot!(
    RENDERER_SWAP_BUFFERS_CB,
    renderer_swap_buffers,
    ViewportRenderCb,
    get_renderer_swap_buffers,
    set_renderer_swap_buffers
);

thread_local! {
    static CONTEXT_CALLBACKS: RefCell<Vec<ContextCallbacks>> = RefCell::new(Vec::new());
}

#[inline]
fn current_context() -> *mut sys::ImGuiContext {
    unsafe { sys::igGetCurrentContext() }
}

#[inline]
fn load_cb<T: Copy>(slot: &CallbackSlot<T>) -> Option<T> {
    let ctx = current_context();
    if ctx.is_null() {
        return None;
    }
    CONTEXT_CALLBACKS.with(|contexts| {
        contexts
            .borrow()
            .iter()
            .find(|entry| entry.ctx == ctx)
            .and_then(|entry| (slot.get)(&entry.callbacks))
    })
}

#[inline]
pub(super) fn store_cb<T: Copy>(slot: &CallbackSlot<T>, cb: Option<T>) {
    let ctx = current_context();
    assert!(
        !ctx.is_null(),
        "PlatformIo typed callbacks require an active ImGui context"
    );

    CONTEXT_CALLBACKS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        if let Some(entry) = contexts.iter_mut().find(|entry| entry.ctx == ctx) {
            (slot.set)(&mut entry.callbacks, cb);
            return;
        }

        let mut callbacks = CallbackSet::default();
        (slot.set)(&mut callbacks, cb);
        contexts.push(ContextCallbacks { ctx, callbacks });
    });
}

pub(crate) fn clear_callbacks_for_context(ctx: *mut sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }
    CONTEXT_CALLBACKS.with(|contexts| {
        contexts.borrow_mut().retain(|entry| entry.ctx != ctx);
    });
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
