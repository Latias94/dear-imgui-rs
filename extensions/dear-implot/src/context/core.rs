use super::ui::PlotUi;
use crate::sys;
use dear_imgui_rs::{Context as ImGuiContext, ContextBinding, ContextBindingError, Ui};

/// ImPlot context that manages the plotting state
///
/// This context is separate from the Dear ImGui context but works alongside it.
/// You need both contexts to create plots.
pub struct PlotContext {
    raw: *mut sys::ImPlotContext,
    imgui_binding: ContextBinding,
    owns_context: bool,
}

#[derive(Clone)]
pub(crate) struct PlotContextBinding {
    plot_ctx_raw: *mut sys::ImPlotContext,
    imgui_binding: ContextBinding,
}

#[must_use = "dropping the guard restores the previous ImPlot context"]
struct PlotContextGuard {
    prev_plot_ctx_raw: *mut sys::ImPlotContext,
    restore_plot: bool,
}

impl PlotContextBinding {
    pub(crate) fn with_bound_context<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.try_with_bound_context(f)
            .unwrap_or_else(|error| panic!("{caller}: {error}"))
    }

    pub(crate) fn try_with_bound_context<R>(
        &self,
        f: impl FnOnce() -> R,
    ) -> Result<R, ContextBindingError> {
        self.imgui_binding.try_with_bound_context(|| {
            let _guard = PlotContextGuard::bind(self.plot_ctx_raw);
            f()
        })
    }
}

impl PlotContextGuard {
    fn bind(plot_ctx_raw: *mut sys::ImPlotContext) -> Self {
        assert!(
            !plot_ctx_raw.is_null(),
            "dear-implot requires an active ImPlot context"
        );
        let prev_plot_ctx_raw = unsafe { sys::ImPlot_GetCurrentContext() };
        let restore_plot = prev_plot_ctx_raw != plot_ctx_raw;
        unsafe {
            sys::ImPlot_SetCurrentContext(plot_ctx_raw);
        }
        Self {
            prev_plot_ctx_raw,
            restore_plot,
        }
    }
}

impl Drop for PlotContextGuard {
    fn drop(&mut self) {
        if self.restore_plot {
            unsafe {
                sys::ImPlot_SetCurrentContext(self.prev_plot_ctx_raw);
            }
        }
    }
}

impl PlotContext {
    /// Try to create a new ImPlot context
    ///
    /// This should be called after creating the Dear ImGui context.
    /// The ImPlot context will use the same Dear ImGui context internally.
    pub fn try_create(imgui_ctx: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        let imgui_binding = imgui_ctx.binding();
        let raw = imgui_binding.with_bound_context(|| unsafe {
            let prev_plot = sys::ImPlot_GetCurrentContext();
            let raw = sys::ImPlot_CreateContext();
            if sys::ImPlot_GetCurrentContext() != prev_plot {
                sys::ImPlot_SetCurrentContext(prev_plot);
            }
            raw
        });
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "ImPlot_CreateContext returned null",
            ));
        }

        Ok(Self {
            raw,
            imgui_binding,
            owns_context: true,
        })
    }

    /// Create a new ImPlot context (panics on error)
    pub fn create(imgui_ctx: &ImGuiContext) -> Self {
        Self::try_create(imgui_ctx).expect("Failed to create ImPlot context")
    }

    pub(crate) fn binding(&self) -> PlotContextBinding {
        PlotContextBinding {
            plot_ctx_raw: self.raw,
            imgui_binding: self.imgui_binding.clone(),
        }
    }

    /// Get a PlotUi for creating plots
    ///
    /// This borrows both the ImPlot context and the Dear ImGui Ui,
    /// ensuring that plots can only be created when both are available.
    pub fn get_plot_ui<'ui>(&'ui self, ui: &'ui Ui) -> PlotUi<'ui> {
        assert_eq!(
            ui.context_id(),
            self.imgui_binding.id(),
            "dear-implot: PlotContext::get_plot_ui() requires a Ui from the owning ImGui context"
        );
        PlotUi { context: self, ui }
    }

    /// Get the raw ImPlot context pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is used safely and not stored
    /// beyond the lifetime of this context.
    pub unsafe fn raw(&self) -> *mut sys::ImPlotContext {
        self.raw
    }
}

impl Drop for PlotContext {
    fn drop(&mut self) {
        if !self.owns_context || self.raw.is_null() {
            return;
        }

        let _ = self.imgui_binding.try_with_bound_context(|| unsafe {
            let prev_plot = sys::ImPlot_GetCurrentContext();
            let restore_plot = if prev_plot == self.raw {
                std::ptr::null_mut()
            } else {
                prev_plot
            };
            sys::ImPlot_DestroyContext(self.raw);
            sys::ImPlot_SetCurrentContext(restore_plot);
        });
    }
}

// ImPlot context is tied to Dear ImGui and not thread-safe to send/share.
