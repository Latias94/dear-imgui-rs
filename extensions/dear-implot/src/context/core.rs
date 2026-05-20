use super::ui::PlotUi;
use crate::sys;
use dear_imgui_rs::{Context as ImGuiContext, Ui};
use dear_imgui_sys as imgui_sys;

/// ImPlot context that manages the plotting state
///
/// This context is separate from the Dear ImGui context but works alongside it.
/// You need both contexts to create plots.
pub struct PlotContext {
    raw: *mut sys::ImPlotContext,
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    owns_context: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct PlotContextBinding {
    plot_ctx_raw: *mut sys::ImPlotContext,
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
}

impl PlotContextBinding {
    pub(crate) fn bind(self, caller: &str) {
        assert!(
            !self.imgui_ctx_raw.is_null(),
            "{caller} requires an active ImGui context"
        );
        assert!(
            !self.plot_ctx_raw.is_null(),
            "{caller} requires an active ImPlot context"
        );
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            self.imgui_ctx_raw,
            "{caller} must be used with the currently-active ImGui context"
        );
        unsafe {
            sys::ImPlot_SetImGuiContext(self.imgui_ctx_raw);
            sys::ImPlot_SetCurrentContext(self.plot_ctx_raw);
        }
    }
}

impl PlotContext {
    /// Try to create a new ImPlot context
    ///
    /// This should be called after creating the Dear ImGui context.
    /// The ImPlot context will use the same Dear ImGui context internally.
    pub fn try_create(imgui_ctx: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        let imgui_ctx_raw = imgui_ctx.as_raw();
        let imgui_alive = Some(imgui_ctx.alive_token());
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            imgui_ctx_raw,
            "dear-implot: PlotContext must be created with the currently-active ImGui context"
        );

        // Bind ImPlot to the ImGui context before creating.
        // On some toolchains/platforms, not setting this can lead to crashes
        // if ImPlot initialization queries ImGui state during CreateContext.
        unsafe { sys::ImPlot_SetImGuiContext(imgui_ctx_raw) };

        let raw = unsafe { sys::ImPlot_CreateContext() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "ImPlot_CreateContext returned null",
            ));
        }

        // Ensure the newly created context is current (defensive, CreateContext should do this).
        unsafe { sys::ImPlot_SetCurrentContext(raw) };

        Ok(Self {
            raw,
            imgui_ctx_raw,
            imgui_alive,
            owns_context: true,
        })
    }

    /// Create a new ImPlot context (panics on error)
    pub fn create(imgui_ctx: &ImGuiContext) -> Self {
        Self::try_create(imgui_ctx).expect("Failed to create ImPlot context")
    }

    /// Get the current ImPlot context as a non-owning raw-context wrapper.
    ///
    /// Returns None if no context is current
    ///
    /// # Safety
    ///
    /// The returned value does not own the current ImPlot context and cannot prove that the
    /// associated ImGui context remains alive. The caller must ensure the raw ImPlot and ImGui
    /// contexts outlive the returned wrapper and are used on the same thread/context stack.
    pub unsafe fn current() -> Option<Self> {
        let raw = unsafe { sys::ImPlot_GetCurrentContext() };
        if raw.is_null() {
            None
        } else {
            Some(Self {
                raw,
                imgui_ctx_raw: unsafe { imgui_sys::igGetCurrentContext() },
                imgui_alive: None,
                owns_context: false,
            })
        }
    }

    /// Set this context as the current ImPlot context
    pub fn set_as_current(&self) {
        self.assert_imgui_alive();
        self.binding().bind("dear-implot: PlotContext");
    }

    pub(crate) fn assert_imgui_alive(&self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot: ImGui context has been dropped"
            );
        }
    }

    pub(crate) fn binding(&self) -> PlotContextBinding {
        PlotContextBinding {
            plot_ctx_raw: self.raw,
            imgui_ctx_raw: self.imgui_ctx_raw,
        }
    }

    pub(crate) fn imgui_alive_token(&self) -> Option<dear_imgui_rs::ContextAliveToken> {
        self.imgui_alive.clone()
    }

    /// Get a PlotUi for creating plots
    ///
    /// This borrows both the ImPlot context and the Dear ImGui Ui,
    /// ensuring that plots can only be created when both are available.
    pub fn get_plot_ui<'ui>(&'ui self, ui: &'ui Ui) -> PlotUi<'ui> {
        self.set_as_current();
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

        if let Some(alive) = &self.imgui_alive {
            if !alive.is_alive() {
                // Avoid calling into ImGui allocators after the context has been dropped.
                // Best-effort: leak the ImPlot context instead of risking UB.
                return;
            }
        }

        unsafe {
            let prev_imgui = imgui_sys::igGetCurrentContext();
            imgui_sys::igSetCurrentContext(self.imgui_ctx_raw);
            sys::ImPlot_SetImGuiContext(self.imgui_ctx_raw);

            if sys::ImPlot_GetCurrentContext() == self.raw {
                sys::ImPlot_SetCurrentContext(std::ptr::null_mut());
            }
            sys::ImPlot_DestroyContext(self.raw);

            imgui_sys::igSetCurrentContext(prev_imgui);
        }
    }
}

// ImPlot context is tied to Dear ImGui and not thread-safe to send/share.
