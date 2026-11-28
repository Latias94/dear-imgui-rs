use super::{Plot3D, Plot3DError, primitive_vertex_count, validate_item_label, validate_lengths};
use crate::{Plot3DDataLayout, Plot3DDataOffset, Plot3DScope, Quad3DFlags};

pub struct Quads3D<'a> {
    pub label: &'a str,
    pub xs_f32: Option<&'a [f32]>,
    pub ys_f32: Option<&'a [f32]>,
    pub zs_f32: Option<&'a [f32]>,
    pub xs_f64: Option<&'a [f64]>,
    pub ys_f64: Option<&'a [f64]>,
    pub zs_f64: Option<&'a [f64]>,
    pub flags: Quad3DFlags,
    layout: Plot3DDataLayout,
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
    pub fn offset(mut self, offset: Plot3DDataOffset) -> Self {
        self.layout = self.layout.with_offset(offset);
        self
    }
}

impl<'a> Plot3D for Quads3D<'a> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DScope<'_>) -> Result<(), Plot3DError> {
        validate_item_label(self.label)?;
        if let Some(pts) = self.points_f32 {
            primitive_vertex_count(
                pts.len(),
                4,
                "quads(points)",
                self.layout,
                !self.flags.contains(Quad3DFlags::NO_LINES),
            )?;
            let mut xs = Vec::with_capacity(pts.len());
            let mut ys = Vec::with_capacity(pts.len());
            let mut zs = Vec::with_capacity(pts.len());
            for p in pts {
                xs.push(p[0]);
                ys.push(p[1]);
                zs.push(p[2]);
            }
            // The point rows were split into contiguous, equally sized coordinate arrays.
            unsafe { ui.plot_quads_f32_raw(self.label, &xs, &ys, &zs, self.flags, self.layout) };
            return Ok(());
        }
        if let Some(pts) = self.points_f64 {
            primitive_vertex_count(
                pts.len(),
                4,
                "quads(points)",
                self.layout,
                !self.flags.contains(Quad3DFlags::NO_LINES),
            )?;
            let mut xs = Vec::with_capacity(pts.len());
            let mut ys = Vec::with_capacity(pts.len());
            let mut zs = Vec::with_capacity(pts.len());
            for p in pts {
                xs.push(p[0]);
                ys.push(p[1]);
                zs.push(p[2]);
            }
            // The point rows were split into contiguous, equally sized coordinate arrays.
            unsafe { ui.plot_quads_f64_raw(self.label, &xs, &ys, &zs, self.flags, self.layout) };
            return Ok(());
        }
        if let (Some(x), Some(y), Some(z)) = (self.xs_f32, self.ys_f32, self.zs_f32) {
            validate_lengths(x, y, "x/y")?;
            validate_lengths(y, z, "y/z")?;
            primitive_vertex_count(
                x.len(),
                4,
                "quads",
                self.layout,
                !self.flags.contains(Quad3DFlags::NO_LINES),
            )?;
            unsafe {
                // High-level plots keep the automatic contiguous stride; offset is normalized by
                // ImPlot3D within the validated slice count.
                ui.plot_quads_f32_raw(self.label, x, y, z, self.flags, self.layout);
            }
            return Ok(());
        }
        if let (Some(x), Some(y), Some(z)) = (self.xs_f64, self.ys_f64, self.zs_f64) {
            validate_lengths(x, y, "x/y")?;
            validate_lengths(y, z, "y/z")?;
            primitive_vertex_count(
                x.len(),
                4,
                "quads",
                self.layout,
                !self.flags.contains(Quad3DFlags::NO_LINES),
            )?;
            unsafe {
                // High-level plots keep the automatic contiguous stride; offset is normalized by
                // ImPlot3D within the validated slice count.
                ui.plot_quads_f64_raw(self.label, x, y, z, self.flags, self.layout);
            }
            return Ok(());
        }
        Err(Plot3DError::EmptyData)
    }
}
