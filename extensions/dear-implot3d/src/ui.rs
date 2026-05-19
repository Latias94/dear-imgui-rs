use std::marker::PhantomData;

use crate::builder::Plot3DBuilder;
use crate::{
    Line3DFlags, Plot3DDataLayout, Plot3DFlags, Quad3DFlags, Scatter3DFlags, Triangle3DFlags,
    debug_end_plot, imgui_sys, len_i32, plot3d_spec_from, sys,
};
use dear_imgui_rs::Ui;

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

#[derive(Clone, Copy)]
pub(crate) struct Plot3DContextBinding {
    pub(crate) plot_ctx_raw: *mut sys::ImPlot3DContext,
    pub(crate) imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
}

impl Plot3DContextBinding {
    pub(crate) fn bind(self) {
        assert!(
            !self.imgui_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImGui context"
        );
        assert!(
            !self.plot_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImPlot3D context"
        );
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            self.imgui_ctx_raw,
            "dear-implot3d: Plot3DUi must be used with the currently-active ImGui context"
        );
        unsafe { sys::ImPlot3D_SetCurrentContext(self.plot_ctx_raw) };
    }
}

/// RAII token that ends the plot on drop
///
/// This token is returned by `Plot3DBuilder::build()` and automatically calls
/// `ImPlot3D_EndPlot()` when it goes out of scope, ensuring proper cleanup.
pub struct Plot3DToken<'ui> {
    pub(crate) binding: Plot3DContextBinding,
    pub(crate) imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    pub(crate) _lifetime: PhantomData<&'ui Ui>,
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

impl Drop for Plot3DToken<'_> {
    fn drop(&mut self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
        self.binding.bind();
        unsafe {
            debug_end_plot();
            sys::ImPlot3D_EndPlot();
        }
    }
}

/// Optional mint support for inputs
///
/// When the `mint` feature is enabled, you can use `mint::Point3<f32>` and `mint::Vector3<f32>`
/// types directly with plotting functions. This provides interoperability with popular math
/// libraries like `glam`, `nalgebra`, `cgmath`, etc.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "mint")]
/// # {
/// use dear_implot3d::*;
/// use mint::Point3;
///
/// # let plot_ui: Plot3DUi = todo!();
/// let points = vec![
///     Point3 { x: 0.0, y: 0.0, z: 0.0 },
///     Point3 { x: 1.0, y: 1.0, z: 1.0 },
///     Point3 { x: 2.0, y: 0.0, z: 2.0 },
/// ];
///
/// if let Some(_token) = plot_ui.begin_plot("Mint Example").build() {
///     plot_ui.plot_line_mint("Line", &points, Line3DFlags::NONE);
/// }
/// # }
/// ```
#[cfg(feature = "mint")]
impl<'ui> Plot3DUi<'ui> {
    /// Plot a 3D line using `mint::Point3<f32>` points
    ///
    /// This is a convenience function that converts mint points to separate x, y, z arrays.
    pub fn plot_line_mint<S: AsRef<str>>(
        &self,
        label: S,
        pts: &[mint::Point3<f32>],
        flags: Line3DFlags,
    ) {
        let mut xs = Vec::with_capacity(pts.len());
        let mut ys = Vec::with_capacity(pts.len());
        let mut zs = Vec::with_capacity(pts.len());
        for p in pts {
            xs.push(p.x);
            ys.push(p.y);
            zs.push(p.z);
        }
        self.plot_line_f32(label, &xs, &ys, &zs, flags);
    }

    /// Plot a 3D scatter using `mint::Point3<f32>` points
    pub fn plot_scatter_mint<S: AsRef<str>>(
        &self,
        label: S,
        pts: &[mint::Point3<f32>],
        flags: Scatter3DFlags,
    ) {
        let mut xs = Vec::with_capacity(pts.len());
        let mut ys = Vec::with_capacity(pts.len());
        let mut zs = Vec::with_capacity(pts.len());
        for p in pts {
            xs.push(p.x);
            ys.push(p.y);
            zs.push(p.z);
        }
        self.plot_scatter_f32(label, &xs, &ys, &zs, flags);
    }

    /// Plot 3D text at a `mint::Point3<f32>` position
    pub fn plot_text_mint(
        &self,
        text: &str,
        pos: mint::Point3<f32>,
        angle: f32,
        pix_offset: [f32; 2],
    ) {
        self.plot_text(text, pos.x, pos.y, pos.z, angle, pix_offset);
    }

    /// Convert a `mint::Point3<f32>` to pixel coordinates
    pub fn plot_to_pixels_mint(&self, point: mint::Point3<f32>) -> [f32; 2] {
        self.plot_to_pixels([point.x, point.y, point.z])
    }
}
