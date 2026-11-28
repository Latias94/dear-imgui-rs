use super::ui::PlotUi;
use crate::sys;
use dear_imgui_rs::{
    Context as ImGuiContext, ContextBinding, ContextError, ContextThreadLease, Ui,
};
use std::cell::RefCell;

/// ImPlot context that manages the plotting state
///
/// Plotting requires the originating Dear ImGui context. The native plot allocation
/// reserves its owner thread until this `PlotContext` is dropped.
pub struct PlotContext {
    raw: *mut sys::ImPlotContext,
    imgui_binding: ContextBinding,
    _thread_lease: ContextThreadLease,
}

thread_local! {
    // One entry per temporary switch. A previous owner can be dropped during a nested call.
    static PREVIOUS_CONTEXTS: RefCell<Vec<*mut sys::ImPlotContext>> = const { RefCell::new(Vec::new()) };
}

struct PlotContextGuard;

impl PlotContext {
    // PlotUi and PlotScope already verified the Ui's owner. Use its frame/scope checks as
    // well as binding the plot context when an operation touches the current window.
    pub(crate) fn with_ui<R>(&self, ui: &Ui, f: impl FnOnce() -> R) -> R {
        ui.with_bound_context(|| {
            let _guard = PlotContextGuard::bind(self.raw);
            f()
        })
    }

    pub(crate) fn with_bound_context<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.try_with_bound_context(f)
            .unwrap_or_else(|error| panic!("{caller}: {error}"))
    }

    pub(crate) fn try_with_bound_context<R>(
        &self,
        f: impl FnOnce() -> R,
    ) -> Result<R, ContextError> {
        self.imgui_binding.try_with_bound_context(|| {
            let _guard = PlotContextGuard::bind(self.raw);
            f()
        })
    }
}

impl PlotContextGuard {
    fn bind(raw: *mut sys::ImPlotContext) -> Option<Self> {
        let previous = unsafe { sys::ImPlot_GetCurrentContext() };
        if previous == raw {
            return None;
        }
        PREVIOUS_CONTEXTS.with_borrow_mut(|stack| stack.push(previous));
        unsafe { sys::ImPlot_SetCurrentContext(raw) };
        Some(Self)
    }
}

impl Drop for PlotContextGuard {
    fn drop(&mut self) {
        let previous = PREVIOUS_CONTEXTS
            .with_borrow_mut(|stack| stack.pop().expect("active ImPlot context binding"));
        unsafe { sys::ImPlot_SetCurrentContext(previous) };
    }
}

impl PlotContext {
    /// Try to create a new ImPlot context
    ///
    /// This should be called after creating the Dear ImGui context.
    /// The ImPlot context will use the same Dear ImGui context internally.
    pub fn try_create(imgui_ctx: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        let thread_lease = ContextThreadLease::acquire()?;
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
            return Err(dear_imgui_rs::ContextError::Creation {
                reason: "ImPlot_CreateContext returned null".into(),
            }
            .into());
        }

        Ok(Self {
            raw,
            imgui_binding,
            _thread_lease: thread_lease,
        })
    }

    /// Create a new ImPlot context (panics on error)
    pub fn create(imgui_ctx: &ImGuiContext) -> Self {
        Self::try_create(imgui_ctx).expect("Failed to create ImPlot context")
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
        PlotUi {
            context: self,
            ui,
            subplot: None,
        }
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
        // DestroyContext frees the plot's own allocations and clears GImPlot only if current.
        // Attribute frees to the originating ImGui context while it is still available.
        let raw = self.raw;
        let _ = PREVIOUS_CONTEXTS.try_with(|stack| {
            for previous in stack.borrow_mut().iter_mut() {
                if *previous == raw {
                    *previous = std::ptr::null_mut();
                }
            }
        });
        if self
            .imgui_binding
            .try_with_bound_context(|| unsafe { sys::ImPlot_DestroyContext(raw) })
            .is_err()
        {
            // The thread lease still covers the allocator after the ImGui context has gone.
            unsafe { sys::ImPlot_DestroyContext(raw) };
        }
    }
}
