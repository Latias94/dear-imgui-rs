use super::Style;
use super::validation::{assert_non_negative_f32, assert_positive_f32};

impl Style {
    /// Get main font scale (formerly io.FontGlobalScale)
    pub fn font_scale_main(&self) -> f32 {
        self.inner().FontScaleMain
    }

    /// Set main font scale (formerly io.FontGlobalScale)
    pub fn set_font_scale_main(&mut self, scale: f32) {
        assert_positive_f32("Style::set_font_scale_main()", "scale", scale);
        self.inner_mut().FontScaleMain = scale;
    }

    /// Get DPI font scale (auto-overwritten if ConfigDpiScaleFonts=true)
    pub fn font_scale_dpi(&self) -> f32 {
        self.inner().FontScaleDpi
    }

    /// Set DPI font scale
    pub fn set_font_scale_dpi(&mut self, scale: f32) {
        assert_positive_f32("Style::set_font_scale_dpi()", "scale", scale);
        self.inner_mut().FontScaleDpi = scale;
    }

    /// Base size used by style for font sizing
    pub fn font_size_base(&self) -> f32 {
        self.inner().FontSizeBase
    }

    pub fn set_font_size_base(&mut self, sz: f32) {
        assert_non_negative_f32("Style::set_font_size_base()", "sz", sz);
        self.inner_mut().FontSizeBase = sz;
    }
}
