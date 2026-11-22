use crate::Plot3DUi;
use crate::sys;

impl Plot3DUi<'_> {
    fn show_native_tool(&self, show: impl FnOnce()) {
        // Upstream demos can begin plots. The metrics and style tools can clear plot/item caches.
        let _exclusive = self.context.enter_plot();
        self.with_bound_context(show);
    }

    /// Show upstream ImPlot3D demos (from C++ demo).
    ///
    /// This displays all available ImPlot3D demos in a single window.
    /// Useful for learning and testing the library.
    pub fn show_all_demos(&self) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowAllDemos() })
    }

    /// Show the ImPlot3D demo. Its ImGui tools use the core font-atlas filter.
    ///
    /// This displays the main demo window with tabs for different plot types.
    pub fn show_demo_window(&self) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowDemoWindow(std::ptr::null_mut()) })
    }

    /// Show the main ImPlot3D demo window with a visibility flag.
    pub fn show_demo_window_with_flag(&self, p_open: &mut bool) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowDemoWindow(p_open as *mut bool) })
    }

    /// Show the style editor with this plot context's saved reference and color preview.
    ///
    /// This displays a window for editing ImPlot3D style settings in real time.
    pub fn show_style_editor(&self) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowStyleEditor(std::ptr::null_mut()) })
    }

    /// Show the ImPlot3D metrics/debugger window.
    ///
    /// This displays performance metrics and debugging information.
    pub fn show_metrics_window(&self) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowMetricsWindow(std::ptr::null_mut()) })
    }

    /// Show the ImPlot3D metrics/debugger window with a visibility flag.
    pub fn show_metrics_window_with_flag(&self, p_open: &mut bool) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowMetricsWindow(p_open as *mut bool) })
    }
}
