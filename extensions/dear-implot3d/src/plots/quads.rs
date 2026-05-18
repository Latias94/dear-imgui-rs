use super::{Plot3D, Plot3DError, validate_lengths, validate_multiple, validate_nonempty};
use crate::{Plot3DDataLayout, Plot3DDataOffset, Plot3DDataStride, Plot3DUi, Quad3DFlags};

pub struct Quads3D<'a> {
    pub label: &'a str,
    pub xs_f32: Option<&'a [f32]>,
    pub ys_f32: Option<&'a [f32]>,
    pub zs_f32: Option<&'a [f32]>,
    pub xs_f64: Option<&'a [f64]>,
    pub ys_f64: Option<&'a [f64]>,
    pub zs_f64: Option<&'a [f64]>,
    pub flags: Quad3DFlags,
    pub layout: Plot3DDataLayout,
    pub points_f32: Option<&'a [[f32; 3]]>,
    pub points_f64: Option<&'a [[f64; 3]]>,
}

impl<'a> Quads3D<'a> {
    pub fn f32(label: &'a str, xs: &'a [f32], ys: &'a [f32], zs: &'a [f32]) -> Self {
        Self {
            label,
            xs_f32: Some(xs),
            ys_f32: Some(ys),
            zs_f32: Some(zs),
            xs_f64: None,
            ys_f64: None,
            zs_f64: None,
            flags: Quad3DFlags::NONE,
            layout: Plot3DDataLayout::DEFAULT,
            points_f32: None,
            points_f64: None,
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
            flags: Quad3DFlags::NONE,
            layout: Plot3DDataLayout::DEFAULT,
            points_f32: None,
            points_f64: None,
        }
    }
    pub fn points_f32(label: &'a str, pts: &'a [[f32; 3]]) -> Self {
        Self {
            label,
            xs_f32: None,
            ys_f32: None,
            zs_f32: None,
            xs_f64: None,
            ys_f64: None,
            zs_f64: None,
            flags: Quad3DFlags::NONE,
            layout: Plot3DDataLayout::DEFAULT,
            points_f32: Some(pts),
            points_f64: None,
        }
    }
    pub fn points_f64(label: &'a str, pts: &'a [[f64; 3]]) -> Self {
        Self {
            label,
            xs_f32: None,
            ys_f32: None,
            zs_f32: None,
            xs_f64: None,
            ys_f64: None,
            zs_f64: None,
            flags: Quad3DFlags::NONE,
            layout: Plot3DDataLayout::DEFAULT,
            points_f32: None,
            points_f64: Some(pts),
        }
    }
    pub fn flags(mut self, flags: Quad3DFlags) -> Self {
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

impl<'a> Plot3D for Quads3D<'a> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        if let Some(pts) = self.points_f32 {
            validate_nonempty(pts)?;
            validate_multiple(pts.len(), 4, "quads(points)")?;
            let mut xs = Vec::with_capacity(pts.len());
            let mut ys = Vec::with_capacity(pts.len());
            let mut zs = Vec::with_capacity(pts.len());
            for p in pts {
                xs.push(p[0]);
                ys.push(p[1]);
                zs.push(p[2]);
            }
            ui.plot_quads_f32(self.label, &xs, &ys, &zs, self.flags);
            return Ok(());
        }
        if let Some(pts) = self.points_f64 {
            validate_nonempty(pts)?;
            validate_multiple(pts.len(), 4, "quads(points)")?;
            let mut xs = Vec::with_capacity(pts.len());
            let mut ys = Vec::with_capacity(pts.len());
            let mut zs = Vec::with_capacity(pts.len());
            for p in pts {
                xs.push(p[0]);
                ys.push(p[1]);
                zs.push(p[2]);
            }
            ui.plot_quads_f64(self.label, &xs, &ys, &zs, self.flags);
            return Ok(());
        }
        if let (Some(x), Some(y), Some(z)) = (self.xs_f32, self.ys_f32, self.zs_f32) {
            validate_nonempty(x)?;
            validate_lengths(x, y, "x/y")?;
            validate_lengths(y, z, "y/z")?;
            validate_multiple(x.len(), 4, "quads")?;
            ui.plot_quads_f32_raw(self.label, x, y, z, self.flags, self.layout);
            return Ok(());
        }
        if let (Some(x), Some(y), Some(z)) = (self.xs_f64, self.ys_f64, self.zs_f64) {
            validate_nonempty(x)?;
            validate_lengths(x, y, "x/y")?;
            validate_lengths(y, z, "y/z")?;
            validate_multiple(x.len(), 4, "quads")?;
            ui.plot_quads_f64_raw(self.label, x, y, z, self.flags, self.layout);
            return Ok(());
        }
        Err(Plot3DError::EmptyData)
    }
}
