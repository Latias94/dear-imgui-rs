use crate::types::Vec2;
use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Plot widgets
///
/// This module contains all plotting-related UI components like line plots and histograms.
use std::ffi::CString;

/// # Widgets: Plots
impl<'frame> Ui<'frame> {
    /// Plot lines
    ///
    /// Display a line plot with the given data points.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let values = [0.6, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2];
    /// ui.plot_lines("Frame Times", &values, Vec2::new(0.0, 80.0));
    /// # });
    /// ```
    pub fn plot_lines(&mut self, label: impl AsRef<str>, values: &[f32], graph_size: Vec2) {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let size_vec = sys::ImVec2 {
            x: graph_size.x,
            y: graph_size.y,
        };

        unsafe {
            sys::ImGui_PlotLines(
                c_label.as_ptr(),
                values.as_ptr(),
                values.len() as i32,
                0,                // values_offset
                std::ptr::null(), // overlay_text
                f32::MAX,         // scale_min (auto)
                f32::MAX,         // scale_max (auto)
                size_vec,
                std::mem::size_of::<f32>() as i32, // stride
            );
        }
    }

    /// Plot lines with custom parameters
    ///
    /// Display a line plot with custom scale and overlay text.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let values = [0.6, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2];
    /// ui.plot_lines_ex(
    ///     "Frame Times",
    ///     &values,
    ///     0,
    ///     Some("avg 0.6"),
    ///     0.0,
    ///     1.0,
    ///     Vec2::new(0.0, 80.0)
    /// );
    /// # });
    /// ```
    pub fn plot_lines_ex(
        &mut self,
        label: impl AsRef<str>,
        values: &[f32],
        values_offset: i32,
        overlay_text: Option<&str>,
        scale_min: f32,
        scale_max: f32,
        graph_size: Vec2,
    ) {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let c_overlay = overlay_text
            .map(|s| CString::new(s).unwrap_or_default())
            .unwrap_or_default();
        let overlay_ptr = if overlay_text.is_some() {
            c_overlay.as_ptr()
        } else {
            std::ptr::null()
        };
        let size_vec = sys::ImVec2 {
            x: graph_size.x,
            y: graph_size.y,
        };

        unsafe {
            sys::ImGui_PlotLines(
                c_label.as_ptr(),
                values.as_ptr(),
                values.len() as i32,
                values_offset,
                overlay_ptr,
                scale_min,
                scale_max,
                size_vec,
                std::mem::size_of::<f32>() as i32, // stride
            );
        }
    }

    /// Plot histogram
    ///
    /// Display a histogram with the given data points.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let values = [0.6, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2];
    /// ui.plot_histogram("Histogram", &values, Vec2::new(0.0, 80.0));
    /// # });
    /// ```
    pub fn plot_histogram(&mut self, label: impl AsRef<str>, values: &[f32], graph_size: Vec2) {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let size_vec = sys::ImVec2 {
            x: graph_size.x,
            y: graph_size.y,
        };

        unsafe {
            sys::ImGui_PlotHistogram(
                c_label.as_ptr(),
                values.as_ptr(),
                values.len() as i32,
                0,                // values_offset
                std::ptr::null(), // overlay_text
                f32::MAX,         // scale_min (auto)
                f32::MAX,         // scale_max (auto)
                size_vec,
                std::mem::size_of::<f32>() as i32, // stride
            );
        }
    }

    /// Plot histogram with custom parameters
    ///
    /// Display a histogram with custom scale and overlay text.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let values = [0.6, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2];
    /// ui.plot_histogram_ex(
    ///     "Histogram",
    ///     &values,
    ///     0,
    ///     Some("avg 0.6"),
    ///     0.0,
    ///     1.0,
    ///     Vec2::new(0.0, 80.0)
    /// );
    /// # });
    /// ```
    pub fn plot_histogram_ex(
        &mut self,
        label: impl AsRef<str>,
        values: &[f32],
        values_offset: i32,
        overlay_text: Option<&str>,
        scale_min: f32,
        scale_max: f32,
        graph_size: Vec2,
    ) {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let c_overlay = overlay_text
            .map(|s| CString::new(s).unwrap_or_default())
            .unwrap_or_default();
        let overlay_ptr = if overlay_text.is_some() {
            c_overlay.as_ptr()
        } else {
            std::ptr::null()
        };
        let size_vec = sys::ImVec2 {
            x: graph_size.x,
            y: graph_size.y,
        };

        unsafe {
            sys::ImGui_PlotHistogram(
                c_label.as_ptr(),
                values.as_ptr(),
                values.len() as i32,
                values_offset,
                overlay_ptr,
                scale_min,
                scale_max,
                size_vec,
                std::mem::size_of::<f32>() as i32, // stride
            );
        }
    }
}
