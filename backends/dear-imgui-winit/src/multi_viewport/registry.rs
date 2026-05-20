use super::*;

pub(super) fn register_viewport_data(ptr: *mut ViewportData) {
    if ptr.is_null() {
        return;
    }
    let ctx = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    VIEWPORT_DATA.with(|items| {
        let mut items = items.borrow_mut();
        if !items
            .iter()
            .any(|(entry_ctx, entry_ptr)| *entry_ctx == ctx && *entry_ptr == ptr)
        {
            items.push((ctx, ptr));
        }
    });
}

pub(super) fn unregister_viewport_data(ptr: *mut ViewportData) {
    if ptr.is_null() {
        return;
    }
    VIEWPORT_DATA.with(|items| {
        items
            .borrow_mut()
            .retain(|(_, entry_ptr)| *entry_ptr != ptr);
    });
}

pub(super) fn is_winit_viewport_data(ptr: *mut ViewportData) -> bool {
    if ptr.is_null() {
        return false;
    }
    let ctx = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    VIEWPORT_DATA.with(|items| {
        items
            .borrow()
            .iter()
            .any(|(entry_ctx, entry_ptr)| *entry_ctx == ctx && *entry_ptr == ptr)
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
