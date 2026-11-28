use super::validation::{
    assert_axis_constraint_range, assert_axis_limit_range, assert_axis_zoom_range,
    assert_finite_f64_slice, axis_tick_count_to_i32, is_valid_axis_range,
};
use super::{PlotSetup, PlotUi};
use crate::{Axis, AxisFlags, AxisFormat, AxisScale, PlotCond, XAxis, YAxis, sys};
use dear_imgui_rs::with_scratch_txt;
use std::cell::Cell;
use std::os::raw::c_char;

impl<'links> PlotSetup<'_, 'links> {
    /// Enable and label an X axis.
    pub fn setup_x_axis(&self, axis: XAxis, label: Option<&str>, flags: AxisFlags) {
        self.setup_axis(axis.into(), label, flags);
    }

    /// Enable and label a Y axis.
    pub fn setup_y_axis(&self, axis: YAxis, label: Option<&str>, flags: AxisFlags) {
        self.setup_axis(axis.into(), label, flags);
    }

    fn setup_axis(&self, axis: Axis, label: Option<&str>, flags: AxisFlags) {
        let idx = axis as usize;
        self.plot.with_bound_context(|| unsafe {
            let enabling = !(*sys::ImPlot_GetCurrentPlot()).Axes[idx].Enabled;
            let configure = |label| sys::ImPlot_SetupAxis(axis.to_sys(), label, flags.bits() as _);
            match label.filter(|label| !label.contains('\0')) {
                Some(label) => with_scratch_txt(label, configure),
                None => configure(std::ptr::null()),
            }
            // BeginPlot applies next settings before secondary axes can be enabled.
            // Apply their pending range once when the axis is enabled.
            if enabling {
                let next = &raw const (*sys::ImPlot_GetCurrentContext()).NextPlotData;
                if (*next).HasRange[idx] {
                    let range = (*next).Range[idx];
                    sys::ImPlot_SetupAxisLimits(
                        axis.to_sys(),
                        range.Min,
                        range.Max,
                        (*next).RangeCond[idx],
                    );
                }
            }
        });
    }

    /// Setup axis limits for a specific X axis
    pub fn setup_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotSetup::setup_x_axis_limits()", min, max);
        self.plot.with_axes(&[axis as sys::ImAxis], || unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        })
    }

    /// Setup axis limits for a specific Y axis
    pub fn setup_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotSetup::setup_y_axis_limits()", min, max);
        self.plot.with_axes(&[axis as sys::ImAxis], || unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        })
    }

    /// Link an enabled axis to external min/max cells for this plot.
    ///
    /// The cells must outlive the entire [`PlotUi::with_plot_setup`] call. Their values are
    /// read during setup and the final valid limits are written back after `EndPlot`, even
    /// on unwind. ImPlot never receives the cells' addresses, so edits to the cells during
    /// drawing do not affect the current plot and can be overwritten by writeback.
    ///
    /// This replaces the axis's existing links, including any automatic subplot links.
    /// `None` or a non-finite value leaves that endpoint unlinked. If the new limits would
    /// not make an increasing range with finite width, neither endpoint is linked and the
    /// current limits are unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the axis is not enabled.
    ///
    /// ```compile_fail
    /// use dear_implot::{Axis, PlotUi};
    /// use std::cell::Cell;
    ///
    /// fn link_local(plots: &PlotUi<'_>) {
    ///     plots.with_plot_setup("linked", [240.0, 180.0], |setup| {
    ///         let min = Cell::new(0.0);
    ///         setup.setup_axis_links(Axis::X1, Some(&min), None);
    ///     }, |_| ());
    /// }
    /// ```
    pub fn setup_axis_links(
        &mut self,
        axis: Axis,
        link_min: Option<&'links Cell<f64>>,
        link_max: Option<&'links Cell<f64>>,
    ) {
        let index = axis as usize;
        let link_min = link_min.filter(|cell| cell.get().is_finite());
        let link_max = link_max.filter(|cell| cell.get().is_finite());
        self.axis_links[index] = self.plot.with_axes(&[axis.to_sys()], || unsafe {
            sys::ImPlot_SetupAxisLinks(axis.to_sys(), std::ptr::null_mut(), std::ptr::null_mut());
            if link_min.is_none() && link_max.is_none() {
                return (None, None);
            }

            let native_axis = &raw mut (*sys::ImPlot_GetCurrentPlot()).Axes[index];
            let range = (*native_axis).Range;
            let min = link_min.map_or(range.Min, Cell::get);
            let max = link_max.map_or(range.Max, Cell::get);
            if !is_valid_axis_range(min, max) {
                return (None, None);
            }

            sys::ImPlotAxis_SetRange_double(native_axis, min, max);
            (link_min, link_max)
        });
    }

    /// Setup both axes labels/flags at once
    pub fn setup_axes(
        &self,
        x_label: Option<&str>,
        y_label: Option<&str>,
        x_flags: AxisFlags,
        y_flags: AxisFlags,
    ) {
        self.setup_x_axis(XAxis::X1, x_label, x_flags);
        self.setup_y_axis(YAxis::Y1, y_label, y_flags);
    }

    /// Setup axes limits (both) at once
    pub fn setup_axes_limits(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        cond: PlotCond,
    ) {
        assert_axis_limit_range("PlotSetup::setup_axes_limits() x axis", x_min, x_max);
        assert_axis_limit_range("PlotSetup::setup_axes_limits() y axis", y_min, y_max);
        self.plot.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond)
        })
    }

    /// Set explicit X tick positions. If supplied, labels must match the positions in length.
    pub fn setup_x_axis_ticks_positions(
        &self,
        axis: XAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        self.ticks_positions(axis.into(), values, labels, keep_default);
    }

    /// Set explicit Y tick positions. ImPlot copies labels into its ticker during setup.
    pub fn setup_y_axis_ticks_positions(
        &self,
        axis: YAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        self.ticks_positions(axis.into(), values, labels, keep_default);
    }

    fn ticks_positions(
        &self,
        axis: Axis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_finite_f64_slice("axis tick positions", "values", values);
        let Ok(count) = i32::try_from(values.len()) else {
            return;
        };
        self.plot.with_axes(&[axis.to_sys()], || {
            with_tick_labels(labels, values.len(), |labels| unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis.to_sys(),
                    values.as_ptr(),
                    count,
                    labels,
                    keep_default,
                )
            });
        });
    }

    /// Set X ticks across a range. Labeled ranges require at least two ticks.
    pub fn setup_x_axis_ticks_range(
        &self,
        axis: XAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        self.ticks_range(axis.into(), v_min, v_max, n_ticks, labels, keep_default);
    }

    /// Set Y ticks across a range. Labeled ranges require at least two ticks.
    pub fn setup_y_axis_ticks_range(
        &self,
        axis: YAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        self.ticks_range(axis.into(), v_min, v_max, n_ticks, labels, keep_default);
    }

    fn ticks_range(
        &self,
        axis: Axis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_axis_limit_range("axis tick range", v_min, v_max);
        let count = axis_tick_count_to_i32("axis tick range", n_ticks, labels.is_some());
        self.plot.with_axes(&[axis.to_sys()], || {
            with_tick_labels(labels, n_ticks, |labels| unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis.to_sys(),
                    v_min,
                    v_max,
                    count,
                    labels,
                    keep_default,
                )
            });
        });
    }

    /// Set a validated X tick format. Replaces a custom formatter registered on this axis.
    pub fn setup_x_axis_format(&self, axis: XAxis, format: &AxisFormat<'_>) {
        self.format(axis.into(), format);
    }

    /// Set a validated Y tick format. Replaces a custom formatter registered on this axis.
    pub fn setup_y_axis_format(&self, axis: YAxis, format: &AxisFormat<'_>) {
        self.format(axis.into(), format);
    }

    fn format(&self, axis: Axis, format: &AxisFormat<'_>) {
        self.plot.with_axes(&[axis.to_sys()], || {
            with_scratch_txt(format.as_str(), |ptr| unsafe {
                // The string overload only writes the axis's 16-byte FormatSpec.
                sys::ImPlot_SetupAxisFormat_PlotFormatter(
                    axis.to_sys(),
                    None,
                    std::ptr::null_mut(),
                );
                sys::ImPlot_SetupAxisFormat_Str(axis.to_sys(), ptr);
            });
        });
    }

    /// Set the built-in scale for an enabled X axis.
    pub fn setup_x_axis_scale(&self, axis: XAxis, scale: AxisScale) {
        self.scale(axis.into(), scale);
    }

    /// Set the built-in scale for an enabled Y axis.
    ///
    /// # Panics
    ///
    /// ImPlot requires a horizontal axis for [`AxisScale::Time`].
    pub fn setup_y_axis_scale(&self, axis: YAxis, scale: AxisScale) {
        assert_ne!(
            scale,
            AxisScale::Time,
            "time ticks require a horizontal axis"
        );
        self.scale(axis.into(), scale);
    }

    fn scale(&self, axis: Axis, scale: AxisScale) {
        self.plot.with_axes(&[axis.to_sys()], || unsafe {
            sys::ImPlot_SetupAxisScale_PlotScale(axis.to_sys(), scale as sys::ImPlotScale)
        });
    }

    /// Setup axis limits constraints
    pub fn setup_axis_limits_constraints(&self, axis: Axis, v_min: f64, v_max: f64) {
        assert_axis_constraint_range("PlotSetup::setup_axis_limits_constraints()", v_min, v_max);
        self.plot.with_axes(&[axis.to_sys()], || unsafe {
            sys::ImPlot_SetupAxisLimitsConstraints(axis.to_sys(), v_min, v_max)
        })
    }

    /// Setup raw axis limits constraints.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn setup_axis_limits_constraints_unchecked(
        &self,
        axis: sys::ImAxis,
        v_min: f64,
        v_max: f64,
    ) {
        assert_axis_constraint_range(
            "PlotSetup::setup_axis_limits_constraints_unchecked()",
            v_min,
            v_max,
        );
        self.plot.with_axes(&[axis], || unsafe {
            sys::ImPlot_SetupAxisLimitsConstraints(axis, v_min, v_max)
        })
    }

    /// Setup axis zoom constraints
    pub fn setup_axis_zoom_constraints(&self, axis: Axis, z_min: f64, z_max: f64) {
        assert_axis_zoom_range("PlotSetup::setup_axis_zoom_constraints()", z_min, z_max);
        self.plot.with_axes(&[axis.to_sys()], || unsafe {
            sys::ImPlot_SetupAxisZoomConstraints(axis.to_sys(), z_min, z_max)
        })
    }

    /// Setup raw axis zoom constraints.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn setup_axis_zoom_constraints_unchecked(
        &self,
        axis: sys::ImAxis,
        z_min: f64,
        z_max: f64,
    ) {
        assert_axis_zoom_range(
            "PlotSetup::setup_axis_zoom_constraints_unchecked()",
            z_min,
            z_max,
        );
        self.plot.with_axes(&[axis], || unsafe {
            sys::ImPlot_SetupAxisZoomConstraints(axis, z_min, z_max)
        })
    }
}

// AddTicksCustom copies the labels synchronously. With no labels it invokes the current
// formatter instead. Keep the null pointer distinct from an array of empty strings.
fn with_tick_labels(
    labels: Option<&[&str]>,
    count: usize,
    apply: impl FnOnce(*const *const c_char),
) {
    match labels {
        Some(labels) if labels.len() == count => {
            crate::plots::with_plot_str_slice(labels, |ptrs| apply(ptrs.as_ptr()))
        }
        Some(_) => (),
        None => apply(std::ptr::null()),
    }
}

impl PlotUi<'_> {
    /// Set next frame limits for a specific axis
    pub fn set_next_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::set_next_x_axis_limits()", min, max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        })
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::set_next_y_axis_limits()", min, max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        })
    }

    /// Set limits for both axes on the next plot
    pub fn set_next_axes_limits(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        cond: PlotCond,
    ) {
        assert_axis_limit_range("PlotUi::set_next_axes_limits() x axis", x_min, x_max);
        assert_axis_limit_range("PlotUi::set_next_axes_limits() y axis", y_min, y_max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetNextAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond)
        })
    }

    /// Fit both axes on the next plot
    pub fn set_next_axes_to_fit(&self) {
        self.with_bound_context(|| unsafe { sys::ImPlot_SetNextAxesToFit() })
    }

    /// Fit a specific axis on the next plot
    pub fn set_next_axis_to_fit(&self, axis: Axis) {
        self.with_bound_context(|| unsafe { sys::ImPlot_SetNextAxisToFit(axis.to_sys()) })
    }

    /// Fit a raw axis on the next plot.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value. Passing an out-of-range
    /// value lets ImPlot index internal next-plot arrays out of bounds.
    pub unsafe fn set_next_axis_to_fit_unchecked(&self, axis: sys::ImAxis) {
        self.with_bound_context(|| unsafe { sys::ImPlot_SetNextAxisToFit(axis) })
    }

    /// Fit a specific X axis on the next plot
    pub fn set_next_x_axis_to_fit(&self, axis: XAxis) {
        self.with_bound_context(|| unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) })
    }

    /// Fit a specific Y axis on the next plot
    pub fn set_next_y_axis_to_fit(&self, axis: YAxis) {
        self.with_bound_context(|| unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) })
    }
}
