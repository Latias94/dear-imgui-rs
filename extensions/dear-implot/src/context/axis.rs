use super::ui::PlotUi;
use super::validation::{
    assert_axis_constraint_range, assert_axis_limit_range, assert_axis_zoom_range,
    assert_finite_f64_slice, axis_tick_count_to_i32,
};
use crate::{Axis, AxisFlags, AxisFormat, PlotCond, XAxis, YAxis, sys};
use dear_imgui_rs::{with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_two};
use std::os::raw::c_char;

impl<'ui> PlotUi<'ui> {
    /// Setup a specific X axis
    pub fn setup_x_axis(&self, axis: XAxis, label: Option<&str>, flags: AxisFlags) {
        self.with_bound_context(|| {
            let label = label.filter(|s| !s.contains('\0'));
            match label {
                Some(label) => with_scratch_txt(label, |ptr| unsafe {
                    sys::ImPlot_SetupAxis(
                        axis as sys::ImAxis,
                        ptr,
                        flags.bits() as sys::ImPlotAxisFlags,
                    )
                }),
                None => unsafe {
                    sys::ImPlot_SetupAxis(
                        axis as sys::ImAxis,
                        std::ptr::null(),
                        flags.bits() as sys::ImPlotAxisFlags,
                    )
                },
            }
        })
    }

    /// Setup a specific Y axis
    pub fn setup_y_axis(&self, axis: YAxis, label: Option<&str>, flags: AxisFlags) {
        self.with_bound_context(|| {
            let label = label.filter(|s| !s.contains('\0'));
            match label {
                Some(label) => with_scratch_txt(label, |ptr| unsafe {
                    sys::ImPlot_SetupAxis(
                        axis as sys::ImAxis,
                        ptr,
                        flags.bits() as sys::ImPlotAxisFlags,
                    )
                }),
                None => unsafe {
                    sys::ImPlot_SetupAxis(
                        axis as sys::ImAxis,
                        std::ptr::null(),
                        flags.bits() as sys::ImPlotAxisFlags,
                    )
                },
            }
        })
    }

    /// Setup axis limits for a specific X axis
    pub fn setup_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::setup_x_axis_limits()", min, max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        })
    }

    /// Setup axis limits for a specific Y axis
    pub fn setup_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::setup_y_axis_limits()", min, max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        })
    }

    /// Link an enabled axis to external min/max values for the active plot.
    ///
    /// # Safety
    ///
    /// Call after `begin_plot` succeeds, before plotting or any other setup-locking call.
    /// The axis must be enabled. Every supplied value must stay initialized at a stable
    /// address until this `PlotContext` is destroyed, unless its link is explicitly cleared.
    /// To clear it, begin the same plot ID, enable this axis, call this method with `None`
    /// for the corresponding value, and end that plot. Also clear any pending next-plot
    /// link for that value. `EndPlot` alone does not unlink every axis: later uses of the
    /// same plot can still write through retained pointers, even when an axis is disabled.
    /// Do not move, drop, or otherwise access values while linked, including during
    /// unwinding. Separate linked values must not alias.
    ///
    /// ```compile_fail
    /// use dear_implot::{Axis, PlotUi};
    /// fn link(plot_ui: &PlotUi<'_>, min: &mut f64, max: &mut f64) {
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
        self.with_bound_context(|| unsafe { sys::ImPlot_SetupAxisLinks(axis.to_sys(), pmin, pmax) })
    }

    /// Link a raw axis to external min/max values (live binding).
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    /// Call after `begin_plot` succeeds, before any setup-locking call, with the axis enabled.
    /// Every supplied value must stay initialized at a stable address until the owning
    /// `PlotContext` is destroyed or the corresponding stored and pending links are
    /// explicitly cleared. Clear the stored link on the same plot ID and enabled axis
    /// with `None`, then end that plot; clear pending next-plot links separately.
    /// `EndPlot` alone does not unlink every axis, and later plots may still write to it.
    /// Do not move, drop, or otherwise access values while linked, including during
    /// unwinding. Separate linked values must not alias.
    pub unsafe fn setup_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.with_bound_context(|| unsafe { sys::ImPlot_SetupAxisLinks(axis, pmin, pmax) })
    }

    /// Setup both axes labels/flags at once
    pub fn setup_axes(
        &self,
        x_label: Option<&str>,
        y_label: Option<&str>,
        x_flags: AxisFlags,
        y_flags: AxisFlags,
    ) {
        self.with_bound_context(|| {
            let x_label = x_label.filter(|s| !s.contains('\0'));
            let y_label = y_label.filter(|s| !s.contains('\0'));

            match (x_label, y_label) {
                (Some(x_label), Some(y_label)) => {
                    with_scratch_txt_two(x_label, y_label, |xp, yp| unsafe {
                        sys::ImPlot_SetupAxes(
                            xp,
                            yp,
                            x_flags.bits() as sys::ImPlotAxisFlags,
                            y_flags.bits() as sys::ImPlotAxisFlags,
                        )
                    })
                }
                (Some(x_label), None) => with_scratch_txt(x_label, |xp| unsafe {
                    sys::ImPlot_SetupAxes(
                        xp,
                        std::ptr::null(),
                        x_flags.bits() as sys::ImPlotAxisFlags,
                        y_flags.bits() as sys::ImPlotAxisFlags,
                    )
                }),
                (None, Some(y_label)) => with_scratch_txt(y_label, |yp| unsafe {
                    sys::ImPlot_SetupAxes(
                        std::ptr::null(),
                        yp,
                        x_flags.bits() as sys::ImPlotAxisFlags,
                        y_flags.bits() as sys::ImPlotAxisFlags,
                    )
                }),
                (None, None) => unsafe {
                    sys::ImPlot_SetupAxes(
                        std::ptr::null(),
                        std::ptr::null(),
                        x_flags.bits() as sys::ImPlotAxisFlags,
                        y_flags.bits() as sys::ImPlotAxisFlags,
                    )
                },
            }
        })
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
        assert_axis_limit_range("PlotUi::setup_axes_limits() x axis", x_min, x_max);
        assert_axis_limit_range("PlotUi::setup_axes_limits() y axis", y_min, y_max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond)
        })
    }

    /// Call after axis setup to finalize configuration
    pub fn setup_finish(&self) {
        self.with_bound_context(|| unsafe { sys::ImPlot_SetupFinish() })
    }

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
    /// Call before `begin_plot`, with no plot active. Enable the axis in the next plot.
    /// Every supplied value must stay initialized at a stable address until the owning
    /// `PlotContext` is destroyed or all corresponding pending and stored links are cleared.
    /// Clear a pending link with this method and `None`; clear an applied link with
    /// `setup_axis_links` and `None` on the same plot ID and enabled axis, then end that plot.
    /// `EndPlot`, a rejected `begin_plot`, a frame change, or disabling an axis does not
    /// generally unlink it. Do not move, drop, or otherwise access values while linked,
    /// including during unwinding. Separate linked values must not alias.
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
    /// Call before `begin_plot`, with no plot active, and enable the axis in the next plot.
    /// Every supplied value must stay initialized at a stable address until the owning
    /// `PlotContext` is destroyed or all corresponding pending and stored links are cleared.
    /// Clear pending links with `None` here; clear applied links with `None` on the same
    /// plot ID and enabled axis using `setup_axis_links_unchecked`, then end that plot.
    /// `EndPlot`, rejected `begin_plot` calls, frame changes, and disabled axes do not
    /// generally unlink it. Do not move, drop, or otherwise access values while linked,
    /// including during unwinding. Separate linked values must not alias.
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

    /// Setup ticks with explicit positions and optional labels for an X axis.
    ///
    /// If `labels` is provided, it must have the same length as `values`.
    pub fn setup_x_axis_ticks_positions(
        &self,
        axis: XAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_finite_f64_slice("PlotUi::setup_x_axis_ticks_positions()", "values", values);
        self.with_bound_context(|| {
            let count = match i32::try_from(values.len()) {
                Ok(v) => v,
                Err(_) => return,
            };
            if let Some(labels) = labels {
                if labels.len() != values.len() {
                    return;
                }
                let cleaned: Vec<&str> = labels
                    .iter()
                    .map(|&s| if s.contains('\0') { "" } else { s })
                    .collect();
                with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                    sys::ImPlot_SetupAxisTicks_doublePtr(
                        axis as sys::ImAxis,
                        values.as_ptr(),
                        count,
                        ptrs.as_ptr() as *const *const c_char,
                        keep_default,
                    )
                })
            } else {
                unsafe {
                    sys::ImPlot_SetupAxisTicks_doublePtr(
                        axis as sys::ImAxis,
                        values.as_ptr(),
                        count,
                        std::ptr::null(),
                        keep_default,
                    )
                }
            }
        })
    }

    /// Setup ticks with explicit positions and optional labels for a Y axis.
    ///
    /// If `labels` is provided, it must have the same length as `values`.
    pub fn setup_y_axis_ticks_positions(
        &self,
        axis: YAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_finite_f64_slice("PlotUi::setup_y_axis_ticks_positions()", "values", values);
        self.with_bound_context(|| {
            let count = match i32::try_from(values.len()) {
                Ok(v) => v,
                Err(_) => return,
            };
            if let Some(labels) = labels {
                if labels.len() != values.len() {
                    return;
                }
                let cleaned: Vec<&str> = labels
                    .iter()
                    .map(|&s| if s.contains('\0') { "" } else { s })
                    .collect();
                with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                    sys::ImPlot_SetupAxisTicks_doublePtr(
                        axis as sys::ImAxis,
                        values.as_ptr(),
                        count,
                        ptrs.as_ptr() as *const *const c_char,
                        keep_default,
                    )
                })
            } else {
                unsafe {
                    sys::ImPlot_SetupAxisTicks_doublePtr(
                        axis as sys::ImAxis,
                        values.as_ptr(),
                        count,
                        std::ptr::null(),
                        keep_default,
                    )
                }
            }
        })
    }

    /// Setup ticks on a range with tick count and optional labels for an X axis.
    ///
    /// If `labels` is provided, it must have length `n_ticks`.
    pub fn setup_x_axis_ticks_range(
        &self,
        axis: XAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_axis_limit_range("PlotUi::setup_x_axis_ticks_range()", v_min, v_max);
        let n_ticks_i32 = axis_tick_count_to_i32("PlotUi::setup_x_axis_ticks_range()", n_ticks);
        self.with_bound_context(|| {
            if let Some(labels) = labels {
                if labels.len() != n_ticks {
                    return;
                }
                let cleaned: Vec<&str> = labels
                    .iter()
                    .map(|&s| if s.contains('\0') { "" } else { s })
                    .collect();
                with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                    sys::ImPlot_SetupAxisTicks_double(
                        axis as sys::ImAxis,
                        v_min,
                        v_max,
                        n_ticks_i32,
                        ptrs.as_ptr() as *const *const c_char,
                        keep_default,
                    )
                })
            } else {
                unsafe {
                    sys::ImPlot_SetupAxisTicks_double(
                        axis as sys::ImAxis,
                        v_min,
                        v_max,
                        n_ticks_i32,
                        std::ptr::null(),
                        keep_default,
                    )
                }
            }
        })
    }

    /// Setup ticks on a range with tick count and optional labels for a Y axis.
    ///
    /// If `labels` is provided, it must have length `n_ticks`.
    pub fn setup_y_axis_ticks_range(
        &self,
        axis: YAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_axis_limit_range("PlotUi::setup_y_axis_ticks_range()", v_min, v_max);
        let n_ticks_i32 = axis_tick_count_to_i32("PlotUi::setup_y_axis_ticks_range()", n_ticks);
        self.with_bound_context(|| {
            if let Some(labels) = labels {
                if labels.len() != n_ticks {
                    return;
                }
                let cleaned: Vec<&str> = labels
                    .iter()
                    .map(|&s| if s.contains('\0') { "" } else { s })
                    .collect();
                with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                    sys::ImPlot_SetupAxisTicks_double(
                        axis as sys::ImAxis,
                        v_min,
                        v_max,
                        n_ticks_i32,
                        ptrs.as_ptr() as *const *const c_char,
                        keep_default,
                    )
                })
            } else {
                unsafe {
                    sys::ImPlot_SetupAxisTicks_double(
                        axis as sys::ImAxis,
                        v_min,
                        v_max,
                        n_ticks_i32,
                        std::ptr::null(),
                        keep_default,
                    )
                }
            }
        })
    }

    /// Setup a validated tick label format for a specific X axis.
    pub fn setup_x_axis_format(&self, axis: XAxis, format: &AxisFormat<'_>) {
        self.with_bound_context(|| {
            with_scratch_txt(format.as_str(), |ptr| unsafe {
                sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, ptr)
            })
        })
    }

    /// Setup a validated tick label format for a specific Y axis.
    pub fn setup_y_axis_format(&self, axis: YAxis, format: &AxisFormat<'_>) {
        self.with_bound_context(|| {
            with_scratch_txt(format.as_str(), |ptr| unsafe {
                sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, ptr)
            })
        })
    }

    /// Setup scale for a specific X axis (pass sys::ImPlotScale variant)
    pub fn setup_x_axis_scale(&self, axis: XAxis, scale: sys::ImPlotScale) {
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale)
        })
    }

    /// Setup scale for a specific Y axis (pass sys::ImPlotScale variant)
    pub fn setup_y_axis_scale(&self, axis: YAxis, scale: sys::ImPlotScale) {
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale)
        })
    }

    /// Setup axis limits constraints
    pub fn setup_axis_limits_constraints(&self, axis: Axis, v_min: f64, v_max: f64) {
        assert_axis_constraint_range("PlotUi::setup_axis_limits_constraints()", v_min, v_max);
        self.with_bound_context(|| unsafe {
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
            "PlotUi::setup_axis_limits_constraints_unchecked()",
            v_min,
            v_max,
        );
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxisLimitsConstraints(axis, v_min, v_max)
        })
    }

    /// Setup axis zoom constraints
    pub fn setup_axis_zoom_constraints(&self, axis: Axis, z_min: f64, z_max: f64) {
        assert_axis_zoom_range("PlotUi::setup_axis_zoom_constraints()", z_min, z_max);
        self.with_bound_context(|| unsafe {
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
            "PlotUi::setup_axis_zoom_constraints_unchecked()",
            z_min,
            z_max,
        );
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupAxisZoomConstraints(axis, z_min, z_max)
        })
    }
}
