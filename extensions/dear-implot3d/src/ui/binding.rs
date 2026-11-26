use crate::{Plot3DContext, sys};
use std::{cell::RefCell, ptr};

thread_local! {
    static PREVIOUS_CONTEXTS: RefCell<Vec<*mut sys::ImPlot3DContext>> = const { RefCell::new(Vec::new()) };
}

#[must_use = "dropping the guard restores the previous ImPlot3D context"]
struct Plot3DContextGuard;

impl Plot3DContext {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.imgui_binding
            .with_bound_context(|| self.with_plot_context(f))
    }

    pub(crate) fn with_plot_context<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = Plot3DContextGuard::bind(self.raw);
        f()
    }

    pub(crate) fn invalidate_saved_binding(&self) {
        let _ = PREVIOUS_CONTEXTS.try_with(|stack| {
            for previous in stack.borrow_mut().iter_mut() {
                if *previous == self.raw {
                    *previous = ptr::null_mut();
                }
            }
        });
    }
}

impl Plot3DContextGuard {
    fn bind(raw: *mut sys::ImPlot3DContext) -> Option<Self> {
        let previous = unsafe { sys::ImPlot3D_GetCurrentContext() };
        if previous == raw {
            return None;
        }
        PREVIOUS_CONTEXTS.with_borrow_mut(|stack| stack.push(previous));
        unsafe { sys::ImPlot3D_SetCurrentContext(raw) };
        Some(Self)
    }
}

impl Drop for Plot3DContextGuard {
    fn drop(&mut self) {
        let previous = PREVIOUS_CONTEXTS
            .with_borrow_mut(|stack| stack.pop().expect("active ImPlot3D context binding"));
        unsafe { sys::ImPlot3D_SetCurrentContext(previous) };
    }
}
