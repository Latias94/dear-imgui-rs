use super::{Plot3D, Plot3DError, validate_lengths, validate_nonempty};
use crate::{Line3DFlags, Plot3DUi};

pub struct Line3D<'a> {
    pub label: &'a str,
    pub xs_f32: Option<&'a [f32]>,
    pub ys_f32: Option<&'a [f32]>,
    pub zs_f32: Option<&'a [f32]>,
    pub xs_f64: Option<&'a [f64]>,
    pub ys_f64: Option<&'a [f64]>,
    pub zs_f64: Option<&'a [f64]>,
    pub flags: Line3DFlags,
    pub offset: i32,
    pub stride: i32,
}

impl<'a> Line3D<'a> {
    pub fn f32(label: &'a str, xs: &'a [f32], ys: &'a [f32], zs: &'a [f32]) -> Self {
        Self {
            label,
            xs_f32: Some(xs),
            ys_f32: Some(ys),
            zs_f32: Some(zs),
            xs_f64: None,
            ys_f64: None,
            zs_f64: None,
            flags: Line3DFlags::NONE,
            offset: 0,
            stride: 0,
        }
    }
    pub fn f64(label: &'a str, xs: &'a [f64], ys: &'a [f64], zs: &'a [f64]) -> Self {
        Self {
            label,
            xs_f32: None,
            ys_f32: None,
            zs_f32: None,
            xs_f64: Some(xs),
            ys_f64: Some(ys),
            zs_f64: Some(zs),
            flags: Line3DFlags::NONE,
            offset: 0,
            stride: 0,
        }
    }
    pub fn flags(mut self, flags: Line3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn offset(mut self, o: i32) -> Self {
        self.offset = o;
        self
    }
    pub fn stride(mut self, s: i32) -> Self {
        self.stride = s;
        self
    }
}

impl<'a> Plot3D for Line3D<'a> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        if let (Some(x), Some(y), Some(z)) = (self.xs_f32, self.ys_f32, self.zs_f32) {
            validate_nonempty(x)?;
            validate_lengths(x, y, "x/y")?;
            validate_lengths(y, z, "y/z")?;
            ui.plot_line_f32_raw(self.label, x, y, z, self.flags, self.offset, self.stride);
            Ok(())
        } else if let (Some(x), Some(y), Some(z)) = (self.xs_f64, self.ys_f64, self.zs_f64) {
            validate_nonempty(x)?;
            validate_lengths(x, y, "x/y")?;
            validate_lengths(y, z, "y/z")?;
            ui.plot_line_f64_raw(self.label, x, y, z, self.flags, self.offset, self.stride);
            Ok(())
        } else {
            Err(Plot3DError::EmptyData)
        }
    }
}
