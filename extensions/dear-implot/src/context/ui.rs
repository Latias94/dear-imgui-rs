use super::core::PlotContext;
use super::token::PlotToken;
use super::validation::assert_finite_vec2;
use crate::{XAxis, YAxis, sys};
use dear_imgui_rs::{Ui, with_scratch_txt};

/// A temporary reference for building plots
///
/// This struct ensures that plots can only be created when both ImGui and ImPlot
/// contexts are available and properly set up.
pub struct PlotUi<'ui> {
    #[allow(dead_code)]
    pub(crate) context: &'ui PlotContext,
    #[allow(dead_code)]
    pub(crate) ui: &'ui Ui,
}

impl<'ui> PlotUi<'ui> {
    #[inline]
    pub(crate) fn bind(&self) {
        self.context.assert_imgui_alive();
        self.context.binding().bind("dear-implot: PlotUi");
    }

    /// Begin a new plot with the given title
    ///
    /// Returns a PlotToken if the plot was successfully started.
    /// The plot will be automatically ended when the token is dropped.
    pub fn begin_plot(&self, title: &str) -> Option<PlotToken<'_>> {
        let size = sys::ImVec2_c { x: -1.0, y: 0.0 };
        if title.contains('\0') {
            return None;
        }
        self.bind();
        let started = with_scratch_txt(title, |ptr| unsafe { sys::ImPlot_BeginPlot(ptr, size, 0) });

        if started {
            Some(PlotToken::new(
                self.context.binding(),
                self.context.imgui_alive_token(),
            ))
        } else {
            None
        }
    }

    /// Begin a plot with custom size
    pub fn begin_plot_with_size(&self, title: &str, size: [f32; 2]) -> Option<PlotToken<'_>> {
        assert_finite_vec2("PlotUi::begin_plot_with_size()", "size", size);
        let plot_size = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        if title.contains('\0') {
            return None;
        }
        self.bind();
        let started = with_scratch_txt(title, |ptr| unsafe {
            sys::ImPlot_BeginPlot(ptr, plot_size, 0)
        });

        if started {
            Some(PlotToken::new(
                self.context.binding(),
                self.context.imgui_alive_token(),
            ))
        } else {
            None
        }
    }

    /// Plot a line with the given label and data
    ///
    /// This is a convenience method that can be called within a plot.
    pub fn plot_line(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }
        let count = match i32::try_from(x_data.len()) {
            Ok(v) => v,
            Err(_) => return,
        };

        let label = if label.contains('\0') { "" } else { label };
        self.bind();
        with_scratch_txt(label, |ptr| unsafe {
            let spec = crate::plots::plot_spec_from(0, crate::plots::PlotDataLayout::DEFAULT);
            sys::ImPlot_PlotLine_doublePtrdoublePtr(
                ptr,
                x_data.as_ptr(),
                y_data.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Plot a scatter plot with the given label and data
    pub fn plot_scatter(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }
        let count = match i32::try_from(x_data.len()) {
            Ok(v) => v,
            Err(_) => return,
        };

        let label = if label.contains('\0') { "" } else { label };
        self.bind();
        with_scratch_txt(label, |ptr| unsafe {
            let spec = crate::plots::plot_spec_from(0, crate::plots::PlotDataLayout::DEFAULT);
            sys::ImPlot_PlotScatter_doublePtrdoublePtr(
                ptr,
                x_data.as_ptr(),
                y_data.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Plot a polygon with the given label and vertex data.
    pub fn plot_polygon(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return;
        }
        let count = match i32::try_from(x_data.len()) {
            Ok(v) => v,
            Err(_) => return,
        };

        let label = if label.contains('\0') { "" } else { label };
        self.bind();
        with_scratch_txt(label, |ptr| unsafe {
            let spec = crate::plots::plot_spec_from(0, crate::plots::PlotDataLayout::DEFAULT);
            sys::ImPlot_PlotPolygon_doublePtr(ptr, x_data.as_ptr(), y_data.as_ptr(), count, spec);
        })
    }

    /// Check if the plot area is hovered
    pub fn is_plot_hovered(&self) -> bool {
        self.bind();
        unsafe { sys::ImPlot_IsPlotHovered() }
    }

    /// Get the mouse position in plot coordinates
    pub fn get_plot_mouse_pos(&self, y_axis: Option<crate::YAxisChoice>) -> sys::ImPlotPoint {
        let y_axis_i32 = crate::y_axis_choice_option_to_i32(y_axis);
        let y_axis = match y_axis_i32 {
            0 => 3,
            1 => 4,
            2 => 5,
            _ => 3,
        };
        self.bind();
        unsafe { sys::ImPlot_GetPlotMousePos(0, y_axis) }
    }

    /// Get the mouse position in plot coordinates for specific axes
    pub fn get_plot_mouse_pos_axes(&self, x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotPoint {
        self.bind();
        unsafe { sys::ImPlot_GetPlotMousePos(x_axis as i32, y_axis as i32) }
    }

    /// Set current axes for subsequent plot submissions
    pub fn set_axes(&self, x_axis: XAxis, y_axis: YAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetAxes(x_axis as i32, y_axis as i32) }
    }
}
