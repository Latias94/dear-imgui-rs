use super::core::PlotContext;
use super::scope::{ActivePlot, PlotScope, PlotSetup};
use super::validation::assert_finite_vec2;
use crate::{XAxis, YAxis, sys};
use dear_imgui_rs::{NativeWindowScope, Ui, with_scratch_txt};

/// A plotting context bound to its Dear ImGui frame, or to a cell in a subplot grid.
pub struct PlotUi<'ui> {
    pub(crate) context: &'ui PlotContext,
    pub(crate) ui: &'ui Ui,
    pub(crate) subplot: Option<&'ui NativeWindowScope<'ui>>,
}

impl PlotUi<'_> {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.context.with_ui(self.ui, || {
            let ctx = unsafe { sys::ImPlot_GetCurrentContext() };
            assert!(
                unsafe { (*ctx).CurrentPlot.is_null() },
                "this operation requires no active plot"
            );
            assert_eq!(
                self.subplot.is_some(),
                unsafe { !(*ctx).CurrentSubplot.is_null() },
                "use the PlotUi supplied by the current subplot grid"
            );
            if let Some(host) = self.subplot {
                assert!(
                    host.is_current(),
                    "subplot requires its original window and table cell"
                );
            }
            f()
        })
    }

    pub(crate) fn with_idle_context<R>(&self, f: impl FnOnce() -> R) -> R {
        assert!(
            self.subplot.is_none(),
            "this operation requires a context outside a subplot grid"
        );
        self.with_bound_context(f)
    }

    /// Draw a plot using its default size. Returns `None` for a skipped plot or a full grid.
    pub fn with_plot<R>(&self, title: &str, draw: impl FnOnce(&PlotScope<'_>) -> R) -> Option<R> {
        self.with_plot_size(title, [-1.0, 0.0], draw)
    }

    /// Draw a plot using a custom size.
    pub fn with_plot_size<R>(
        &self,
        title: &str,
        size: [f32; 2],
        draw: impl FnOnce(&PlotScope<'_>) -> R,
    ) -> Option<R> {
        self.with_plot_setup(title, size, |_| (), draw)
    }

    /// Configure axes and the legend before drawing. Both closures are skipped for a clipped plot.
    ///
    /// ImPlot pushes its item ID when setup finishes. The setup closure's window, table and ID
    /// scopes are balanced before that push. Drawing scopes are balanced before `EndPlot`.
    ///
    /// ```compile_fail
    /// use dear_implot::PlotUi;
    /// fn escape(ui: &PlotUi<'_>) {
    ///     let _plot = ui.with_plot("plot", |plot| plot);
    /// }
    /// ```
    pub fn with_plot_setup<R>(
        &self,
        title: &str,
        size: [f32; 2],
        setup: impl FnOnce(&mut PlotSetup<'_>),
        draw: impl FnOnce(&PlotScope<'_>) -> R,
    ) -> Option<R> {
        assert_finite_vec2("PlotUi::with_plot_setup()", "size", size);
        if title.contains('\0') {
            return None;
        }
        let has_cell = self.with_bound_context(|| unsafe {
            let grid = (*sys::ImPlot_GetCurrentContext()).CurrentSubplot;
            grid.is_null() || (*grid).CurrentIdx < (*grid).Rows * (*grid).Cols
        });
        if !has_cell {
            return None;
        }
        self.ui.with_native_window_scope(|host| {
            let size = sys::ImVec2_c {
                x: size[0],
                y: size[1],
            };
            let raw = self
                .context
                .with_bound_context("dear-implot: BeginPlot", || {
                    with_scratch_txt(title, |ptr| unsafe {
                        sys::ImPlot_BeginPlot(ptr, size, 0).then(|| sys::ImPlot_GetCurrentPlot())
                    })
                })?;
            let mut active = ActivePlot::new(self, host, raw);
            host.run(|| setup(&mut active.setup()));
            active
                .plot
                .with_bound_context(|| unsafe { sys::ImPlot_SetupFinish() });
            Some(host.run(|| draw(&active.plot)))
        })
    }
}

impl PlotScope<'_> {
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
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                let spec = crate::plots::plot_spec_from(
                    self.next_spec.take(),
                    0,
                    crate::plots::PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotLine_doublePtrdoublePtr(
                    ptr,
                    x_data.as_ptr(),
                    y_data.as_ptr(),
                    count,
                    spec,
                );
            })
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
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                let spec = crate::plots::plot_spec_from(
                    self.next_spec.take(),
                    0,
                    crate::plots::PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotScatter_doublePtrdoublePtr(
                    ptr,
                    x_data.as_ptr(),
                    y_data.as_ptr(),
                    count,
                    spec,
                );
            })
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
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                let spec = crate::plots::plot_spec_from(
                    self.next_spec.take(),
                    0,
                    crate::plots::PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotPolygon_doublePtr(
                    ptr,
                    x_data.as_ptr(),
                    y_data.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Check if the plot area is hovered
    pub fn is_plot_hovered(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImPlot_IsPlotHovered() })
    }

    /// Get the mouse position in plot coordinates
    pub fn get_plot_mouse_pos(&self, y_axis: Option<crate::YAxisChoice>) -> sys::ImPlotPoint {
        self.plot_mouse_position(y_axis)
    }

    /// Get the mouse position in plot coordinates for specific axes
    pub fn get_plot_mouse_pos_axes(&self, x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotPoint {
        self.plot_mouse_position_axes(x_axis, y_axis)
    }

    /// Set current axes for subsequent plot submissions
    pub fn set_axes(&self, x_axis: XAxis, y_axis: YAxis) {
        self.with_axes(&[x_axis as _, y_axis as _], || unsafe {
            sys::ImPlot_SetAxes(x_axis as i32, y_axis as i32)
        })
    }
}
