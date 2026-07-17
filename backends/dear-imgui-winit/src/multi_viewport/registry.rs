use super::*;
use std::cell::RefCell;

struct RegisteredViewportData {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    context: ContextBinding,
    data: *mut ViewportData,
}

thread_local! {
    static VIEWPORT_DATA: RefCell<Vec<RegisteredViewportData>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn register_viewport_data(context: &ContextBinding, ptr: *mut ViewportData) {
    if ptr.is_null() {
        return;
    }
    let Ok(context_raw) =
        context.try_with_bound_context(|| unsafe { dear_imgui_rs::sys::igGetCurrentContext() })
    else {
        return;
    };
    VIEWPORT_DATA.with(|items| {
        let mut items = items.borrow_mut();
        if !items
            .iter()
            .any(|entry| entry.context.id() == context.id() && entry.data == ptr)
        {
            items.push(RegisteredViewportData {
                context_raw,
                context: context.clone(),
                data: ptr,
            });
        }
    });
}

pub(super) fn unregister_viewport_data(ptr: *mut ViewportData) {
    if ptr.is_null() {
        return;
    }
    VIEWPORT_DATA.with(|items| {
        items.borrow_mut().retain(|entry| entry.data != ptr);
    });
}

pub(super) fn with_registered_context<R>(
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    f: impl FnOnce(&ContextBinding) -> R,
) -> Option<R> {
    if context_raw.is_null() {
        return None;
    }
    let context = VIEWPORT_DATA.with(|items| {
        items
            .borrow()
            .iter()
            .find(|entry| entry.context_raw == context_raw && entry.context.is_alive())
            .map(|entry| entry.context.clone())
    })?;
    context.try_with_bound_context(|| f(&context)).ok()
}

pub(super) fn is_winit_viewport_data(ptr: *mut ViewportData) -> bool {
    if ptr.is_null() {
        return false;
    }
    let context_raw = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    VIEWPORT_DATA.with(|items| {
        items.borrow().iter().any(|entry| {
            entry.context_raw == context_raw && entry.context.is_alive() && entry.data == ptr
        })
    })
}

pub(super) unsafe fn viewport_data_ref<'a>(
    vp: *const dear_imgui_rs::sys::ImGuiViewport,
) -> Option<&'a ViewportData> {
    if vp.is_null() {
        return None;
    }
    let ptr = unsafe { (*vp).PlatformUserData as *mut ViewportData };
    if is_winit_viewport_data(ptr) {
        unsafe { ptr.as_ref() }
    } else {
        None
    }
}

pub(super) unsafe fn viewport_data_mut<'a>(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Option<&'a mut ViewportData> {
    if vp.is_null() {
        return None;
    }
    let ptr = unsafe { (*vp).PlatformUserData as *mut ViewportData };
    if is_winit_viewport_data(ptr) {
        unsafe { ptr.as_mut() }
    } else {
        None
    }
}

pub(super) unsafe fn drop_viewport_data(ptr: *mut ViewportData) {
    if !is_winit_viewport_data(ptr) {
        return;
    }
    unregister_viewport_data(ptr);
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}
