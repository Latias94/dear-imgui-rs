// Utility functions for ImPlot

use crate::{Axis, PlotScope, PlotUi, XAxis, YAxis, sys};
use dear_imgui_rs::with_scratch_txt;
use std::{borrow::Cow, fmt};

fn literal_printf_format(text: &str) -> Cow<'_, str> {
    if text.contains('%') {
        Cow::Owned(text.replace('%', "%%"))
    } else {
        Cow::Borrowed(text)
    }
}

fn assert_finite_f64(caller: &str, name: &str, value: f64) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must be finite"
    );
}

fn assert_finite_color(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} must be finite"
    );
}

fn assert_finite_point(caller: &str, name: &str, value: sys::ImPlotPoint) {
    assert!(
        value.x.is_finite() && value.y.is_finite(),
        "{caller} {name} must be finite"
    );
}

impl PlotScope<'_> {
    /// Check whether the subplot grid containing this plot is hovered.
    pub fn is_subplots_hovered(&self) -> bool {
        self.with_bound_context(|| unsafe {
            !(*sys::ImPlot_GetCurrentContext()).CurrentSubplot.is_null()
                && sys::ImPlot_IsSubplotsHovered()
        })
    }

    /// Check if a legend entry is hovered.
    pub fn is_legend_entry_hovered(&self, label: &str) -> bool {
        let label = if label.contains('\0') { "" } else { label };
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot_IsLegendEntryHovered(ptr)
            })
        })
    }

    /// Get the mouse position in plot coordinates.
    pub fn plot_mouse_position(
        &self,
        y_axis_choice: Option<crate::YAxisChoice>,
    ) -> sys::ImPlotPoint {
        self.mouse_pos(
            crate::IMPLOT_AUTO,
            crate::y_axis_choice_option_to_i32(y_axis_choice),
        )
    }

    /// Get the mouse position in plot coordinates for specific axes.
    pub fn plot_mouse_position_axes(&self, x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotPoint {
        self.mouse_pos(x_axis as _, y_axis as _)
    }

    fn mouse_pos(&self, x: sys::ImAxis, y: sys::ImAxis) -> sys::ImPlotPoint {
        self.with_axes(&[x, y], || unsafe { sys::ImPlot_GetPlotMousePos(x, y) })
    }

    /// Convert pixels using the current X axis and the selected or current Y axis.
    pub fn pixels_to_plot(
        &self,
        pixel_position: [f32; 2],
        y_axis: Option<crate::YAxisChoice>,
    ) -> sys::ImPlotPoint {
        self.convert_pixels(
            pixel_position,
            crate::IMPLOT_AUTO,
            crate::y_axis_choice_option_to_i32(y_axis),
        )
    }

    /// Convert pixels using specific axes.
    pub fn pixels_to_plot_axes(
        &self,
        pixel_position: [f32; 2],
        x_axis: XAxis,
        y_axis: YAxis,
    ) -> sys::ImPlotPoint {
        self.convert_pixels(pixel_position, x_axis as _, y_axis as _)
    }

    fn convert_pixels(&self, pos: [f32; 2], x: sys::ImAxis, y: sys::ImAxis) -> sys::ImPlotPoint {
        assert_finite_vec2("PlotScope::pixels_to_plot()", "pixel_position", pos);
        self.with_axes(&[x, y], || unsafe {
            sys::ImPlot_PixelsToPlot_Float(pos[0], pos[1], x, y)
        })
    }

    /// Convert coordinates using the current X axis and the selected or current Y axis.
    pub fn plot_to_pixels(
        &self,
        pos: sys::ImPlotPoint,
        y_axis: Option<crate::YAxisChoice>,
    ) -> [f32; 2] {
        self.convert_plot_point(
            pos,
            crate::IMPLOT_AUTO,
            crate::y_axis_choice_option_to_i32(y_axis),
        )
    }

    /// Convert coordinates using specific axes.
    pub fn plot_to_pixels_axes(
        &self,
        pos: sys::ImPlotPoint,
        x_axis: XAxis,
        y_axis: YAxis,
    ) -> [f32; 2] {
        self.convert_plot_point(pos, x_axis as _, y_axis as _)
    }

    fn convert_plot_point(
        &self,
        pos: sys::ImPlotPoint,
        x: sys::ImAxis,
        y: sys::ImAxis,
    ) -> [f32; 2] {
        assert_finite_point("PlotScope::plot_to_pixels()", "position", pos);
        self.with_axes(&[x, y], || unsafe {
            let pixels = sys::ImPlot_PlotToPixels_double(pos.x, pos.y, x, y);
            [pixels.x, pixels.y]
        })
    }

    /// Get limits for the selected axes. Each `None` uses the corresponding current axis.
    pub fn plot_limits(
        &self,
        x_axis: Option<XAxis>,
        y_axis: Option<crate::YAxisChoice>,
    ) -> sys::ImPlotRect {
        let x = x_axis.map_or(crate::IMPLOT_AUTO, |axis| axis as _);
        let y = crate::y_axis_choice_option_to_i32(y_axis);
        self.with_axes(&[x, y], || unsafe { sys::ImPlot_GetPlotLimits(x, y) })
    }

    /// Whether a plot has an active selection region.
    pub fn is_plot_selected(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImPlot_IsPlotSelected() })
    }

    /// Get the current plot selection rectangle for specific axes.
    pub fn plot_selection_axes(&self, x_axis: XAxis, y_axis: YAxis) -> Option<sys::ImPlotRect> {
        if !self.is_plot_selected() {
            return None;
        }
        self.with_axes(&[x_axis as _, y_axis as _], || {
            let rect = unsafe { sys::ImPlot_GetPlotSelection(x_axis as i32, y_axis as i32) };
            Some(rect)
        })
    }

    /// Draw a simple round annotation marker at (x,y).
    pub fn annotation_point(
        &self,
        x: f64,
        y: f64,
        color: [f32; 4],
        pixel_offset: [f32; 2],
        clamp: bool,
        round: bool,
    ) {
        assert_finite_f64("PlotScope::annotation_point()", "x", x);
        assert_finite_f64("PlotScope::annotation_point()", "y", y);
        assert_finite_color("PlotScope::annotation_point()", "color", color);
        assert_finite_vec2(
            "PlotScope::annotation_point()",
            "pixel_offset",
            pixel_offset,
        );
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
        self.with_bound_context(|| unsafe {
            sys::ImPlot_Annotation_Bool(x, y, col, off, clamp, round)
        })
    }

    /// Draw a literal text annotation at (x,y).
    ///
    /// The underlying non-variadic C entrypoint still accepts a `printf` format
    /// string, so percent signs are escaped before crossing the FFI boundary.
    pub fn annotation_text(
        &self,
        x: f64,
        y: f64,
        color: [f32; 4],
        pixel_offset: [f32; 2],
        clamp: bool,
        text: &str,
    ) {
        assert_finite_f64("PlotScope::annotation_text()", "x", x);
        assert_finite_f64("PlotScope::annotation_text()", "y", y);
        assert_finite_color("PlotScope::annotation_text()", "color", color);
        assert_finite_vec2("PlotScope::annotation_text()", "pixel_offset", pixel_offset);
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
        assert!(!text.contains('\0'), "text contained NUL");
        let text = literal_printf_format(text);
        self.with_bound_context(|| {
            with_scratch_txt(text.as_ref(), |ptr| unsafe {
                sys::ImPlot_Annotation_Str0(x, y, col, off, clamp, ptr)
            })
        })
    }

    /// Tag the X axis at position x with a tick-like mark.
    pub fn tag_x(&self, x: f64, color: [f32; 4], round: bool) {
        assert_finite_f64("PlotScope::tag_x()", "x", x);
        assert_finite_color("PlotScope::tag_x()", "color", color);
        let col = sys::ImVec4_c {
            x: color[0],
            y: color[1],
            z: color[2],
            w: color[3],
        };
        self.with_bound_context(|| unsafe { sys::ImPlot_TagX_Bool(x, col, round) })
    }

    /// Tag the X axis at position x with a literal text label.
    pub fn tag_x_text(&self, x: f64, color: [f32; 4], text: &str) {
        assert_finite_f64("PlotScope::tag_x_text()", "x", x);
        assert_finite_color("PlotScope::tag_x_text()", "color", color);
        let col = sys::ImVec4_c {
            x: color[0],
            y: color[1],
            z: color[2],
            w: color[3],
        };
        assert!(!text.contains('\0'), "text contained NUL");
        let text = literal_printf_format(text);
        self.with_bound_context(|| {
            with_scratch_txt(text.as_ref(), |ptr| unsafe {
                sys::ImPlot_TagX_Str0(x, col, ptr)
            })
        })
    }

    /// Tag the Y axis at position y with a tick-like mark.
    pub fn tag_y(&self, y: f64, color: [f32; 4], round: bool) {
        assert_finite_f64("PlotScope::tag_y()", "y", y);
        assert_finite_color("PlotScope::tag_y()", "color", color);
        let col = sys::ImVec4_c {
            x: color[0],
            y: color[1],
            z: color[2],
            w: color[3],
        };
        self.with_bound_context(|| unsafe { sys::ImPlot_TagY_Bool(y, col, round) })
    }

    /// Tag the Y axis at position y with a literal text label.
    pub fn tag_y_text(&self, y: f64, color: [f32; 4], text: &str) {
        assert_finite_f64("PlotScope::tag_y_text()", "y", y);
        assert_finite_color("PlotScope::tag_y_text()", "color", color);
        let col = sys::ImVec4_c {
            x: color[0],
            y: color[1],
            z: color[2],
            w: color[3],
        };
        assert!(!text.contains('\0'), "text contained NUL");
        let text = literal_printf_format(text);
        self.with_bound_context(|| {
            with_scratch_txt(text.as_ref(), |ptr| unsafe {
                sys::ImPlot_TagY_Str0(y, col, ptr)
            })
        })
    }

    /// Get the current plot limits for specific axes.
    pub fn plot_limits_axes(&self, x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotRect {
        self.with_axes(&[x_axis as _, y_axis as _], || unsafe {
            sys::ImPlot_GetPlotLimits(x_axis as i32, y_axis as i32)
        })
    }

    /// Check if an enabled axis is hovered. Disabled axes return `false`.
    pub fn is_axis_hovered(&self, axis: Axis) -> bool {
        // SAFETY: Axis only represents the six native axes.
        unsafe { self.is_axis_hovered_unchecked(axis.to_sys()) }
    }

    /// Check if a raw axis is hovered.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the current plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn is_axis_hovered_unchecked(&self, axis: sys::ImAxis) -> bool {
        self.with_bound_context(|| unsafe {
            (*sys::ImPlot_GetCurrentPlot()).Axes[axis as usize].Enabled
                && sys::ImPlot_IsAxisHovered(axis)
        })
    }

    /// Check if the X axis is hovered.
    pub fn is_plot_x_axis_hovered(&self) -> bool {
        self.with_bound_context(|| unsafe {
            sys::ImPlot_IsAxisHovered((*sys::ImPlot_GetCurrentPlot()).CurrentX)
        })
    }

    /// Check if a specific X axis is hovered.
    pub fn is_plot_x_axis_hovered_axis(&self, x_axis: XAxis) -> bool {
        self.is_axis_hovered(x_axis.into())
    }

    /// Check if a Y axis is hovered.
    pub fn is_plot_y_axis_hovered(&self, y_axis_choice: Option<crate::YAxisChoice>) -> bool {
        let axis = match y_axis_choice {
            None => {
                return self.with_bound_context(|| unsafe {
                    let current = (*sys::ImPlot_GetCurrentPlot()).CurrentY;
                    sys::ImPlot_IsAxisHovered(current)
                });
            }
            Some(crate::YAxisChoice::First) => Axis::Y1,
            Some(crate::YAxisChoice::Second) => Axis::Y2,
            Some(crate::YAxisChoice::Third) => Axis::Y3,
        };
        self.is_axis_hovered(axis)
    }

    /// Check if a specific Y axis is hovered.
    pub fn is_plot_y_axis_hovered_axis(&self, y_axis: YAxis) -> bool {
        self.is_axis_hovered(y_axis.into())
    }

    /// Get current plot position (top-left) in pixels.
    pub fn plot_pos(&self) -> [f32; 2] {
        self.with_bound_context(|| {
            let out = unsafe { sys::ImPlot_GetPlotPos() };
            [out.x, out.y]
        })
    }

    /// Get current plot size in pixels.
    pub fn plot_size(&self) -> [f32; 2] {
        self.with_bound_context(|| {
            let out = unsafe { sys::ImPlot_GetPlotSize() };
            [out.x, out.y]
        })
    }
}

impl PlotUi<'_> {
    /// Check if any subplots area is hovered.
    pub fn is_subplots_hovered(&self) -> bool {
        assert!(self.subplot.is_some(), "subplot hover requires a grid");
        self.with_bound_context(|| unsafe { sys::ImPlot_IsSubplotsHovered() })
    }

    /// Show the native ImPlot demo when the `demo` feature is enabled.
    pub fn show_demo_window(&self, show: &mut bool) {
        self.with_idle_context(|| {
            #[cfg(feature = "demo")]
            unsafe {
                sys::ImPlot_ShowDemoWindow(show)
            };
            #[cfg(not(feature = "demo"))]
            let _ = show;
        });
    }

    /// Show the built-in user guide for ImPlot.
    pub fn show_user_guide(&self) {
        self.with_bound_context(|| unsafe { sys::ImPlot_ShowUserGuide() })
    }

    /// Show the native metrics inspector outside any plot or subplot grid.
    pub fn show_metrics_window(&self, open: &mut bool) {
        self.with_idle_context(|| unsafe { sys::ImPlot_ShowMetricsWindow(open as *mut bool) })
    }
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

/// Stable identity for ImPlot drag point/line helpers.
///
/// This wraps the upstream `int id` parameter so safe Rust callers do not
/// confuse tool identities with coordinates, counts, or other signed values.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct DragToolId(i32);

impl DragToolId {
    /// Create a drag tool identity from a raw signed id.
    #[inline]
    pub const fn new(id: i32) -> Self {
        Self(id)
    }

    /// Return the raw `int` value expected by the ImPlot FFI.
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

impl From<i32> for DragToolId {
    #[inline]
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<DragToolId> for i32 {
    #[inline]
    fn from(value: DragToolId) -> Self {
        value.raw()
    }
}

impl fmt::Display for DragToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn color4(rgba: [f32; 4]) -> sys::ImVec4_c {
    sys::ImVec4_c {
        x: rgba[0],
        y: rgba[1],
        z: rgba[2],
        w: rgba[3],
    }
}

impl PlotScope<'_> {
    /// Draggable point with result flags.
    pub fn drag_point(
        &self,
        id: DragToolId,
        x: &mut f64,
        y: &mut f64,
        color: [f32; 4],
        size: f32,
        flags: crate::DragToolFlags,
    ) -> DragResult {
        let mut clicked = false;
        let mut hovered = false;
        let mut held = false;
        self.with_bound_context(|| {
            let changed = unsafe {
                sys::ImPlot_DragPoint(
                    id.raw(),
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
        })
    }

    /// Draggable vertical line at x.
    pub fn drag_line_x(
        &self,
        id: DragToolId,
        x: &mut f64,
        color: [f32; 4],
        thickness: f32,
        flags: crate::DragToolFlags,
    ) -> DragResult {
        let mut clicked = false;
        let mut hovered = false;
        let mut held = false;
        self.with_bound_context(|| {
            let changed = unsafe {
                sys::ImPlot_DragLineX(
                    id.raw(),
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
        })
    }

    /// Draggable horizontal line at y.
    pub fn drag_line_y(
        &self,
        id: DragToolId,
        y: &mut f64,
        color: [f32; 4],
        thickness: f32,
        flags: crate::DragToolFlags,
    ) -> DragResult {
        let mut clicked = false;
        let mut hovered = false;
        let mut held = false;
        self.with_bound_context(|| {
            let changed = unsafe {
                sys::ImPlot_DragLineY(
                    id.raw(),
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{DragToolId, literal_printf_format};
    use std::borrow::Cow;

    #[test]
    fn literal_plot_text_borrows_strings_without_percent_signs() {
        let text = literal_printf_format("ordinary label");

        assert!(matches!(text, Cow::Borrowed("ordinary label")));
    }

    #[test]
    fn literal_plot_text_escapes_every_printf_directive() {
        let text = literal_printf_format("100% complete: %s %n %%");

        assert_eq!(text, "100%% complete: %%s %%n %%%%");
        assert!(matches!(text, Cow::Owned(_)));
    }

    #[test]
    fn drag_tool_id_round_trips_raw_values() {
        let id = DragToolId::new(-7);
        assert_eq!(id.raw(), -7);
        assert_eq!(i32::from(id), -7);

        let other = DragToolId::from(120482);
        assert_eq!(other.raw(), 120482);
        assert_eq!(other.to_string(), "120482");
    }
}
