use super::validation::{
    assert_axis_constraint_range, assert_axis_limit_range, assert_axis_zoom_range,
    assert_finite_f64_slice, axis_tick_count_to_i32,
};
use super::{PlotSetup, PlotUi};
use crate::{Axis, AxisFlags, AxisFormat, AxisScale, PlotCond, XAxis, YAxis, sys};
use dear_imgui_rs::with_scratch_txt;
use std::os::raw::c_char;

impl PlotSetup<'_> {
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
            // BeginPlot applies next settings before secondary axes can be enabled. Apply
            // their pending links and range once, in the same order as ApplyNextPlotData.
            if enabling {
                let next = &(*sys::ImPlot_GetCurrentContext()).NextPlotData;
                sys::ImPlot_SetupAxisLinks(axis.to_sys(), next.LinkedMin[idx], next.LinkedMax[idx]);
                if next.HasRange[idx] {
                    let range = next.Range[idx];
                    sys::ImPlot_SetupAxisLimits(
                        axis.to_sys(),
                        range.Min,
                        range.Max,
                        next.RangeCond[idx],
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

    /// Link an enabled axis to external min/max values for the active plot.
    ///
    /// # Safety
    ///
    /// The axis must be enabled. Keep each value initialized at a stable, distinct address
    /// until this plot ends, including unwinding. Do not otherwise access a linked value.
    /// The plot writes its final limits and clears the cached pointers when it ends.
    ///
    /// ```compile_fail
    /// use dear_implot::{Axis, PlotSetup};
    /// fn link(plot_ui: &PlotSetup<'_>, min: &mut f64, max: &mut f64) {
    ///     plot_ui.setup_axis_links(Axis::Y1, Some(min), Some(max));
    /// }
    /// ```
    pub unsafe fn setup_axis_links(
        &self,
        axis: Axis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.plot.with_axes(&[axis.to_sys()], || unsafe {
            sys::ImPlot_SetupAxisLinks(axis.to_sys(), pmin, pmax)
        })
    }

    /// Link a raw axis to external min/max values (live binding).
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    /// The axis must be enabled. The storage and access requirements of [`Self::setup_axis_links`] apply.
    pub unsafe fn setup_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.plot.with_axes(&[axis], || unsafe {
            sys::ImPlot_SetupAxisLinks(axis, pmin, pmax)
        })
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

    /// Link an axis to external min/max values for the next plot.
    ///
    /// # Safety
    ///
    /// Primary axes consume these links at `BeginPlot`. Enable a secondary axis in the setup
    /// closure to apply its pending links.
    /// Keep values initialized, stable, distinct and otherwise inaccessible until the plot
    /// that consumes these settings has ended, the pending links are cleared, or the context
    /// is destroyed. A skipped call or a frame boundary is insufficient to establish this.
    ///
    /// ```compile_fail
    /// use dear_implot::{Axis, PlotUi};
    /// fn link(plot_ui: &PlotUi<'_>, min: &mut f64, max: &mut f64) {
    ///     plot_ui.set_next_axis_links(Axis::Y1, Some(min), Some(max));
    /// }
    /// ```
    pub unsafe fn set_next_axis_links(
        &self,
        axis: Axis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetNextAxisLinks(axis.to_sys(), pmin, pmax)
        })
    }

    /// Link a raw axis to external min/max values for the next plot.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value. Passing an out-of-range
    /// value lets ImPlot index internal next-plot arrays out of bounds.
    /// The storage and access requirements of [`Self::set_next_axis_links`] apply.
    pub unsafe fn set_next_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.with_bound_context(|| unsafe { sys::ImPlot_SetNextAxisLinks(axis, pmin, pmax) })
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
