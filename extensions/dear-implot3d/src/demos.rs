use crate::Plot3DUi;
use crate::sys;

impl Plot3DUi<'_> {
    /// Show upstream ImPlot3D demos (from C++ demo).
    ///
    /// This displays all available ImPlot3D demos in a single window.
    /// Useful for learning and testing the library.
    pub fn show_all_demos(&self) {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_ShowAllDemos() }
    }

    /// Show the main ImPlot3D demo window (C++ upstream).
    ///
    /// This displays the main demo window with tabs for different plot types.
    pub fn show_demo_window(&self) {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_ShowDemoWindow(std::ptr::null_mut()) }
    }

    /// Show the main ImPlot3D demo window with a visibility flag.
    pub fn show_demo_window_with_flag(&self, p_open: &mut bool) {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_ShowDemoWindow(p_open as *mut bool) }
    }

    /// Show the ImPlot3D style editor window.
    ///
    /// This displays a window for editing ImPlot3D style settings in real time.
    pub fn show_style_editor(&self) {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_ShowStyleEditor(std::ptr::null_mut()) }
    }

    /// Show the ImPlot3D metrics/debugger window.
    ///
    /// This displays performance metrics and debugging information.
    pub fn show_metrics_window(&self) {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_ShowMetricsWindow(std::ptr::null_mut()) }
    }

    /// Show the ImPlot3D metrics/debugger window with a visibility flag.
    pub fn show_metrics_window_with_flag(&self, p_open: &mut bool) {
        let _guard = self.bind();
        unsafe { sys::ImPlot3D_ShowMetricsWindow(p_open as *mut bool) }
    }
}
