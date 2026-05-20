use super::ui::PlotUi;
use super::validation::{
    assert_axis_constraint_range, assert_axis_limit_range, assert_axis_zoom_range,
    assert_finite_f64_slice, axis_tick_count_to_i32,
};
use crate::{Axis, AxisFlags, PlotCond, XAxis, YAxis, sys};
use dear_imgui_rs::{with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_two};
use std::os::raw::c_char;

impl<'ui> PlotUi<'ui> {
    /// Setup a specific X axis
    pub fn setup_x_axis(&self, axis: XAxis, label: Option<&str>, flags: AxisFlags) {
        self.bind();
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
    }

    /// Setup a specific Y axis
    pub fn setup_y_axis(&self, axis: YAxis, label: Option<&str>, flags: AxisFlags) {
        self.bind();
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
    }

    /// Setup axis limits for a specific X axis
    pub fn setup_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::setup_x_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Setup axis limits for a specific Y axis
    pub fn setup_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::setup_y_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Link an axis to external min/max values (live binding)
    pub fn setup_axis_links(
        &self,
        axis: Axis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLinks(axis.to_sys(), pmin, pmax) }
    }

    /// Link a raw axis to external min/max values (live binding).
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn setup_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLinks(axis, pmin, pmax) }
    }

    /// Setup both axes labels/flags at once
    pub fn setup_axes(
        &self,
        x_label: Option<&str>,
        y_label: Option<&str>,
        x_flags: AxisFlags,
        y_flags: AxisFlags,
    ) {
        self.bind();
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
        self.bind();
        unsafe { sys::ImPlot_SetupAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond) }
    }

    /// Call after axis setup to finalize configuration
    pub fn setup_finish(&self) {
        self.bind();
        unsafe { sys::ImPlot_SetupFinish() }
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::set_next_x_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::set_next_y_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Link an axis to external min/max for next frame
    pub fn set_next_axis_links(
        &self,
        axis: Axis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisLinks(axis.to_sys(), pmin, pmax) }
    }

    /// Link a raw axis to external min/max for the next frame.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value. Passing an out-of-range
    /// value lets ImPlot index internal next-plot arrays out of bounds.
    pub unsafe fn set_next_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisLinks(axis, pmin, pmax) }
    }

    /// Set next frame limits for both axes
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
        self.bind();
        unsafe {
            sys::ImPlot_SetNextAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond)
        }
    }

    /// Fit next frame both axes
    pub fn set_next_axes_to_fit(&self) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxesToFit() }
    }

    /// Fit next frame a specific axis
    pub fn set_next_axis_to_fit(&self, axis: Axis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis.to_sys()) }
    }

    /// Fit next frame a raw axis.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value. Passing an out-of-range
    /// value lets ImPlot index internal next-plot arrays out of bounds.
    pub unsafe fn set_next_axis_to_fit_unchecked(&self, axis: sys::ImAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis) }
    }

    /// Fit next frame a specific X axis
    pub fn set_next_x_axis_to_fit(&self, axis: XAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
    }

    /// Fit next frame a specific Y axis
    pub fn set_next_y_axis_to_fit(&self, axis: YAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
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
        self.bind();
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
        self.bind();
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
        self.bind();
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
        self.bind();
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
    }

    /// Setup tick label format string for a specific X axis
    pub fn setup_x_axis_format(&self, axis: XAxis, fmt: &str) {
        if fmt.contains('\0') {
            return;
        }
        self.bind();
        with_scratch_txt(fmt, |ptr| unsafe {
            sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, ptr)
        })
    }

    /// Setup tick label format string for a specific Y axis
    pub fn setup_y_axis_format(&self, axis: YAxis, fmt: &str) {
        if fmt.contains('\0') {
            return;
        }
        self.bind();
        with_scratch_txt(fmt, |ptr| unsafe {
            sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, ptr)
        })
    }

    /// Setup scale for a specific X axis (pass sys::ImPlotScale variant)
    pub fn setup_x_axis_scale(&self, axis: XAxis, scale: sys::ImPlotScale) {
        self.bind();
        unsafe { sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale) }
    }

    /// Setup scale for a specific Y axis (pass sys::ImPlotScale variant)
    pub fn setup_y_axis_scale(&self, axis: YAxis, scale: sys::ImPlotScale) {
        self.bind();
        unsafe { sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale) }
    }

    /// Setup axis limits constraints
    pub fn setup_axis_limits_constraints(&self, axis: Axis, v_min: f64, v_max: f64) {
        assert_axis_constraint_range("PlotUi::setup_axis_limits_constraints()", v_min, v_max);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLimitsConstraints(axis.to_sys(), v_min, v_max) }
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
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLimitsConstraints(axis, v_min, v_max) }
    }

    /// Setup axis zoom constraints
    pub fn setup_axis_zoom_constraints(&self, axis: Axis, z_min: f64, z_max: f64) {
        assert_axis_zoom_range("PlotUi::setup_axis_zoom_constraints()", z_min, z_max);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisZoomConstraints(axis.to_sys(), z_min, z_max) }
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
        self.bind();
        unsafe { sys::ImPlot_SetupAxisZoomConstraints(axis, z_min, z_max) }
    }
}
