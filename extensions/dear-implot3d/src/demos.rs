use crate::Plot3DUi;
use crate::sys;

impl Plot3DUi<'_> {
    fn show_native_tool(&self, show: impl FnOnce()) {
        // Upstream demos can begin plots. The metrics and style tools can clear plot/item caches.
        let _exclusive = self.context.enter_plot();
        self.with_bound_context(|| {
            show();
            // The native style editor shares its saved reference style across contexts. A
            // restored custom colormap may belong to a different context's registry.
            unsafe {
                let style = sys::ImPlot3D_GetStyle();
                if !(0..sys::ImPlot3D_GetColormapCount()).contains(&(*style).Colormap) {
                    (*style).Colormap = sys::ImPlot3DColormap_Deep as _;
                }
            }
        });
    }

    /// Show upstream ImPlot3D demos (from C++ demo).
    ///
    /// This displays all available ImPlot3D demos in a single window.
    /// Useful for learning and testing the library.
    pub fn show_all_demos(&self) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowAllDemos() })
    }

    /// Show the main ImPlot3D demo window (C++ upstream).
    ///
    /// This displays the main demo window with tabs for different plot types.
    pub fn show_demo_window(&self) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowDemoWindow(std::ptr::null_mut()) })
    }

    /// Show the main ImPlot3D demo window with a visibility flag.
    pub fn show_demo_window_with_flag(&self, p_open: &mut bool) {
        self.show_native_tool(|| unsafe { sys::ImPlot3D_ShowDemoWindow(p_open as *mut bool) })
    }

    /// Show the ImPlot3D style editor window.
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
