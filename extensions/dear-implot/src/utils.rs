// Utility functions for ImPlot

use crate::sys;

/// Check if the plot area is hovered
pub fn is_plot_hovered() -> bool {
    unsafe { sys::ImPlot_IsPlotHovered() }
}

/// Get the mouse position in plot coordinates
pub fn get_plot_mouse_position(y_axis_choice: Option<crate::YAxisChoice>) -> sys::ImPlotPoint {
    let x_axis = 0; // ImAxis_X1
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 3,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 4, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 5,  // ImAxis_Y3
        None => 3,                             // Default to Y1
    };
    let mut out = sys::ImPlotPoint { x: 0.0, y: 0.0 };
    unsafe {
        sys::ImPlot_GetPlotMousePos(&mut out as *mut sys::ImPlotPoint, x_axis, y_axis);
    }
    out
}

/// Convert pixels to plot coordinates
pub fn pixels_to_plot(
    pixel_position: [f32; 2],
    y_axis_choice: Option<crate::YAxisChoice>,
) -> sys::ImPlotPoint {
    // Map absolute pixel coordinates to plot coordinates using current plot's axes
    let y_index = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 0,
        Some(crate::YAxisChoice::Second) => 1,
        Some(crate::YAxisChoice::Third) => 2,
        None => 0,
    };
    unsafe {
        let plot = sys::ImPlot_GetCurrentPlot();
        if plot.is_null() {
            return sys::ImPlotPoint { x: 0.0, y: 0.0 };
        }
        let x_axis_ptr = sys::ImPlotPlot_XAxis_Nil(plot, 0);
        let y_axis_ptr = sys::ImPlotPlot_YAxis_Nil(plot, y_index);
        let x = sys::ImPlotAxis_PixelsToPlot(x_axis_ptr, pixel_position[0]);
        let y = sys::ImPlotAxis_PixelsToPlot(y_axis_ptr, pixel_position[1]);
        sys::ImPlotPoint { x, y }
    }
}

/// Convert plot coordinates to pixels
pub fn plot_to_pixels(
    plot_position: sys::ImPlotPoint,
    y_axis_choice: Option<crate::YAxisChoice>,
) -> [f32; 2] {
    let y_index = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 0,
        Some(crate::YAxisChoice::Second) => 1,
        Some(crate::YAxisChoice::Third) => 2,
        None => 0,
    };
    unsafe {
        let plot = sys::ImPlot_GetCurrentPlot();
        if plot.is_null() {
            return [0.0, 0.0];
        }
        let x_axis_ptr = sys::ImPlotPlot_XAxis_Nil(plot, 0);
        let y_axis_ptr = sys::ImPlotPlot_YAxis_Nil(plot, y_index);
        let px = sys::ImPlotAxis_PlotToPixels(x_axis_ptr, plot_position.x);
        let py = sys::ImPlotAxis_PlotToPixels(y_axis_ptr, plot_position.y);
        [px, py]
    }
}

/// Get the current plot limits
pub fn get_plot_limits(
    _x_axis_choice: Option<crate::YAxisChoice>,
    y_axis_choice: Option<crate::YAxisChoice>,
) -> sys::ImPlotRect {
    let x_axis = 0; // ImAxis_X1
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 3,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 4, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 5,  // ImAxis_Y3
        None => 3,                             // Default to Y1
    };
    let mut rect: sys::ImPlotRect = unsafe { std::mem::zeroed() };
    unsafe { sys::ImPlot_GetPlotLimits(&mut rect as *mut sys::ImPlotRect, x_axis, y_axis) };
    rect
}

/// Check if an axis is hovered
pub fn is_axis_hovered(axis: i32) -> bool {
    unsafe { sys::ImPlot_IsAxisHovered(axis) }
}

/// Check if the X axis is hovered
pub fn is_plot_x_axis_hovered() -> bool {
    is_axis_hovered(0) // ImAxis_X1
}

/// Check if a Y axis is hovered
pub fn is_plot_y_axis_hovered(y_axis_choice: Option<crate::YAxisChoice>) -> bool {
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 3,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 4, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 5,  // ImAxis_Y3
        None => 3,                             // Default to Y1
    };
    is_axis_hovered(y_axis)
}

/// Show the ImPlot demo window (requires sys demo symbols to be linked)
#[cfg(feature = "demo")]
pub fn show_demo_window(show: &mut bool) {
    unsafe { sys::ImPlot_ShowDemoWindow(show) }
}

/// Stub when demo feature is disabled
#[cfg(not(feature = "demo"))]
pub fn show_demo_window(_show: &mut bool) {}
