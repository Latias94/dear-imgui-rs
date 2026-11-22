use crate::{Plot3DContext, sys};

#[must_use = "dropping the guard restores the previous ImPlot3D context"]
struct Plot3DContextGuard {
    prev_plot_ctx_raw: *mut sys::ImPlot3DContext,
    restore_plot: bool,
}

impl Plot3DContext {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.imgui_binding
            .with_bound_context(|| self.with_plot_context(f))
    }

    pub(crate) fn with_plot_context<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = Plot3DContextGuard::bind(self.raw);
        f()
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
