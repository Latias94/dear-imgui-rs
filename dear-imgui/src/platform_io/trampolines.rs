use super::Viewport;
use crate::sys;
use core::ffi::c_char;
use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

type ViewportCb = unsafe extern "C" fn(*mut Viewport);
type ViewportVec2Cb = unsafe extern "C" fn(*mut Viewport, sys::ImVec2);
type RawViewportVec2OutCb = unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2);
type ViewportVec2OutCb = unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec2);
type RawViewportVec4OutCb = unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4);
type ViewportVec4OutCb = unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec4);
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
    platform_get_window_pos_raw: Option<RawViewportVec2OutCb>,
    platform_get_window_pos: Option<ViewportVec2OutCb>,
    platform_set_window_size: Option<ViewportVec2Cb>,
    platform_get_window_size_raw: Option<RawViewportVec2OutCb>,
    platform_get_window_size: Option<ViewportVec2OutCb>,
    platform_get_window_framebuffer_scale_raw: Option<RawViewportVec2OutCb>,
    platform_get_window_framebuffer_scale: Option<ViewportVec2OutCb>,
    platform_set_window_focus: Option<ViewportCb>,
    platform_get_window_dpi_scale: Option<ViewportF32RetCb>,
    platform_get_window_focus: Option<ViewportBoolRetCb>,
    platform_get_window_minimized: Option<ViewportBoolRetCb>,
    platform_on_changed_viewport: Option<ViewportCb>,
    platform_get_window_work_area_insets_raw: Option<RawViewportVec4OutCb>,
    platform_get_window_work_area_insets: Option<ViewportVec4OutCb>,
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

impl CallbackSet {
    fn clear_platform(&mut self) {
        self.platform_create_window = None;
        self.platform_destroy_window = None;
        self.platform_show_window = None;
        self.platform_set_window_pos = None;
        self.platform_get_window_pos_raw = None;
        self.platform_get_window_pos = None;
        self.platform_set_window_size = None;
        self.platform_get_window_size_raw = None;
        self.platform_get_window_size = None;
        self.platform_get_window_framebuffer_scale_raw = None;
        self.platform_get_window_framebuffer_scale = None;
        self.platform_set_window_focus = None;
        self.platform_get_window_dpi_scale = None;
        self.platform_get_window_focus = None;
        self.platform_get_window_minimized = None;
        self.platform_on_changed_viewport = None;
        self.platform_get_window_work_area_insets_raw = None;
        self.platform_get_window_work_area_insets = None;
        self.platform_set_window_title = None;
        self.platform_set_window_alpha = None;
        self.platform_update_window = None;
        self.platform_render_window = None;
        self.platform_swap_buffers = None;
    }

    fn clear_renderer(&mut self) {
        self.renderer_create_window = None;
        self.renderer_destroy_window = None;
        self.renderer_set_window_size = None;
        self.renderer_render_window = None;
        self.renderer_swap_buffers = None;
    }

    fn is_empty(&self) -> bool {
        self.platform_create_window.is_none()
            && self.platform_destroy_window.is_none()
            && self.platform_show_window.is_none()
            && self.platform_set_window_pos.is_none()
            && self.platform_get_window_pos_raw.is_none()
            && self.platform_get_window_pos.is_none()
            && self.platform_set_window_size.is_none()
            && self.platform_get_window_size_raw.is_none()
            && self.platform_get_window_size.is_none()
            && self.platform_get_window_framebuffer_scale_raw.is_none()
            && self.platform_get_window_framebuffer_scale.is_none()
            && self.platform_set_window_focus.is_none()
            && self.platform_get_window_dpi_scale.is_none()
            && self.platform_get_window_focus.is_none()
            && self.platform_get_window_minimized.is_none()
            && self.platform_on_changed_viewport.is_none()
            && self.platform_get_window_work_area_insets_raw.is_none()
            && self.platform_get_window_work_area_insets.is_none()
            && self.platform_set_window_title.is_none()
            && self.platform_set_window_alpha.is_none()
            && self.platform_update_window.is_none()
            && self.platform_render_window.is_none()
            && self.platform_swap_buffers.is_none()
            && self.renderer_create_window.is_none()
            && self.renderer_destroy_window.is_none()
            && self.renderer_set_window_size.is_none()
            && self.renderer_render_window.is_none()
            && self.renderer_swap_buffers.is_none()
    }
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
    RawViewportVec2OutCb,
    get_platform_get_window_pos_raw,
    set_platform_get_window_pos_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_POS_CB,
    platform_get_window_pos,
    ViewportVec2OutCb,
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
    RawViewportVec2OutCb,
    get_platform_get_window_size_raw,
    set_platform_get_window_size_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_SIZE_CB,
    platform_get_window_size,
    ViewportVec2OutCb,
    get_platform_get_window_size,
    set_platform_get_window_size
);
callback_slot!(
    PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB,
    platform_get_window_framebuffer_scale_raw,
    RawViewportVec2OutCb,
    get_platform_get_window_framebuffer_scale_raw,
    set_platform_get_window_framebuffer_scale_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB,
    platform_get_window_framebuffer_scale,
    ViewportVec2OutCb,
    get_platform_get_window_framebuffer_scale,
    set_platform_get_window_framebuffer_scale
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
    PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB,
    platform_get_window_work_area_insets_raw,
    RawViewportVec4OutCb,
    get_platform_get_window_work_area_insets_raw,
    set_platform_get_window_work_area_insets_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB,
    platform_get_window_work_area_insets,
    ViewportVec4OutCb,
    get_platform_get_window_work_area_insets,
    set_platform_get_window_work_area_insets
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
        if let Some(index) = contexts.iter().position(|entry| entry.ctx == ctx) {
            (slot.set)(&mut contexts[index].callbacks, cb);
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
            return;
        }

        if cb.is_none() {
            return;
        }

        let mut callbacks = CallbackSet::default();
        (slot.set)(&mut callbacks, cb);
        contexts.push(ContextCallbacks { ctx, callbacks });
    });
}

pub(super) fn clear_cb_for_current_context<T: Copy>(slot: &CallbackSlot<T>) {
    let ctx = current_context();
    if ctx.is_null() {
        return;
    }

    CONTEXT_CALLBACKS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        if let Some(index) = contexts.iter().position(|entry| entry.ctx == ctx) {
            (slot.set)(&mut contexts[index].callbacks, None);
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
        }
    });
}

pub(super) fn clear_cb_for_platform_io<T: Copy>(
    platform_io: *const sys::ImGuiPlatformIO,
    slot: &CallbackSlot<T>,
) {
    if platform_io.is_null() {
        return;
    }

    CONTEXT_CALLBACKS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        if let Some(index) = contexts.iter().position(|entry| unsafe {
            let entry_platform_io = sys::igGetPlatformIO_ContextPtr(entry.ctx);
            !entry_platform_io.is_null()
                && std::ptr::addr_eq(entry_platform_io.cast_const(), platform_io)
        }) {
            (slot.set)(&mut contexts[index].callbacks, None);
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
        }
    });
}

pub(crate) fn clear_platform_callbacks_for_platform_io(platform_io: *const sys::ImGuiPlatformIO) {
    if platform_io.is_null() {
        return;
    }

    CONTEXT_CALLBACKS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        if let Some(index) = contexts.iter().position(|entry| unsafe {
            let entry_platform_io = sys::igGetPlatformIO_ContextPtr(entry.ctx);
            !entry_platform_io.is_null()
                && std::ptr::addr_eq(entry_platform_io.cast_const(), platform_io)
        }) {
            contexts[index].callbacks.clear_platform();
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
        }
    });
}

pub(crate) fn clear_renderer_callbacks_for_platform_io(platform_io: *const sys::ImGuiPlatformIO) {
    if platform_io.is_null() {
        return;
    }

    CONTEXT_CALLBACKS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        if let Some(index) = contexts.iter().position(|entry| unsafe {
            let entry_platform_io = sys::igGetPlatformIO_ContextPtr(entry.ctx);
            !entry_platform_io.is_null()
                && std::ptr::addr_eq(entry_platform_io.cast_const(), platform_io)
        }) {
            contexts[index].callbacks.clear_renderer();
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
        }
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
