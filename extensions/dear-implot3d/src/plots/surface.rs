use super::{Plot3D, Plot3DError};
use crate::{Plot3DDataLayout, Plot3DDataOffset, Plot3DDataStride, Plot3DUi, Surface3DFlags};

pub struct Surface3D<'a> {
    pub label: &'a str,
    pub xs: &'a [f32],
    pub ys: &'a [f32],
    pub zs: &'a [f32],
    pub scale_min: f64,
    pub scale_max: f64,
    pub flags: Surface3DFlags,
    pub layout: Plot3DDataLayout,
}

impl<'a> Surface3D<'a> {
    pub fn new(label: &'a str, xs: &'a [f32], ys: &'a [f32], zs: &'a [f32]) -> Self {
        Self {
            label,
            xs,
            ys,
            zs,
            scale_min: f64::NAN,
            scale_max: f64::NAN,
            flags: Surface3DFlags::NONE,
            layout: Plot3DDataLayout::DEFAULT,
        }
    }
    pub fn scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }
    pub fn flags(mut self, flags: Surface3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn data_layout(mut self, layout: Plot3DDataLayout) -> Self {
        self.layout = layout;
        self
    }
    pub fn offset(mut self, offset: Plot3DDataOffset) -> Self {
        self.layout = self.layout.with_offset(offset);
        self
    }
    pub fn stride(mut self, stride: Plot3DDataStride) -> Self {
        self.layout = self.layout.with_stride(stride);
        self
    }
}

impl<'a> Plot3D for Surface3D<'a> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        let x_count = self.xs.len();
        let y_count = self.ys.len();
        let expected = x_count
            .checked_mul(y_count)
            .ok_or(Plot3DError::GridSizeMismatch {
                x_count,
                y_count,
                z_len: self.zs.len(),
            })?;
        if self.zs.len() != expected {
            return Err(Plot3DError::GridSizeMismatch {
                x_count,
                y_count,
                z_len: self.zs.len(),
            });
        }
        ui.surface_f32_raw(
            self.label,
            self.xs,
            self.ys,
            self.zs,
            self.scale_min,
            self.scale_max,
            self.flags,
            self.layout,
        );
        Ok(())
    }
}
