use super::{Plot3D, Plot3DError};
use crate::{Image3DFlags, Plot3DUi};
use dear_imgui_rs::texture::TextureRef;

pub struct Image3DByAxes<'a, T: Into<TextureRef> + Copy> {
    pub label: &'a str,
    pub tex: T,
    pub center: [f32; 3],
    pub axis_u: [f32; 3],
    pub axis_v: [f32; 3],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub tint: [f32; 4],
    pub flags: Image3DFlags,
}

impl<'a, T: Into<TextureRef> + Copy> Image3DByAxes<'a, T> {
    pub fn new(
        label: &'a str,
        tex: T,
        center: [f32; 3],
        axis_u: [f32; 3],
        axis_v: [f32; 3],
    ) -> Self {
        Self {
            label,
            tex,
            center,
            axis_u,
            axis_v,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            flags: Image3DFlags::NONE,
        }
    }
    pub fn uv(mut self, uv0: [f32; 2], uv1: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self.uv1 = uv1;
        self
    }
    pub fn tint(mut self, col: [f32; 4]) -> Self {
        self.tint = col;
        self
    }
    pub fn flags(mut self, flags: Image3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn alpha(mut self, a: f32) -> Self {
        self.tint[3] = a;
        self
    }
    pub fn uv_rect(mut self, u0: f32, v0: f32, u1: f32, v1: f32) -> Self {
        self.uv0 = [u0, v0];
        self.uv1 = [u1, v1];
        self
    }
    pub fn flip_v(mut self) -> Self {
        let (u0, v0) = (self.uv0[0], self.uv0[1]);
        let (u1, v1) = (self.uv1[0], self.uv1[1]);
        self.uv0 = [u0, v1];
        self.uv1 = [u1, v0];
        self
    }
}

impl<'a, T: Into<TextureRef> + Copy> Plot3D for Image3DByAxes<'a, T> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        ui.image_by_axes(self.label, self.tex, self.center, self.axis_u, self.axis_v)
            .uv(self.uv0, self.uv1)
            .tint(self.tint)
            .flags(self.flags)
            .plot();
        Ok(())
    }
}

pub struct Image3DByCorners<'a, T: Into<TextureRef> + Copy> {
    pub label: &'a str,
    pub tex: T,
    pub p0: [f32; 3],
    pub p1: [f32; 3],
    pub p2: [f32; 3],
    pub p3: [f32; 3],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub uv2: [f32; 2],
    pub uv3: [f32; 2],
    pub tint: [f32; 4],
    pub flags: Image3DFlags,
}

impl<'a, T: Into<TextureRef> + Copy> Image3DByCorners<'a, T> {
    pub fn new(
        label: &'a str,
        tex: T,
        p0: [f32; 3],
        p1: [f32; 3],
        p2: [f32; 3],
        p3: [f32; 3],
    ) -> Self {
        Self {
            label,
            tex,
            p0,
            p1,
            p2,
            p3,
            uv0: [0.0, 0.0],
            uv1: [1.0, 0.0],
            uv2: [1.0, 1.0],
            uv3: [0.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            flags: Image3DFlags::NONE,
        }
    }
    pub fn uvs(mut self, uv0: [f32; 2], uv1: [f32; 2], uv2: [f32; 2], uv3: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self.uv1 = uv1;
        self.uv2 = uv2;
        self.uv3 = uv3;
        self
    }
    pub fn tint(mut self, col: [f32; 4]) -> Self {
        self.tint = col;
        self
    }
    pub fn flags(mut self, flags: Image3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn alpha(mut self, a: f32) -> Self {
        self.tint[3] = a;
        self
    }
    pub fn flip_v(mut self) -> Self {
        std::mem::swap(&mut self.uv0, &mut self.uv3);
        std::mem::swap(&mut self.uv1, &mut self.uv2);
        self
    }
}

impl<'a, T: Into<TextureRef> + Copy> Plot3D for Image3DByCorners<'a, T> {
    fn label(&self) -> &str {
        self.label
    }
    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        ui.image_by_corners(self.label, self.tex, self.p0, self.p1, self.p2, self.p3)
            .uvs(self.uv0, self.uv1, self.uv2, self.uv3)
            .tint(self.tint)
            .flags(self.flags)
            .plot();
        Ok(())
    }
}
