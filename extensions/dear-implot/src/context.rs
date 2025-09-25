use crate::sys;
use dear_imgui::{Context as ImGuiContext, Ui};
use dear_imgui_sys as imgui_sys;

/// ImPlot context that manages the plotting state
///
/// This context is separate from the Dear ImGui context but works alongside it.
/// You need both contexts to create plots.
pub struct PlotContext {
    raw: *mut sys::ImPlotContext,
}

impl PlotContext {
    /// Try to create a new ImPlot context
    ///
    /// This should be called after creating the Dear ImGui context.
    /// The ImPlot context will use the same Dear ImGui context internally.
    pub fn try_create(_imgui_ctx: &ImGuiContext) -> dear_imgui::ImGuiResult<Self> {
        // Bind ImPlot to the current Dear ImGui context before creating.
        // On some toolchains/platforms, not setting this can lead to crashes
        // if ImPlot initialization queries ImGui state during CreateContext.
        unsafe {
            sys::ImPlot_SetImGuiContext(imgui_sys::igGetCurrentContext());
        }

        let raw = unsafe { sys::ImPlot_CreateContext() };
        if raw.is_null() {
            return Err(dear_imgui::ImGuiError::context_creation(
                "ImPlot_CreateContext returned null",
            ));
        }

        // Ensure the newly created context is current (defensive, CreateContext should do this).
        unsafe {
            sys::ImPlot_SetCurrentContext(raw);
        }

        Ok(Self { raw })
    }

    /// Create a new ImPlot context (panics on error)
    pub fn create(imgui_ctx: &ImGuiContext) -> Self {
        Self::try_create(imgui_ctx).expect("Failed to create ImPlot context")
    }

    /// Get the current ImPlot context
    ///
    /// Returns None if no context is current
    pub fn current() -> Option<Self> {
        let raw = unsafe { sys::ImPlot_GetCurrentContext() };
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    /// Set this context as the current ImPlot context
    pub fn set_as_current(&self) {
        unsafe {
            sys::ImPlot_SetCurrentContext(self.raw);
        }
    }

    /// Get a PlotUi for creating plots
    ///
    /// This borrows both the ImPlot context and the Dear ImGui Ui,
    /// ensuring that plots can only be created when both are available.
    pub fn get_plot_ui<'ui>(&'ui self, _ui: &'ui Ui) -> PlotUi<'ui> {
        PlotUi { context: self }
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
        if !self.raw.is_null() {
            unsafe {
                sys::ImPlot_DestroyContext(self.raw);
            }
        }
    }
}

// PlotContext is Send + Sync because ImPlot contexts can be used across threads
// (though not concurrently - you need proper synchronization)
unsafe impl Send for PlotContext {}
unsafe impl Sync for PlotContext {}

/// A temporary reference for building plots
///
/// This struct ensures that plots can only be created when both ImGui and ImPlot
/// contexts are available and properly set up.
pub struct PlotUi<'ui> {
    #[allow(dead_code)]
    context: &'ui PlotContext,
}

impl<'ui> PlotUi<'ui> {
    /// Begin a new plot with the given title
    ///
    /// Returns a PlotToken if the plot was successfully started.
    /// The plot will be automatically ended when the token is dropped.
    pub fn begin_plot(&self, title: &str) -> Option<PlotToken<'_>> {
        let title_cstr = std::ffi::CString::new(title).ok()?;

        let size = sys::ImVec2 { x: -1.0, y: 0.0 };
        let started = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), size, 0) };

        if started {
            Some(PlotToken::new())
        } else {
            None
        }
    }

    /// Begin a plot with custom size
    pub fn begin_plot_with_size(&self, title: &str, size: [f32; 2]) -> Option<PlotToken<'_>> {
        let title_cstr = std::ffi::CString::new(title).ok()?;

        let plot_size = sys::ImVec2 {
            x: size[0],
            y: size[1],
        };
        let started = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), plot_size, 0) };

        if started {
            Some(PlotToken::new())
        } else {
            None
        }
    }

    /// Plot a line with the given label and data
    ///
    /// This is a convenience method that can be called within a plot.
    pub fn plot_line(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }

        let label_cstr = std::ffi::CString::new(label).unwrap_or_default();

        unsafe {
            sys::ImPlot_PlotLine_doublePtrdoublePtr(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                y_data.as_ptr(),
                x_data.len() as i32,
                0,
                0,
                0,
            );
        }
    }

    /// Plot a scatter plot with the given label and data
    pub fn plot_scatter(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }

        let label_cstr = std::ffi::CString::new(label).unwrap_or_default();

        unsafe {
            sys::ImPlot_PlotScatter_doublePtrdoublePtr(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                y_data.as_ptr(),
                x_data.len() as i32,
                0,
                0,
                0,
            );
        }
    }

    /// Check if the plot area is hovered
    pub fn is_plot_hovered(&self) -> bool {
        unsafe { sys::ImPlot_IsPlotHovered() }
    }

    /// Get the mouse position in plot coordinates
    pub fn get_plot_mouse_pos(&self, y_axis: Option<crate::YAxisChoice>) -> sys::ImPlotPoint {
        let y_axis_i32 = crate::y_axis_choice_option_to_i32(y_axis);
        let y_axis = match y_axis_i32 {
            0 => 3,
            1 => 4,
            2 => 5,
            _ => 3,
        };
        let mut out = sys::ImPlotPoint { x: 0.0, y: 0.0 };
        unsafe {
            sys::ImPlot_GetPlotMousePos(&mut out as *mut sys::ImPlotPoint, 0, y_axis);
        }
        out
    }
}

/// Token that represents an active plot
///
/// The plot will be automatically ended when this token is dropped.
pub struct PlotToken<'ui> {
    _lifetime: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> PlotToken<'ui> {
    /// Create a new PlotToken (internal use only)
    pub(crate) fn new() -> Self {
        Self {
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Manually end the plot
    ///
    /// This is called automatically when the token is dropped,
    /// but you can call it manually if needed.
    pub fn end(self) {
        // The actual ending happens in Drop
    }
}

impl<'ui> Drop for PlotToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImPlot_EndPlot();
        }
    }
}
