// Utility functions for ImPlot

use crate::sys;

/// Check if the plot area is hovered
pub fn is_plot_hovered() -> bool {
    unsafe { sys::ImPlot_IsPlotHovered() }
}

/// Check if the plot is selected (selection box)
pub fn is_plot_selected() -> bool {
    // Note: ImPlot_IsPlotQueried doesn't exist in current version
    // This is a placeholder - check ImPlot documentation for correct function
    false
}

/// Get the mouse position in plot coordinates
pub fn get_plot_mouse_position(y_axis_choice: Option<crate::YAxisChoice>) -> sys::ImPlotPoint {
    let x_axis = 0; // ImAxis_X1
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 1,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 2, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 3,  // ImAxis_Y3
        None => 1,                             // Default to Y1
    };
    unsafe { sys::ImPlot_GetPlotMousePos(x_axis, y_axis) }
}

/// Convert pixels to plot coordinates
pub fn pixels_to_plot(
    pixel_position: [f32; 2],
    y_axis_choice: Option<crate::YAxisChoice>,
) -> sys::ImPlotPoint {
    let x_axis = 0; // ImAxis_X1
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 1,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 2, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 3,  // ImAxis_Y3
        None => 1,                             // Default to Y1
    };
    let pixel_vec = sys::ImVec2 {
        x: pixel_position[0],
        y: pixel_position[1],
    };
    unsafe { sys::ImPlot_PixelsToPlot(&pixel_vec as *const sys::ImVec2, x_axis, y_axis) }
}

/// Convert plot coordinates to pixels
pub fn plot_to_pixels(
    plot_position: sys::ImPlotPoint,
    y_axis_choice: Option<crate::YAxisChoice>,
) -> [f32; 2] {
    let x_axis = 0; // ImAxis_X1
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 1,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 2, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 3,  // ImAxis_Y3
        None => 1,                             // Default to Y1
    };
    let pixel_position = unsafe {
        sys::ImPlot_PlotToPixels(&plot_position as *const sys::ImPlotPoint, x_axis, y_axis)
    };
    [pixel_position.x, pixel_position.y]
}

/// Get the current plot limits
pub fn get_plot_limits(
    _x_axis_choice: Option<crate::YAxisChoice>,
    y_axis_choice: Option<crate::YAxisChoice>,
) -> sys::ImPlotRect {
    let x_axis = 0; // ImAxis_X1
    let y_axis = match y_axis_choice {
        Some(crate::YAxisChoice::First) => 1,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 2, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 3,  // ImAxis_Y3
        None => 1,                             // Default to Y1
    };
    unsafe { sys::ImPlot_GetPlotLimits(x_axis, y_axis) }
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
        Some(crate::YAxisChoice::First) => 1,  // ImAxis_Y1
        Some(crate::YAxisChoice::Second) => 2, // ImAxis_Y2
        Some(crate::YAxisChoice::Third) => 3,  // ImAxis_Y3
        None => 1,                             // Default to Y1
    };
    is_axis_hovered(y_axis)
}

/// Show the ImPlot demo window
pub fn show_demo_window(show: &mut bool) {
    unsafe {
        sys::ImPlot_ShowDemoWindow(show);
    }
}
