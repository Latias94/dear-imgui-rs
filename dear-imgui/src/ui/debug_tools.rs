use super::*;

impl Ui {
    /// Renders a demo window (previously called a test window), which demonstrates most
    /// Dear ImGui features.
    #[doc(alias = "ShowDemoWindow")]
    pub fn show_demo_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowDemoWindow(opened);
        });
    }

    /// Renders an about window.
    ///
    /// Displays the Dear ImGui version/credits, and build/system information.
    #[doc(alias = "ShowAboutWindow")]
    pub fn show_about_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowAboutWindow(opened);
        });
    }

    /// Renders a metrics/debug window.
    ///
    /// Displays Dear ImGui internals: draw commands (with individual draw calls and vertices),
    /// window list, basic internal state, etc.
    #[doc(alias = "ShowMetricsWindow")]
    pub fn show_metrics_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowMetricsWindow(opened);
        });
    }

    /// Renders a basic help/info block (not a window)
    #[doc(alias = "ShowUserGuide")]
    pub fn show_user_guide(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowUserGuide();
        });
    }

    // ============================================================================
    // Additional Demo, Debug, Information (non-duplicate methods)
    // ============================================================================

    /// Renders a debug log window.
    ///
    /// Displays a simplified log of important dear imgui events.
    #[doc(alias = "ShowDebugLogWindow")]
    pub fn show_debug_log_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowDebugLogWindow(opened);
        });
    }

    /// Renders an ID stack tool window.
    ///
    /// Hover items with mouse to query information about the source of their unique ID.
    #[doc(alias = "ShowIDStackToolWindow")]
    pub fn show_id_stack_tool_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowIDStackToolWindow(opened);
        });
    }

    /// Returns the Dear ImGui version string
    #[doc(alias = "GetVersion")]
    pub fn get_version(&self) -> &str {
        self.run_with_bound_context(|| unsafe {
            let version_ptr = sys::igGetVersion();
            if version_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(version_ptr);
            c_str.to_str().unwrap_or("Unknown")
        })
    }
}
