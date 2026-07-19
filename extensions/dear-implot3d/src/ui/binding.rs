use crate::sys;
use dear_imgui_rs::{ContextBinding, ContextBindingError};

#[derive(Clone)]
pub(crate) struct Plot3DContextBinding {
    pub(crate) plot_ctx_raw: *mut sys::ImPlot3DContext,
    pub(crate) imgui_binding: ContextBinding,
}

#[must_use = "dropping the guard restores the previous ImPlot3D context"]
struct Plot3DContextGuard {
    prev_plot_ctx_raw: *mut sys::ImPlot3DContext,
    restore_plot: bool,
}

impl Plot3DContextBinding {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.imgui_binding.with_bound_context(|| {
            let _guard = Plot3DContextGuard::bind(self.plot_ctx_raw);
            f()
        })
    }

    pub(crate) fn try_with_bound_context<R>(
        &self,
        f: impl FnOnce() -> R,
    ) -> Result<R, ContextBindingError> {
        self.imgui_binding.try_with_bound_context(|| {
            let _guard = Plot3DContextGuard::bind(self.plot_ctx_raw);
            f()
        })
    }
}

impl Plot3DContextGuard {
    fn bind(plot_ctx_raw: *mut sys::ImPlot3DContext) -> Self {
        assert!(
            !plot_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImPlot3D context"
        );
        let prev_plot_ctx_raw = unsafe { sys::ImPlot3D_GetCurrentContext() };
        let restore_plot = prev_plot_ctx_raw != plot_ctx_raw;
        unsafe {
            sys::ImPlot3D_SetCurrentContext(plot_ctx_raw);
        }
        Self {
            prev_plot_ctx_raw,
            restore_plot,
        }
    }
}

impl Drop for Plot3DContextGuard {
    fn drop(&mut self) {
        if self.restore_plot {
            unsafe {
                sys::ImPlot3D_SetCurrentContext(self.prev_plot_ctx_raw);
            }
        }
    }
}
