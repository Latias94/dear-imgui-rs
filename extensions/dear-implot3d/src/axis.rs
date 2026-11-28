use crate::{
    Axis3D, Axis3DFlags, Plot3DCond, Plot3DScope, Plot3DSetup, axis_tick_count_to_i32, imvec2,
    len_i32, sys,
};
use std::ffi::c_char;
use std::ptr;

#[derive(Clone, Copy, Default)]
pub(crate) struct AxisDomain {
    log_scale: bool,
    constraints: Option<[f64; 2]>,
    has_nonpositive_ticks: bool,
}

impl Plot3DSetup<'_> {
    /// Sets the axis labels and flags. Labels may contain literal percent signs.
    /// If any label contains NUL, all axes are left unchanged.
    pub fn setup_axes(
        &self,
        x_label: &str,
        y_label: &str,
        z_label: &str,
        x_flags: Axis3DFlags,
        y_flags: Axis3DFlags,
        z_flags: Axis3DFlags,
    ) {
        self.with_bound_context(|| {
            if x_label.contains('\0') || y_label.contains('\0') || z_label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt_three(
                x_label,
                y_label,
                z_label,
                |x_ptr, y_ptr, z_ptr| unsafe {
                    sys::ImPlot3D_SetupAxes(
                        x_ptr,
                        y_ptr,
                        z_ptr,
                        x_flags.bits() as i32,
                        y_flags.bits() as i32,
                        z_flags.bits() as i32,
                    )
                },
            )
        })
    }

    /// Sets one axis label and its flags. The axis menu displays the label verbatim.
    /// A label containing NUL leaves the axis unchanged.
    pub fn setup_axis(&self, axis: Axis3D, label: &str, flags: Axis3DFlags) {
        self.with_bound_context(|| {
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot3D_SetupAxis(axis as i32, ptr, flags.bits() as i32)
            })
        })
    }

    /// Sets the axis limits. Endpoints must be finite and strictly increasing with a
    /// finite span. Use [`Axis3DFlags::INVERT`] to reverse the axis.
    /// A logarithmic axis additionally requires strictly positive endpoints.
    pub fn setup_axis_limits(&self, axis: Axis3D, min: f64, max: f64, cond: Plot3DCond) {
        self.check_axis_limits(axis, min, max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupAxisLimits(axis as i32, min, max, cond as i32)
        })
    }

    /// Sets all three axis limits with the same bounds as [`Self::setup_axis_limits`].
    #[allow(
        clippy::too_many_arguments,
        reason = "Keeps the native SetupAxesLimits argument order for the existing public API"
    )]
    pub fn setup_axes_limits(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cond: Plot3DCond,
    ) {
        self.check_axis_limits(Axis3D::X, x_min, x_max);
        self.check_axis_limits(Axis3D::Y, y_min, y_max);
        self.check_axis_limits(Axis3D::Z, z_min, z_max);
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupAxesLimits(x_min, x_max, y_min, y_max, z_min, z_max, cond as i32)
        })
    }

    fn check_axis_limits(&self, axis: Axis3D, min: f64, max: f64) {
        assert_axis_span("axis limits", axis, min, max);
        if self.axes[axis as usize].get().log_scale {
            assert_log_span("axis limits", axis, min, max);
        }
    }

    /// Sets a base-10 logarithmic scale and fixed limits for this submission.
    /// Call this in each setup where the axis should be logarithmic.
    ///
    /// # Panics
    ///
    /// Panics if the endpoints are not finite, strictly positive and increasing, or
    /// if ticks or limit constraints already configured on this axis are outside
    /// the logarithmic domain.
    pub fn setup_axis_log_limits(&self, axis: Axis3D, min: f64, max: f64) {
        assert_log_span("axis limits", axis, min, max);
        let domain = &self.axes[axis as usize];
        let mut state = domain.get();
        assert!(
            !state.has_nonpositive_ticks,
            "ImPlot3D {axis:?} logarithmic axis ticks must be positive"
        );
        if let Some([lower, upper]) = state.constraints {
            assert_log_span("axis limit constraints", axis, lower, upper);
        }
        self.with_bound_context(|| unsafe {
            if !state.log_scale {
                sys::ImPlot3D_SetupAxisScale_Plot3DScale(
                    axis as i32,
                    sys::ImPlot3DScale_Log10 as _,
                );
            }
            sys::ImPlot3D_SetupAxisLimits(axis as i32, min, max, Plot3DCond::Always as i32);
        });
        state.log_scale = true;
        domain.set(state);
    }

    /// Constrains the axis endpoints. Bounds must be finite and ordered.
    /// A logarithmic axis requires strictly positive, distinct bounds.
    pub fn setup_axis_limits_constraints(&self, axis: Axis3D, v_min: f64, v_max: f64) {
        assert!(
            v_min.is_finite() && v_max.is_finite() && v_min <= v_max,
            "ImPlot3D {axis:?} axis limit constraints require finite, ordered endpoints"
        );
        let domain = &self.axes[axis as usize];
        let mut state = domain.get();
        if state.log_scale {
            assert_log_span("axis limit constraints", axis, v_min, v_max);
        }
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupAxisLimitsConstraints(axis as i32, v_min, v_max)
        });
        state.constraints = Some([v_min, v_max]);
        domain.set(state);
    }

    /// Constrains the visible span. Both bounds must be finite and positive, with
    /// the minimum no larger than the maximum.
    pub fn setup_axis_zoom_constraints(&self, axis: Axis3D, z_min: f64, z_max: f64) {
        assert!(
            z_min.is_finite() && z_max.is_finite() && z_min > 0.0 && z_min <= z_max,
            "ImPlot3D {axis:?} axis zoom constraints require positive, finite, ordered bounds"
        );
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupAxisZoomConstraints(axis as i32, z_min, z_max)
        })
    }

    /// Setup axis ticks using explicit positions and optional labels.
    ///
    /// If `labels` is provided, it must have the same length as `values` and contain no NUL
    /// bytes, otherwise the operation is ignored. Positions must be finite and, on a log axis,
    /// strictly positive.
    pub fn setup_axis_ticks_values(
        &self,
        axis: Axis3D,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        self.with_bound_context(|| {
            let Some(n_ticks) = len_i32(values.len()) else {
                return;
            };
            with_tick_labels(labels, values.len(), |labels| {
                assert!(
                    values.iter().all(|value| value.is_finite()),
                    "ImPlot3D {axis:?} axis ticks must be finite"
                );
                self.record_tick_domain(axis, values.iter().any(|&value| value <= 0.0));
                unsafe {
                    sys::ImPlot3D_SetupAxisTicks_doublePtr(
                        axis as i32,
                        values.as_ptr(),
                        n_ticks,
                        labels,
                        keep_default,
                    )
                };
            });
        })
    }

    /// Setup evenly spaced axis ticks over a range with optional labels.
    ///
    /// If `labels` is provided, `n_ticks` must be at least 2 and `labels` must
    /// have length `n_ticks`. A label length mismatch or NUL byte leaves the axis unchanged.
    /// Without labels, ImPlot3D clamps a count of 1 to 2 ticks.
    ///
    /// # Panics
    ///
    /// Panics for an invalid range, if `n_ticks` is zero, exceeds `i32::MAX`,
    /// or is less than 2 when `labels` is provided. On a log axis the endpoints
    /// must be strictly positive.
    pub fn setup_axis_ticks_range(
        &self,
        axis: Axis3D,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        if labels.is_some() {
            // ImPlot3D clamps the count to at least 2 before indexing labels.
            assert!(
                n_ticks >= 2,
                "Plot3DSetup::setup_axis_ticks_range() n_ticks must be at least 2 when labels are provided"
            );
        }
        let n_ticks_i32 = axis_tick_count_to_i32("Plot3DSetup::setup_axis_ticks_range()", n_ticks);
        self.with_bound_context(|| {
            with_tick_labels(labels, n_ticks, |labels| {
                assert_axis_span("axis tick range", axis, v_min, v_max);
                self.record_tick_domain(axis, v_min <= 0.0);
                unsafe {
                    sys::ImPlot3D_SetupAxisTicks_double(
                        axis as i32,
                        v_min,
                        v_max,
                        n_ticks_i32,
                        labels,
                        keep_default,
                    )
                };
            });
        })
    }

    fn record_tick_domain(&self, axis: Axis3D, has_nonpositive_ticks: bool) {
        if has_nonpositive_ticks {
            let domain = &self.axes[axis as usize];
            let mut state = domain.get();
            assert!(
                !state.log_scale,
                "ImPlot3D {axis:?} logarithmic axis ticks must be positive"
            );
            state.has_nonpositive_ticks = true;
            domain.set(state);
        }
    }

    pub fn setup_box_scale(&self, x: f32, y: f32, z: f32) {
        assert!(
            [x, y, z].into_iter().all(|v| v.is_finite() && v > 0.0),
            "ImPlot3D box scale must be finite and positive"
        );
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupBoxScale(x as f64, y as f64, z as f64)
        })
    }

    pub fn setup_box_rotation(
        &self,
        elevation: f32,
        azimuth: f32,
        animate: bool,
        cond: Plot3DCond,
    ) {
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupBoxRotation_double(
                elevation as f64,
                azimuth as f64,
                animate,
                cond as i32,
            )
        })
    }

    pub fn setup_box_initial_rotation(&self, elevation: f32, azimuth: f32) {
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_SetupBoxInitialRotation_double(elevation as f64, azimuth as f64)
        })
    }
}

impl Plot3DScope<'_> {
    /// Draws text at a plot position. Text containing NUL is not drawn.
    pub fn plot_text(&self, text: &str, x: f32, y: f32, z: f32, angle: f32, pix_offset: [f32; 2]) {
        self.with_bound_context(|| {
            if text.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(text, |text_ptr| unsafe {
                sys::ImPlot3D_PlotText(
                    text_ptr,
                    x as f64,
                    y as f64,
                    z as f64,
                    angle as f64,
                    imvec2(pix_offset[0], pix_offset[1]),
                )
            })
        })
    }

    pub fn plot_to_pixels(&self, point: [f32; 3]) -> [f32; 2] {
        self.with_bound_context(|| unsafe {
            let out = sys::ImPlot3D_PlotToPixels_double(
                point[0] as f64,
                point[1] as f64,
                point[2] as f64,
            );
            [out.x, out.y]
        })
    }

    pub fn get_frame_pos(&self) -> [f32; 2] {
        self.with_bound_context(|| unsafe {
            let out = sys::ImPlot3D_GetPlotRectPos();
            [out.x, out.y]
        })
    }

    pub fn get_frame_size(&self) -> [f32; 2] {
        self.with_bound_context(|| unsafe {
            let out = sys::ImPlot3D_GetPlotRectSize();
            [out.x, out.y]
        })
    }
}

fn assert_axis_span(what: &str, axis: Axis3D, min: f64, max: f64) {
    assert!(
        min < max && (max - min).is_finite(),
        "ImPlot3D {axis:?} {what} require increasing endpoints with a finite span"
    );
}

fn assert_log_span(what: &str, axis: Axis3D, min: f64, max: f64) {
    assert!(
        min > 0.0 && max.is_finite() && min.log10() < max.log10(),
        "ImPlot3D {axis:?} logarithmic {what} require positive, finite endpoints with a nonzero log span"
    );
}

fn with_tick_labels(labels: Option<&[&str]>, count: usize, f: impl FnOnce(*const *const c_char)) {
    match labels {
        None => f(ptr::null()),
        Some(labels)
            if labels.len() == count && !labels.iter().any(|label| label.contains('\0')) =>
        {
            dear_imgui_rs::with_scratch_txt_slice(labels, |pointers| f(pointers.as_ptr()));
        }
        Some(_) => {}
    }
}
