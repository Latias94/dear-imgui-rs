use std::borrow::Cow;
use std::marker::PhantomData;

use dear_imgui_rs::texture::TextureRef;

use crate::item_style::{Plot3DItemStyle, plot3d_spec_with_style};
use crate::{
    Image3DFlags, Item3DFlags, Plot3DDataLayout, Plot3DUi, debug_before_plot, imvec2, imvec4, sys,
};

/// Image by axes builder
pub struct Image3DByAxesBuilder<'ui, 'tex> {
    pub(crate) _ui: &'ui Plot3DUi<'ui>,
    pub(crate) label: Cow<'ui, str>,
    pub(crate) tex_ref: sys::ImTextureRef_c,
    pub(crate) _texture: PhantomData<&'tex mut dear_imgui_rs::texture::TextureData>,
    pub(crate) center: [f32; 3],
    pub(crate) axis_u: [f32; 3],
    pub(crate) axis_v: [f32; 3],
    pub(crate) uv0: [f32; 2],
    pub(crate) uv1: [f32; 2],
    pub(crate) tint: [f32; 4],
    pub(crate) flags: Image3DFlags,
    pub(crate) item_flags: Item3DFlags,
    pub(crate) style: Plot3DItemStyle,
}

impl<'ui, 'tex> Image3DByAxesBuilder<'ui, 'tex> {
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
    pub fn plot(self) {
        let _guard = self._ui.bind();
        let label = self.label.as_ref();
        let label = if label.contains('\0') { "image" } else { label };
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            debug_before_plot();
            let spec = plot3d_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                Plot3DDataLayout::DEFAULT,
            );
            sys::ImPlot3D_PlotImage_Vec2(
                label_ptr,
                self.tex_ref,
                sys::ImPlot3DPoint_c {
                    x: self.center[0] as f64,
                    y: self.center[1] as f64,
                    z: self.center[2] as f64,
                },
                sys::ImPlot3DPoint_c {
                    x: self.axis_u[0] as f64,
                    y: self.axis_u[1] as f64,
                    z: self.axis_u[2] as f64,
                },
                sys::ImPlot3DPoint_c {
                    x: self.axis_v[0] as f64,
                    y: self.axis_v[1] as f64,
                    z: self.axis_v[2] as f64,
                },
                imvec2(self.uv0[0], self.uv0[1]),
                imvec2(self.uv1[0], self.uv1[1]),
                imvec4(self.tint[0], self.tint[1], self.tint[2], self.tint[3]),
                spec,
            );
        })
    }
}

/// Image by corners builder
pub struct Image3DByCornersBuilder<'ui, 'tex> {
    pub(crate) _ui: &'ui Plot3DUi<'ui>,
    pub(crate) label: Cow<'ui, str>,
    pub(crate) tex_ref: sys::ImTextureRef_c,
    pub(crate) _texture: PhantomData<&'tex mut dear_imgui_rs::texture::TextureData>,
    pub(crate) p0: [f32; 3],
    pub(crate) p1: [f32; 3],
    pub(crate) p2: [f32; 3],
    pub(crate) p3: [f32; 3],
    pub(crate) uv0: [f32; 2],
    pub(crate) uv1: [f32; 2],
    pub(crate) uv2: [f32; 2],
    pub(crate) uv3: [f32; 2],
    pub(crate) tint: [f32; 4],
    pub(crate) flags: Image3DFlags,
    pub(crate) item_flags: Item3DFlags,
    pub(crate) style: Plot3DItemStyle,
}

impl<'ui, 'tex> Image3DByCornersBuilder<'ui, 'tex> {
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
    pub fn plot(self) {
        let _guard = self._ui.bind();
        let label = self.label.as_ref();
        let label = if label.contains('\0') { "image" } else { label };
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            debug_before_plot();
            let spec = plot3d_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                Plot3DDataLayout::DEFAULT,
            );
            sys::ImPlot3D_PlotImage_Plot3DPoint(
                label_ptr,
                self.tex_ref,
                sys::ImPlot3DPoint_c {
                    x: self.p0[0] as f64,
                    y: self.p0[1] as f64,
                    z: self.p0[2] as f64,
                },
                sys::ImPlot3DPoint_c {
                    x: self.p1[0] as f64,
                    y: self.p1[1] as f64,
                    z: self.p1[2] as f64,
                },
                sys::ImPlot3DPoint_c {
                    x: self.p2[0] as f64,
                    y: self.p2[1] as f64,
                    z: self.p2[2] as f64,
                },
                sys::ImPlot3DPoint_c {
                    x: self.p3[0] as f64,
                    y: self.p3[1] as f64,
                    z: self.p3[2] as f64,
                },
                imvec2(self.uv0[0], self.uv0[1]),
                imvec2(self.uv1[0], self.uv1[1]),
                imvec2(self.uv2[0], self.uv2[1]),
                imvec2(self.uv3[0], self.uv3[1]),
                imvec4(self.tint[0], self.tint[1], self.tint[2], self.tint[3]),
                spec,
            );
        })
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Image oriented by center and axes
    pub fn image_by_axes<'tex, T: Into<TextureRef<'tex>>>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        tex: T,
        center: [f32; 3],
        axis_u: [f32; 3],
        axis_v: [f32; 3],
    ) -> Image3DByAxesBuilder<'ui, 'tex> {
        let _guard = self.bind();
        let tr = tex.into().raw();
        let tex_ref = sys::ImTextureRef_c {
            _TexData: tr._TexData as *mut sys::ImTextureData,
            _TexID: tr._TexID as sys::ImTextureID,
        };
        debug_before_plot();
        Image3DByAxesBuilder {
            _ui: self,
            label: label.into(),
            tex_ref,
            _texture: PhantomData,
            center,
            axis_u,
            axis_v,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            flags: Image3DFlags::NONE,
            item_flags: Item3DFlags::NONE,
            style: Plot3DItemStyle::default(),
        }
    }

    /// Image by 4 corner points (p0..p3)
    pub fn image_by_corners<'tex, T: Into<TextureRef<'tex>>>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        tex: T,
        p0: [f32; 3],
        p1: [f32; 3],
        p2: [f32; 3],
        p3: [f32; 3],
    ) -> Image3DByCornersBuilder<'ui, 'tex> {
        let _guard = self.bind();
        let tr = tex.into().raw();
        let tex_ref = sys::ImTextureRef_c {
            _TexData: tr._TexData as *mut sys::ImTextureData,
            _TexID: tr._TexID as sys::ImTextureID,
        };
        debug_before_plot();
        Image3DByCornersBuilder {
            _ui: self,
            label: label.into(),
            tex_ref,
            _texture: PhantomData,
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
            item_flags: Item3DFlags::NONE,
            style: Plot3DItemStyle::default(),
        }
    }
}
