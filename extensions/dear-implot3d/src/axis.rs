use crate::{
    Axis3D, Axis3DFlags, Plot3DCond, Plot3DUi, axis_tick_count_to_i32, compat_ffi,
    debug_before_plot, debug_before_setup, imvec2, len_i32, sys,
};

/// Axis helpers
impl<'ui> Plot3DUi<'ui> {
    pub fn setup_axes(
        &self,
        x_label: &str,
        y_label: &str,
        z_label: &str,
        x_flags: Axis3DFlags,
        y_flags: Axis3DFlags,
        z_flags: Axis3DFlags,
    ) {
        let _guard = self.bind();
        debug_before_setup();
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
    }

    pub fn setup_axis(&self, axis: Axis3D, label: &str, flags: Axis3DFlags) {
        let _guard = self.bind();
        debug_before_setup();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |ptr| unsafe {
            sys::ImPlot3D_SetupAxis(axis as i32, ptr, flags.bits() as i32)
        })
    }

    pub fn setup_axis_limits(&self, axis: Axis3D, min: f64, max: f64, cond: Plot3DCond) {
        let _guard = self.bind();
        debug_before_setup();
        unsafe { sys::ImPlot3D_SetupAxisLimits(axis as i32, min, max, cond as i32) }
    }

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
        let _guard = self.bind();
        debug_before_setup();
        unsafe {
            sys::ImPlot3D_SetupAxesLimits(x_min, x_max, y_min, y_max, z_min, z_max, cond as i32)
        }
    }

    pub fn setup_axis_limits_constraints(&self, axis: Axis3D, v_min: f64, v_max: f64) {
        let _guard = self.bind();
        debug_before_setup();
        unsafe { sys::ImPlot3D_SetupAxisLimitsConstraints(axis as i32, v_min, v_max) }
    }

    pub fn setup_axis_zoom_constraints(&self, axis: Axis3D, z_min: f64, z_max: f64) {
        let _guard = self.bind();
        debug_before_setup();
        unsafe { sys::ImPlot3D_SetupAxisZoomConstraints(axis as i32, z_min, z_max) }
    }

    /// Setup axis ticks using explicit positions and optional labels.
    ///
    /// If `labels` is provided, it must have the same length as `values`.
    pub fn setup_axis_ticks_values(
        &self,
        axis: Axis3D,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let _guard = self.bind();
        debug_before_setup();
        let Some(n_ticks) = len_i32(values.len()) else {
            return;
        };
        if let Some(lbls) = labels {
            if lbls.len() != values.len() {
                return;
            }
            let cleaned: Vec<&str> = lbls
                .iter()
                .map(|&s| if s.contains('\0') { "" } else { s })
                .collect();
            dear_imgui_rs::with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                sys::ImPlot3D_SetupAxisTicks_doublePtr(
                    axis as i32,
                    values.as_ptr(),
                    n_ticks,
                    ptrs.as_ptr(),
                    keep_default,
                )
            });
        } else {
            unsafe {
                sys::ImPlot3D_SetupAxisTicks_doublePtr(
                    axis as i32,
                    values.as_ptr(),
                    n_ticks,
                    std::ptr::null(),
                    keep_default,
                )
            };
        }
    }

    pub fn setup_axis_ticks_range(
        &self,
        axis: Axis3D,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let n_ticks_i32 = axis_tick_count_to_i32("Plot3DUi::setup_axis_ticks_range()", n_ticks);
        let _guard = self.bind();
        debug_before_setup();
        if let Some(lbls) = labels {
            if lbls.len() != n_ticks {
                return;
            }
            let cleaned: Vec<&str> = lbls
                .iter()
                .map(|&s| if s.contains('\0') { "" } else { s })
                .collect();
            dear_imgui_rs::with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                sys::ImPlot3D_SetupAxisTicks_double(
                    axis as i32,
                    v_min,
                    v_max,
                    n_ticks_i32,
                    ptrs.as_ptr(),
                    keep_default,
                )
            });
        } else {
            unsafe {
                sys::ImPlot3D_SetupAxisTicks_double(
                    axis as i32,
                    v_min,
                    v_max,
                    n_ticks_i32,
                    std::ptr::null(),
                    keep_default,
                )
            };
        }
    }

    pub fn setup_box_scale(&self, x: f32, y: f32, z: f32) {
        let _guard = self.bind();
        debug_before_setup();
        unsafe { sys::ImPlot3D_SetupBoxScale(x as f64, y as f64, z as f64) }
    }

    pub fn setup_box_rotation(
        &self,
        elevation: f32,
        azimuth: f32,
        animate: bool,
        cond: Plot3DCond,
    ) {
        let _guard = self.bind();
        debug_before_setup();
        unsafe {
            sys::ImPlot3D_SetupBoxRotation_double(
                elevation as f64,
                azimuth as f64,
                animate,
                cond as i32,
            )
        }
    }

    pub fn setup_box_initial_rotation(&self, elevation: f32, azimuth: f32) {
        let _guard = self.bind();
        debug_before_setup();
        unsafe { sys::ImPlot3D_SetupBoxInitialRotation_double(elevation as f64, azimuth as f64) }
    }

    pub fn plot_text(&self, text: &str, x: f32, y: f32, z: f32, angle: f32, pix_offset: [f32; 2]) {
        let _guard = self.bind();
        if text.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(text, |text_ptr| unsafe {
            debug_before_plot();
            sys::ImPlot3D_PlotText(
                text_ptr,
                x as f64,
                y as f64,
                z as f64,
                angle as f64,
                imvec2(pix_offset[0], pix_offset[1]),
            )
        })
    }

    pub fn plot_to_pixels(&self, point: [f32; 3]) -> [f32; 2] {
        let _guard = self.bind();
        unsafe {
            let out = compat_ffi::ImPlot3D_PlotToPixels_double(
                point[0] as f64,
                point[1] as f64,
                point[2] as f64,
            );
            [out.x, out.y]
        }
    }

    pub fn get_frame_pos(&self) -> [f32; 2] {
        let _guard = self.bind();
        unsafe {
            let out = compat_ffi::ImPlot3D_GetPlotRectPos();
            [out.x, out.y]
        }
    }

    pub fn get_frame_size(&self) -> [f32; 2] {
        let _guard = self.bind();
        unsafe {
            let out = compat_ffi::ImPlot3D_GetPlotRectSize();
            [out.x, out.y]
        }
    }
}
