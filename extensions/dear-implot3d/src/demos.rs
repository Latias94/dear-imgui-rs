use crate::sys;

/// Show upstream ImPlot3D demos (from C++ demo)
///
/// This displays all available ImPlot3D demos in a single window.
/// Useful for learning and testing the library.
pub fn show_all_demos() {
    unsafe { sys::ImPlot3D_ShowAllDemos() }
}

/// Show the main ImPlot3D demo window (C++ upstream)
///
/// This displays the main demo window with tabs for different plot types.
/// Pass `None` to always show, or `Some(&mut bool)` to control visibility.
///
/// # Example
///
/// ```no_run
/// use dear_implot3d::*;
///
/// let mut show_demo = true;
/// show_demo_window_with_flag(&mut show_demo);
/// ```
pub fn show_demo_window() {
    unsafe { sys::ImPlot3D_ShowDemoWindow(std::ptr::null_mut()) }
}

/// Show the main ImPlot3D demo window with a visibility flag
pub fn show_demo_window_with_flag(p_open: &mut bool) {
    unsafe { sys::ImPlot3D_ShowDemoWindow(p_open as *mut bool) }
}

/// Show the ImPlot3D style editor window
///
/// This displays a window for editing ImPlot3D style settings in real-time.
/// Pass `None` to use the current style, or `Some(&mut ImPlot3DStyle)` to edit a specific style.
pub fn show_style_editor() {
    unsafe { sys::ImPlot3D_ShowStyleEditor(std::ptr::null_mut()) }
}

/// Show the ImPlot3D metrics/debugger window
///
/// This displays performance metrics and debugging information.
/// Pass `None` to always show, or `Some(&mut bool)` to control visibility.
pub fn show_metrics_window() {
    unsafe { sys::ImPlot3D_ShowMetricsWindow(std::ptr::null_mut()) }
}

/// Show the ImPlot3D metrics/debugger window with a visibility flag
pub fn show_metrics_window_with_flag(p_open: &mut bool) {
    unsafe { sys::ImPlot3D_ShowMetricsWindow(p_open as *mut bool) }
}
