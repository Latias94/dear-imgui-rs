use std::marker::PhantomData;

use crate::builder::Plot3DBuilder;
use crate::{
    Line3DFlags, Plot3DDataLayout, Plot3DFlags, Quad3DFlags, Scatter3DFlags, Triangle3DFlags,
    imgui_sys, len_i32, plot3d_spec_from, sys,
};
use dear_imgui_rs::Ui;

use super::binding::Plot3DContextBinding;

/// Per-frame access helper mirroring `dear-implot`
///
/// This provides access to all 3D plotting functions. It is tied to the lifetime
/// of the current ImGui frame and should be obtained via `Plot3DContext::get_plot_ui()`.
///
/// # Example
///
/// ```no_run
/// use dear_implot3d::*;
///
/// # let plot_ui: Plot3DUi = todo!();
/// if let Some(_token) = plot_ui.begin_plot("My 3D Plot").build() {
///     plot_ui.setup_axes("X", "Y", "Z", Axis3DFlags::NONE, Axis3DFlags::NONE, Axis3DFlags::NONE);
///
///     let xs = [0.0, 1.0, 2.0];
///     let ys = [0.0, 1.0, 0.0];
///     let zs = [0.0, 0.5, 1.0];
///     plot_ui.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
/// }
/// ```
pub struct Plot3DUi<'ui> {
    pub(crate) _ui: &'ui Ui,
    pub(crate) binding: Plot3DContextBinding,
    pub(crate) imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
}

impl<'ui> Plot3DUi<'ui> {
    pub(crate) fn from_current(ui: &'ui Ui) -> Self {
        let imgui_ctx_raw = unsafe { imgui_sys::igGetCurrentContext() };
        assert!(
            !imgui_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImGui context"
        );
        let plot_ctx_raw = unsafe { sys::ImPlot3D_GetCurrentContext() };
        assert!(
            !plot_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImPlot3D context"
        );
        Self {
            _ui: ui,
            binding: Plot3DContextBinding {
                plot_ctx_raw,
                imgui_ctx_raw,
            },
            imgui_alive: None,
        }
    }

    pub(crate) fn bind(&self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
        self.binding.bind();
    }

    /// Builder to configure and begin a 3D plot
    ///
    /// Returns a `Plot3DBuilder` that allows you to configure the plot before calling `.build()`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dear_implot3d::*;
    ///
    /// # let plot_ui: Plot3DUi = todo!();
    /// if let Some(_token) = plot_ui
    ///     .begin_plot("My Plot")
    ///     .size([600.0, 400.0])
    ///     .flags(Plot3DFlags::NO_LEGEND)
    ///     .build()
    /// {
    ///     // Plot content here
    /// }
    /// ```
    pub fn begin_plot<S: AsRef<str>>(&self, title: S) -> Plot3DBuilder<'ui> {
        self.bind();
        Plot3DBuilder {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            title: title.as_ref().into(),
            size: None,
            flags: Plot3DFlags::empty(),
            _lifetime: PhantomData,
        }
    }

    /// Convenience: plot a simple 3D line (f32)
    ///
    /// This is a quick way to plot a line without using the builder pattern.
    /// For more control, use the `plots::Line3D` builder.
    ///
    /// # Arguments
    ///
    /// * `label` - Label for the legend
    /// * `xs` - X coordinates
    /// * `ys` - Y coordinates
    /// * `zs` - Z coordinates
    /// * `flags` - Line flags (e.g., `Line3DFlags::SEGMENTS`, `Line3DFlags::LOOP`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dear_implot3d::*;
    ///
    /// # let plot_ui: Plot3DUi = todo!();
    /// let xs = [0.0, 1.0, 2.0];
    /// let ys = [0.0, 1.0, 0.0];
    /// let zs = [0.0, 0.5, 1.0];
    /// plot_ui.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
    /// ```
    pub fn plot_line_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Line3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotLine_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Line plot (f32) with an explicit data layout.
    pub fn plot_line_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Line3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotLine_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot a simple 3D line (f64)
    pub fn plot_line_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Line3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotLine_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Line plot (f64) with an explicit data layout.
    pub fn plot_line_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Line3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotLine_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot a 3D scatter (f32)
    pub fn plot_scatter_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Scatter3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotScatter_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Scatter plot (f32) with an explicit data layout.
    pub fn plot_scatter_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Scatter3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotScatter_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot a 3D scatter (f64)
    pub fn plot_scatter_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Scatter3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotScatter_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Scatter plot (f64) with an explicit data layout.
    pub fn plot_scatter_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Scatter3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotScatter_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot triangles from interleaved xyz arrays (count must be multiple of 3)
    pub fn plot_triangles_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Triangle3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotTriangle_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    pub fn plot_triangles_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Triangle3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotTriangle_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot quads from interleaved xyz arrays (count must be multiple of 4)
    pub fn plot_quads_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Quad3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotQuad_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    pub fn plot_quads_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Quad3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotQuad_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot triangles from interleaved xyz arrays (f64)
    pub fn plot_triangles_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Triangle3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotTriangle_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    pub fn plot_triangles_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Triangle3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotTriangle_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Convenience: plot quads from interleaved xyz arrays (f64)
    pub fn plot_quads_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Quad3DFlags,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT);
            sys::ImPlot3D_PlotQuad_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }

    pub fn plot_quads_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Quad3DFlags,
        layout: Plot3DDataLayout,
    ) {
        self.bind();
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let Some(count) = len_i32(xs.len()) else {
            return;
        };
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotQuad_doublePtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                count,
                spec,
            );
        })
    }
}
