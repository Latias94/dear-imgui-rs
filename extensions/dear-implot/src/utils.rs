// Utility functions for ImPlot

use crate::{XAxis, YAxis, sys};

/// Check if the plot area is hovered
pub fn is_plot_hovered() -> bool {
    unsafe { sys::ImPlot_IsPlotHovered() }
}

/// Check if any subplots area is hovered
pub fn is_subplots_hovered() -> bool {
    unsafe { sys::ImPlot_IsSubplotsHovered() }
}

/// Check if a legend entry is hovered
pub fn is_legend_entry_hovered(label: &str) -> bool {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    unsafe { sys::ImPlot_IsLegendEntryHovered(c.as_ptr()) }
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
    unsafe { sys::ImPlot_GetPlotMousePos(x_axis as sys::ImAxis, y_axis as sys::ImAxis) }
}

/// Get the mouse position in plot coordinates for specific axes
pub fn get_plot_mouse_position_axes(x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotPoint {
    unsafe { sys::ImPlot_GetPlotMousePos(x_axis as sys::ImAxis, y_axis as sys::ImAxis) }
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

/// Convert pixels to plot coordinates for specific axes
pub fn pixels_to_plot_axes(
    pixel_position: [f32; 2],
    x_axis: XAxis,
    y_axis: YAxis,
) -> sys::ImPlotPoint {
    unsafe {
        let plot = sys::ImPlot_GetCurrentPlot();
        if plot.is_null() {
            return sys::ImPlotPoint { x: 0.0, y: 0.0 };
        }
        let x_axis_ptr = sys::ImPlotPlot_XAxis_Nil(plot, x_axis as i32);
        let y_axis_ptr = sys::ImPlotPlot_YAxis_Nil(plot, y_axis.to_index());
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

/// Convert plot coordinates to pixels for specific axes
pub fn plot_to_pixels_axes(
    plot_position: sys::ImPlotPoint,
    x_axis: XAxis,
    y_axis: YAxis,
) -> [f32; 2] {
    unsafe {
        let plot = sys::ImPlot_GetCurrentPlot();
        if plot.is_null() {
            return [0.0, 0.0];
        }
        let x_axis_ptr = sys::ImPlotPlot_XAxis_Nil(plot, x_axis as i32);
        let y_axis_ptr = sys::ImPlotPlot_YAxis_Nil(plot, y_axis.to_index());
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
    unsafe { sys::ImPlot_GetPlotLimits(x_axis, y_axis) }
}

/// Whether a plot has an active selection region
pub fn is_plot_selected() -> bool {
    unsafe { sys::ImPlot_IsPlotSelected() }
}

/// Get the current plot selection rectangle for specific axes
pub fn get_plot_selection_axes(x_axis: XAxis, y_axis: YAxis) -> Option<sys::ImPlotRect> {
    if !is_plot_selected() {
        return None;
    }
    let rect = unsafe { sys::ImPlot_GetPlotSelection(x_axis as i32, y_axis as i32) };
    Some(rect)
}

/// Draw a simple round annotation marker at (x,y)
pub fn annotation_point(
    x: f64,
    y: f64,
    color: [f32; 4],
    pixel_offset: [f32; 2],
    clamp: bool,
    round: bool,
) {
    let col = sys::ImVec4_c {
        x: color[0],
        y: color[1],
        z: color[2],
        w: color[3],
    };
    let off = sys::ImVec2_c {
        x: pixel_offset[0],
        y: pixel_offset[1],
    };
    unsafe { sys::ImPlot_Annotation_Bool(x, y, col, off, clamp, round) }
}

/// Tag the X axis at position x with a tick-like mark
pub fn tag_x(x: f64, color: [f32; 4], round: bool) {
    let col = sys::ImVec4_c {
        x: color[0],
        y: color[1],
        z: color[2],
        w: color[3],
    };
    unsafe { sys::ImPlot_TagX_Bool(x, col, round) }
}

/// Tag the Y axis at position y with a tick-like mark
pub fn tag_y(y: f64, color: [f32; 4], round: bool) {
    let col = sys::ImVec4_c {
        x: color[0],
        y: color[1],
        z: color[2],
        w: color[3],
    };
    unsafe { sys::ImPlot_TagY_Bool(y, col, round) }
}

/// Get the current plot limits for specific axes
pub fn get_plot_limits_axes(x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotRect {
    unsafe { sys::ImPlot_GetPlotLimits(x_axis as i32, y_axis as i32) }
}

/// Check if an axis is hovered
pub fn is_axis_hovered(axis: i32) -> bool {
    unsafe { sys::ImPlot_IsAxisHovered(axis) }
}

/// Check if the X axis is hovered
pub fn is_plot_x_axis_hovered() -> bool {
    is_axis_hovered(XAxis::X1 as i32)
}

/// Check if a specific X axis is hovered
pub fn is_plot_x_axis_hovered_axis(x_axis: XAxis) -> bool {
    is_axis_hovered(x_axis as i32)
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

/// Check if a specific Y axis is hovered
pub fn is_plot_y_axis_hovered_axis(y_axis: YAxis) -> bool {
    is_axis_hovered(y_axis as i32)
}

/// Show the ImPlot demo window (requires sys demo symbols to be linked)
#[cfg(feature = "demo")]
pub fn show_demo_window(show: &mut bool) {
    unsafe { sys::ImPlot_ShowDemoWindow(show) }
}

/// Stub when demo feature is disabled
#[cfg(not(feature = "demo"))]
pub fn show_demo_window(_show: &mut bool) {}

/// Show the built-in user guide for ImPlot
pub fn show_user_guide() {
    unsafe { sys::ImPlot_ShowUserGuide() }
}

/// Show the metrics window (pass &mut bool for open state)
pub fn show_metrics_window(open: &mut bool) {
    unsafe { sys::ImPlot_ShowMetricsWindow(open as *mut bool) }
}

/// Get current plot position (top-left) in pixels
pub fn get_plot_pos() -> [f32; 2] {
    let out = unsafe { sys::ImPlot_GetPlotPos() };
    [out.x, out.y]
}

/// Get current plot size in pixels
pub fn get_plot_size() -> [f32; 2] {
    let out = unsafe { sys::ImPlot_GetPlotSize() };
    [out.x, out.y]
}

/// Get the underlying ImDrawList for the current plot (unsafe pointer)
pub fn get_plot_draw_list() -> *mut sys::ImDrawList {
    unsafe { sys::ImPlot_GetPlotDrawList() }
}

/// Push plot clip rect
pub fn push_plot_clip_rect(expand: f32) {
    unsafe { sys::ImPlot_PushPlotClipRect(expand) }
}

/// Pop plot clip rect
pub fn pop_plot_clip_rect() {
    unsafe { sys::ImPlot_PopPlotClipRect() }
}

/// Result of a drag interaction
#[derive(Debug, Clone, Copy, Default)]
pub struct DragResult {
    /// True if the underlying value changed this frame
    pub changed: bool,
    /// True if it was clicked this frame
    pub clicked: bool,
    /// True if hovered this frame
    pub hovered: bool,
    /// True if held/active this frame
    pub held: bool,
}

fn color4(rgba: [f32; 4]) -> sys::ImVec4_c {
    sys::ImVec4_c {
        x: rgba[0],
        y: rgba[1],
        z: rgba[2],
        w: rgba[3],
    }
}

/// Draggable point with result flags
pub fn drag_point(
    id: i32,
    x: &mut f64,
    y: &mut f64,
    color: [f32; 4],
    size: f32,
    flags: crate::DragToolFlags,
) -> DragResult {
    let mut clicked = false;
    let mut hovered = false;
    let mut held = false;
    let changed = unsafe {
        sys::ImPlot_DragPoint(
            id,
            x as *mut f64,
            y as *mut f64,
            color4(color),
            size,
            flags.bits() as i32,
            &mut clicked as *mut bool,
            &mut hovered as *mut bool,
            &mut held as *mut bool,
        )
    };
    DragResult {
        changed,
        clicked,
        hovered,
        held,
    }
}

/// Draggable vertical line at x
pub fn drag_line_x(
    id: i32,
    x: &mut f64,
    color: [f32; 4],
    thickness: f32,
    flags: crate::DragToolFlags,
) -> DragResult {
    let mut clicked = false;
    let mut hovered = false;
    let mut held = false;
    let changed = unsafe {
        sys::ImPlot_DragLineX(
            id,
            x as *mut f64,
            color4(color),
            thickness,
            flags.bits() as i32,
            &mut clicked as *mut bool,
            &mut hovered as *mut bool,
            &mut held as *mut bool,
        )
    };
    DragResult {
        changed,
        clicked,
        hovered,
        held,
    }
}

/// Draggable horizontal line at y
pub fn drag_line_y(
    id: i32,
    y: &mut f64,
    color: [f32; 4],
    thickness: f32,
    flags: crate::DragToolFlags,
) -> DragResult {
    let mut clicked = false;
    let mut hovered = false;
    let mut held = false;
    let changed = unsafe {
        sys::ImPlot_DragLineY(
            id,
            y as *mut f64,
            color4(color),
            thickness,
            flags.bits() as i32,
            &mut clicked as *mut bool,
            &mut hovered as *mut bool,
            &mut held as *mut bool,
        )
    };
    DragResult {
        changed,
        clicked,
        hovered,
        held,
    }
}
