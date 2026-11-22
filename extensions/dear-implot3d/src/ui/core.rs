use crate::builder::Plot3DBuilder;
use crate::plots::primitive_vertex_count;
use crate::{
    Line3DFlags, Plot3DContext, Plot3DDataLayout, Plot3DFlags, Plot3DScope, Quad3DFlags,
    Scatter3DFlags, Triangle3DFlags, len_i32, sys,
};
use dear_imgui_rs::Ui;

/// Frame access to an ImPlot3D context. Begin a plot to configure axes and submit items.
///
/// ```compile_fail,E0599
/// use dear_implot3d::Plot3DUi;
/// fn query_outside_plot(ui: &Plot3DUi<'_>) { ui.get_frame_size(); }
/// ```
#[derive(Clone, Copy)]
pub struct Plot3DUi<'ui> {
    pub(crate) imgui: &'ui Ui,
    pub(crate) context: &'ui Plot3DContext,
}

impl<'ui> Plot3DUi<'ui> {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.imgui
            .with_bound_context(|| self.context.with_plot_context(f))
    }

    /// Configure a plot for the current window and table cell.
    pub fn begin_plot(&self, title: impl AsRef<str>) -> Plot3DBuilder<'ui> {
        Plot3DBuilder {
            ui: *self,
            title: title.as_ref().into(),
            size: [0.0; 2],
            flags: Plot3DFlags::empty(),
        }
    }
}

impl Plot3DScope<'_> {
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
    /// # fn draw(plot_ui: &Plot3DScope<'_>) {
    /// let xs = [0.0, 1.0, 2.0];
    /// let ys = [0.0, 1.0, 0.0];
    /// let zs = [0.0, 0.5, 1.0];
    /// plot_ui.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
    /// # }
    /// ```
    pub fn plot_line_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Line3DFlags,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = len_i32(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), Plot3DDataLayout::DEFAULT);
                if count == i32::MAX && (spec.Flags as u32 & Line3DFlags::LOOP.bits()) != 0 {
                    return;
                }
                sys::ImPlot3D_PlotLine_FloatPtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Line plot (f32) with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f32`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_line_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Line3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = layout.sample_count(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                if count == i32::MAX && (spec.Flags as u32 & Line3DFlags::LOOP.bits()) != 0 {
                    return;
                }
                sys::ImPlot3D_PlotLine_FloatPtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
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
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = len_i32(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), Plot3DDataLayout::DEFAULT);
                if count == i32::MAX && (spec.Flags as u32 & Line3DFlags::LOOP.bits()) != 0 {
                    return;
                }
                sys::ImPlot3D_PlotLine_doublePtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Line plot (f64) with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f64`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_line_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Line3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = layout.sample_count(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                if count == i32::MAX && (spec.Flags as u32 & Line3DFlags::LOOP.bits()) != 0 {
                    return;
                }
                sys::ImPlot3D_PlotLine_doublePtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
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
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = len_i32(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), Plot3DDataLayout::DEFAULT);
                sys::ImPlot3D_PlotScatter_FloatPtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Scatter plot (f32) with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f32`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_scatter_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Scatter3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = layout.sample_count(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                sys::ImPlot3D_PlotScatter_FloatPtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
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
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = len_i32(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), Plot3DDataLayout::DEFAULT);
                sys::ImPlot3D_PlotScatter_doublePtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Scatter plot (f64) with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f64`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_scatter_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Scatter3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Some(count) = layout.sample_count(xs.len()) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                sys::ImPlot3D_PlotScatter_doublePtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Convenience: plot triangles from separate X/Y/Z arrays (count must be multiple of 3)
    pub fn plot_triangles_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Triangle3DFlags,
    ) {
        // The default stride addresses one element of each slice per vertex.
        unsafe { self.plot_triangles_f32_raw(label, xs, ys, zs, flags, Plot3DDataLayout::DEFAULT) }
    }

    /// Plot f32 triangle coordinates with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f32`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_triangles_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Triangle3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Ok(count) = primitive_vertex_count(
                xs.len(),
                3,
                "triangles",
                layout,
                !flags.contains(Triangle3DFlags::NO_LINES),
            ) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                sys::ImPlot3D_PlotTriangle_FloatPtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Convenience: plot quads from separate X/Y/Z arrays (count must be multiple of 4)
    pub fn plot_quads_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Quad3DFlags,
    ) {
        // The default stride addresses one element of each slice per vertex.
        unsafe { self.plot_quads_f32_raw(label, xs, ys, zs, flags, Plot3DDataLayout::DEFAULT) }
    }

    /// Plot f32 quad coordinates with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f32`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_quads_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Quad3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Ok(count) = primitive_vertex_count(
                xs.len(),
                4,
                "quads",
                layout,
                !flags.contains(Quad3DFlags::NO_LINES),
            ) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                sys::ImPlot3D_PlotQuad_FloatPtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Convenience: plot triangles from separate X/Y/Z arrays (f64)
    pub fn plot_triangles_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Triangle3DFlags,
    ) {
        // The default stride addresses one element of each slice per vertex.
        unsafe { self.plot_triangles_f64_raw(label, xs, ys, zs, flags, Plot3DDataLayout::DEFAULT) }
    }

    /// Plot f64 triangle coordinates with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f64`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_triangles_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Triangle3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Ok(count) = primitive_vertex_count(
                xs.len(),
                3,
                "triangles",
                layout,
                !flags.contains(Triangle3DFlags::NO_LINES),
            ) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                sys::ImPlot3D_PlotTriangle_doublePtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    /// Convenience: plot quads from separate X/Y/Z arrays (f64)
    pub fn plot_quads_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Quad3DFlags,
    ) {
        // The default stride addresses one element of each slice per vertex.
        unsafe { self.plot_quads_f64_raw(label, xs, ys, zs, flags, Plot3DDataLayout::DEFAULT) }
    }

    /// Plot f64 quad coordinates with an explicit data layout.
    ///
    /// # Safety
    ///
    /// For every submitted index, `layout` must address an initialized, properly aligned `f64`
    /// within each coordinate allocation. All three allocations must remain alive for this call.
    pub unsafe fn plot_quads_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Quad3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let label = label.as_ref();
        self.with_bound_context(|| {
            if xs.len() != ys.len() || ys.len() != zs.len() {
                return;
            }
            let Ok(count) = primitive_vertex_count(
                xs.len(),
                4,
                "quads",
                layout,
                !flags.contains(Quad3DFlags::NO_LINES),
            ) else {
                return;
            };
            if label.contains('\0') {
                return;
            }
            dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
                let spec = self.next_item.spec(flags.bits(), layout);
                sys::ImPlot3D_PlotQuad_doublePtr(
                    label_ptr,
                    xs.as_ptr(),
                    ys.as_ptr(),
                    zs.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }
}
