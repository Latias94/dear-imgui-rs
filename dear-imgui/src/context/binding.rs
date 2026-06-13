use parking_lot::ReentrantMutex;
use std::ptr;

use crate::sys;

// This mutex needs to be used to guard all public functions that can affect the underlying
// Dear ImGui active context
pub(crate) static CTX_MUTEX: ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

pub(super) fn clear_current_context() {
    unsafe {
        sys::igSetCurrentContext(ptr::null_mut());
    }
}

pub(super) fn no_current_context() -> bool {
    let ctx = unsafe { sys::igGetCurrentContext() };
    ctx.is_null()
}

struct BoundContextGuard {
    prev: *mut sys::ImGuiContext,
    restore: bool,
}

impl BoundContextGuard {
    fn bind(ctx: *mut sys::ImGuiContext) -> Self {
        unsafe {
            let prev = sys::igGetCurrentContext();
            let restore = prev != ctx;
            if restore {
                sys::igSetCurrentContext(ctx);
            }
            Self { prev, restore }
        }
    }
}

impl Drop for BoundContextGuard {
    fn drop(&mut self) {
        if self.restore {
            unsafe {
                sys::igSetCurrentContext(self.prev);
            }
        }
    }
}

pub(crate) fn with_bound_context<R>(ctx: *mut sys::ImGuiContext, f: impl FnOnce() -> R) -> R {
    let _guard = BoundContextGuard::bind(ctx);
    f()
}
