//! Image plot implementation

use super::{Plot, PlotError, plot_spec_from, with_plot_str_or_empty};
use crate::{ImageFlags, ItemFlags, sys};

/// Plot an image in plot coordinates using an ImTextureID
pub struct ImagePlot<'a> {
    label: &'a str,
    tex_id: sys::ImTextureID,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    uv0: [f32; 2],
    uv1: [f32; 2],
    tint: [f32; 4],
    flags: ImageFlags,
    item_flags: ItemFlags,
}

impl<'a> ImagePlot<'a> {
    pub fn new(
        label: &'a str,
        tex_id: sys::ImTextureID,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Self {
        Self {
            label,
            tex_id,
            bounds_min,
            bounds_max,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
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

impl<'a> Plot for ImagePlot<'a> {
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
        // Construct ImTextureRef from ImTextureID
        let tex_ref = sys::ImTextureRef_c {
            _TexData: std::ptr::null_mut(),
            _TexID: self.tex_id,
        };
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_from(
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
    pub fn plot_image(
        &self,
        label: &str,
        tex_id: sys::ImTextureID,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Result<(), PlotError> {
        let plot = ImagePlot::new(label, tex_id, bounds_min, bounds_max);
        plot.validate()?;
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
        // ImTextureID is ImU64 in the shared dear-imgui-sys bindings.
        let raw: sys::ImTextureID = texture.id();
        self.plot_image(label, raw, bounds_min, bounds_max)
    }
}
