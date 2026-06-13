use std::borrow::Cow;

use crate::item_style::{Plot3DItemStyle, plot3d_spec_with_style};
use crate::{
    Item3DFlags, Plot3DDataLayout, Plot3DUi, Surface3DFlags, debug_before_plot, plot3d_spec_from,
    surface_count_to_i32, sys,
};

/// Surface (grid) plot builder (f32 variant)
pub struct Surface3DBuilder<'ui> {
    pub(crate) _ui: &'ui Plot3DUi<'ui>,
    pub(crate) label: Cow<'ui, str>,
    pub(crate) xs: &'ui [f32],
    pub(crate) ys: &'ui [f32],
    pub(crate) zs: &'ui [f32],
    pub(crate) scale_min: f64,
    pub(crate) scale_max: f64,
    pub(crate) flags: Surface3DFlags,
    pub(crate) item_flags: Item3DFlags,
    pub(crate) style: Plot3DItemStyle,
}

impl<'ui> Surface3DBuilder<'ui> {
    pub fn scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }
    pub fn flags(mut self, flags: Surface3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn plot(self) {
        let _guard = self._ui.bind();
        let x_count = match i32::try_from(self.xs.len()) {
            Ok(v) => v,
            Err(_) => return,
        };
        let y_count = match i32::try_from(self.ys.len()) {
            Ok(v) => v,
            Err(_) => return,
        };
        let expected = match self.xs.len().checked_mul(self.ys.len()) {
            Some(v) => v,
            None => return,
        };
        if self.zs.len() != expected {
            return;
        }
        let label = self.label.as_ref();
        let label = if label.contains('\0') {
            "surface"
        } else {
            label
        };
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                Plot3DDataLayout::DEFAULT,
            );
            sys::ImPlot3D_PlotSurface_FloatPtr(
                label_ptr,
                self.xs.as_ptr(),
                self.ys.as_ptr(),
                self.zs.as_ptr(),
                x_count,
                y_count,
                self.scale_min,
                self.scale_max,
                spec,
            );
        })
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Start a surface plot (f32)
    pub fn surface_f32(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        xs: &'ui [f32],
        ys: &'ui [f32],
        zs: &'ui [f32],
    ) -> Surface3DBuilder<'ui> {
        let _guard = self.bind();
        Surface3DBuilder {
            _ui: self,
            label: label.into(),
            xs,
            ys,
            zs,
            scale_min: f64::NAN,
            scale_max: f64::NAN,
            flags: Surface3DFlags::NONE,
            item_flags: Item3DFlags::NONE,
            style: Plot3DItemStyle::default(),
        }
    }

    /// Raw surface plot (f32) with an explicit data layout.
    pub fn surface_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        scale_min: f64,
        scale_max: f64,
        flags: Surface3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let _guard = self.bind();
        debug_before_plot();
        let x_count = xs.len();
        let y_count = ys.len();
        let Some(x_count_i32) = surface_count_to_i32(x_count) else {
            return;
        };
        let Some(y_count_i32) = surface_count_to_i32(y_count) else {
            return;
        };
        let expected = match x_count.checked_mul(y_count) {
            Some(v) => v,
            None => return,
        };
        if zs.len() != expected {
            // Invalid grid: require zs to be x_count * y_count
            return;
        }

        // Flatten xs/ys to per-vertex arrays expected by the C++ API (length = x_count * y_count)
        let mut xs_flat = Vec::with_capacity(expected);
        let mut ys_flat = Vec::with_capacity(expected);
        for yi in 0..y_count {
            for xi in 0..x_count {
                xs_flat.push(xs[xi]);
                ys_flat.push(ys[yi]);
            }
        }

        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotSurface_FloatPtr(
                label_ptr,
                xs_flat.as_ptr(),
                ys_flat.as_ptr(),
                zs.as_ptr(),
                x_count_i32,
                y_count_i32,
                scale_min,
                scale_max,
                spec,
            );
        })
    }

    /// Plot a surface with already flattened per-vertex X/Y arrays (no internal allocation)
    ///
    /// Use this when you already have per-vertex `xs_flat` and `ys_flat` of length `x_count * y_count`,
    /// matching the layout of `zs`. This avoids per-frame allocations for large dynamic grids.
    pub fn surface_f32_flat<S: AsRef<str>>(
        &self,
        label: S,
        xs_flat: &[f32],
        ys_flat: &[f32],
        zs: &[f32],
        x_count: usize,
        y_count: usize,
        scale_min: f64,
        scale_max: f64,
        flags: Surface3DFlags,
        layout: Plot3DDataLayout,
    ) {
        let _guard = self.bind();
        debug_before_plot();
        let Some(x_count_i32) = surface_count_to_i32(x_count) else {
            return;
        };
        let Some(y_count_i32) = surface_count_to_i32(y_count) else {
            return;
        };
        let Some(expected) = x_count.checked_mul(y_count) else {
            return;
        };
        if xs_flat.len() != expected || ys_flat.len() != expected || zs.len() != expected {
            return;
        }
        let label = label.as_ref();
        if label.contains('\0') {
            return;
        }
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            let spec = plot3d_spec_from(flags.bits(), layout);
            sys::ImPlot3D_PlotSurface_FloatPtr(
                label_ptr,
                xs_flat.as_ptr(),
                ys_flat.as_ptr(),
                zs.as_ptr(),
                x_count_i32,
                y_count_i32,
                scale_min,
                scale_max,
                spec,
            );
        })
    }
}
