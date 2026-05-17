//! Image plot implementation

use super::{Plot, PlotError, PlotItemStyle, plot_spec_with_style, with_plot_str_or_empty};
use crate::{ImageFlags, ItemFlags, sys};
use dear_imgui_rs::texture::TextureRef;
use std::marker::PhantomData;

fn to_sys_texture_ref<'tex>(texture: impl Into<TextureRef<'tex>>) -> sys::ImTextureRef_c {
    let texture = texture.into().raw();
    sys::ImTextureRef_c {
        _TexData: texture._TexData as *mut sys::ImTextureData,
        _TexID: texture._TexID as sys::ImTextureID,
    }
}

/// Plot an image in plot coordinates using an ImGui texture reference.
pub struct ImagePlot<'a, 'tex> {
    label: &'a str,
    tex_ref: sys::ImTextureRef_c,
    _texture: PhantomData<&'tex mut dear_imgui_rs::texture::TextureData>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    uv0: [f32; 2],
    uv1: [f32; 2],
    tint: [f32; 4],
    style: PlotItemStyle,
    flags: ImageFlags,
    item_flags: ItemFlags,
}

impl<'a, 'tex> super::PlotItemStyled for ImagePlot<'a, 'tex> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a, 'tex> ImagePlot<'a, 'tex> {
    pub fn new(
        label: &'a str,
        texture: impl Into<TextureRef<'tex>>,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Self {
        Self {
            label,
            tex_ref: to_sys_texture_ref(texture),
            _texture: PhantomData,
            bounds_min,
            bounds_max,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            style: PlotItemStyle::default(),
            flags: ImageFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    pub fn with_uv(mut self, uv0: [f32; 2], uv1: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self.uv1 = uv1;
        self
    }
    pub fn with_tint(mut self, tint: [f32; 4]) -> Self {
        self.tint = tint;
        self
    }
    pub fn with_flags(mut self, flags: ImageFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    pub fn validate(&self) -> Result<(), PlotError> {
        Ok(())
    }
}

impl<'a, 'tex> Plot for ImagePlot<'a, 'tex> {
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }
        let uv0 = sys::ImVec2_c {
            x: self.uv0[0],
            y: self.uv0[1],
        };
        let uv1 = sys::ImVec2_c {
            x: self.uv1[0],
            y: self.uv1[1],
        };
        let tint = sys::ImVec4_c {
            x: self.tint[0],
            y: self.tint[1],
            z: self.tint[2],
            w: self.tint[3],
        };
        let tex_ref = sys::ImTextureRef_c {
            _TexData: self.tex_ref._TexData,
            _TexID: self.tex_ref._TexID,
        };
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                0,
                crate::IMPLOT_AUTO,
            );
            sys::ImPlot_PlotImage(
                label_ptr,
                tex_ref,
                self.bounds_min,
                self.bounds_max,
                uv0,
                uv1,
                tint,
                spec,
            )
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience methods on PlotUi
impl<'ui> crate::PlotUi<'ui> {
    pub fn plot_image<'tex>(
        &self,
        label: &str,
        texture: impl Into<TextureRef<'tex>>,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Result<(), PlotError> {
        let plot = ImagePlot::new(label, texture, bounds_min, bounds_max);
        plot.validate()?;
        self.bind();
        plot.plot();
        Ok(())
    }

    /// Plot an image using ImGui's TextureId wrapper (if available)
    #[allow(unused_variables)]
    pub fn plot_image_with_imgui_texture(
        &self,
        label: &str,
        texture: dear_imgui_rs::TextureId,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Result<(), PlotError> {
        self.plot_image(label, texture, bounds_min, bounds_max)
    }
}
