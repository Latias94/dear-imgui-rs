use super::set::{CallbackSet, CallbackSlot, ContextCallbacks};
use crate::sys;
use std::cell::RefCell;

thread_local! {
    static CONTEXT_CALLBACKS: RefCell<Vec<ContextCallbacks>> = RefCell::new(Vec::new());
}

#[inline]
fn current_context() -> *mut sys::ImGuiContext {
    unsafe { sys::igGetCurrentContext() }
}

#[inline]
pub(in crate::platform_io::trampolines) fn load_cb<T: Copy>(slot: &CallbackSlot<T>) -> Option<T> {
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
pub(in crate::platform_io) fn store_cb<T: Copy>(slot: &CallbackSlot<T>, cb: Option<T>) {
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

pub(in crate::platform_io) fn clear_cb_for_current_context<T: Copy>(slot: &CallbackSlot<T>) {
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

pub(in crate::platform_io) fn clear_cb_for_platform_io<T: Copy>(
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

pub(in crate::platform_io) fn clear_platform_callbacks_for_platform_io(
    platform_io: *const sys::ImGuiPlatformIO,
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
            contexts[index].callbacks.clear_platform();
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
        }
    });
}

pub(in crate::platform_io) fn clear_renderer_callbacks_for_platform_io(
    platform_io: *const sys::ImGuiPlatformIO,
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
            contexts[index].callbacks.clear_renderer();
            if contexts[index].callbacks.is_empty() {
                contexts.remove(index);
            }
        }
    });
}

pub(in crate::platform_io) fn clear_callbacks_for_context(ctx: *mut sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }
    CONTEXT_CALLBACKS.with(|contexts| {
        contexts.borrow_mut().retain(|entry| entry.ctx != ctx);
    });
}

#[inline]
pub(in crate::platform_io::trampolines) fn abort_if_panicked<T>(
    ctx: &str,
    res: Result<T, Box<dyn std::any::Any + Send>>,
) -> T {
    match res {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-imgui-rs: panic in PlatformIO callback ({ctx})");
            std::process::abort();
        }
    }
}
