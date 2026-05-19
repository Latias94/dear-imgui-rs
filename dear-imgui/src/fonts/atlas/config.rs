use std::ffi::c_char;
use std::ptr;

use crate::sys;

use super::loader::{FontLoader, FontLoaderFlags};
use super::validation::{
    RASTERIZER_MULTIPLY_MAX, assert_finite_f32, assert_finite_vec2,
    assert_font_source_for_add_font, assert_non_negative_f32, assert_non_negative_i8,
    assert_positive_f32, assert_reference_font_size_for_metrics, validate_font_size_pixels,
};

/// Font configuration for loading fonts with v1.92+ features
#[derive(Debug)]
pub struct FontConfig {
    pub(super) raw: sys::ImFontConfig,
    glyph_exclude_ranges: Option<Vec<sys::ImWchar>>,
}

impl Clone for FontConfig {
    fn clone(&self) -> Self {
        let mut raw = self.raw;
        let glyph_exclude_ranges = self.glyph_exclude_ranges.clone();
        if let Some(ref ranges) = glyph_exclude_ranges {
            raw.GlyphExcludeRanges = ranges.as_ptr();
        }
        Self {
            raw,
            glyph_exclude_ranges,
        }
    }
}

impl FontConfig {
    /// Creates a new font configuration with default settings
    pub fn new() -> Self {
        // Use ImGui's constructor to ensure all defaults are initialized
        // (e.g., RasterizerDensity defaults to 1.0f which avoids assertions).
        unsafe {
            let cfg = sys::ImFontConfig_ImFontConfig();
            if cfg.is_null() {
                panic!("ImFontConfig_ImFontConfig() returned null");
            }
            let raw = *cfg;
            sys::ImFontConfig_destroy(cfg);
            Self {
                raw,
                glyph_exclude_ranges: None,
            }
        }
    }

    /// Returns the raw ImFontConfig pointer
    pub(crate) fn raw(&self) -> *const sys::ImFontConfig {
        &self.raw
    }

    fn has_reference_size_dependent_metrics(&self) -> bool {
        self.raw.GlyphOffset.x != 0.0
            || self.raw.GlyphOffset.y != 0.0
            || self.raw.GlyphMinAdvanceX != 0.0
            || self.raw.GlyphMaxAdvanceX != f32::MAX
    }

    fn validate_common(&self, caller: &str) {
        validate_font_size_pixels(caller, "SizePixels", self.raw.SizePixels);
        assert_finite_vec2(
            caller,
            "GlyphOffset",
            [self.raw.GlyphOffset.x, self.raw.GlyphOffset.y],
        );
        assert_non_negative_f32(caller, "GlyphMinAdvanceX", self.raw.GlyphMinAdvanceX);
        assert_non_negative_f32(caller, "GlyphMaxAdvanceX", self.raw.GlyphMaxAdvanceX);
        assert!(
            self.raw.GlyphMinAdvanceX <= self.raw.GlyphMaxAdvanceX,
            "{caller} GlyphMinAdvanceX must be less than or equal to GlyphMaxAdvanceX"
        );
        assert_finite_f32(caller, "GlyphExtraAdvanceX", self.raw.GlyphExtraAdvanceX);
        assert_non_negative_f32(caller, "RasterizerMultiply", self.raw.RasterizerMultiply);
        assert!(
            self.raw.RasterizerMultiply <= RASTERIZER_MULTIPLY_MAX,
            "{caller} RasterizerMultiply must be less than or equal to {RASTERIZER_MULTIPLY_MAX}"
        );
        assert_positive_f32(caller, "RasterizerDensity", self.raw.RasterizerDensity);
        assert_non_negative_i8(caller, "OversampleH", self.raw.OversampleH);
        assert_non_negative_i8(caller, "OversampleV", self.raw.OversampleV);
    }

    pub(super) fn validate_for_add_font(&self, caller: &str) {
        self.validate_common(caller);
        assert_font_source_for_add_font(caller, &self.raw);
        assert_reference_font_size_for_metrics(
            caller,
            self.raw.SizePixels,
            self.has_reference_size_dependent_metrics(),
        );
    }

    pub(super) fn validate_for_add_font_default(&self, caller: &str) {
        self.validate_common(caller);
    }

    pub(super) fn validate_for_add_font_with_size(&self, caller: &str, size_pixels: f32) {
        self.validate_common(caller);
        let effective_size_pixels = if size_pixels > 0.0 {
            size_pixels
        } else {
            self.raw.SizePixels
        };
        assert_reference_font_size_for_metrics(
            caller,
            effective_size_pixels,
            self.has_reference_size_dependent_metrics(),
        );
    }

    /// Set the font size in pixels
    ///
    /// Note: With v1.92+ dynamic fonts, size can be 0.0 to use default sizing
    pub fn size_pixels(mut self, size: f32) -> Self {
        validate_font_size_pixels("FontConfig::size_pixels()", "size", size);
        self.raw.SizePixels = size;
        self
    }

    /// Set whether to merge this font with the previous one
    pub fn merge_mode(mut self, merge: bool) -> Self {
        self.raw.MergeMode = merge;
        self
    }

    /// Control whether the atlas takes ownership of `FontData` passed from memory.
    ///
    /// Dear ImGui's `AddFontFromMemoryTTF()` stores the `FontData` pointer for potential rebuilds.
    /// When this flag is `true`, the atlas will later free `FontData` using Dear ImGui's allocator.
    /// When it is `false`, Dear ImGui will *not* free the pointer and the caller must ensure the
    /// memory stays valid for as long as the atlas may use it.
    pub fn font_data_owned_by_atlas(mut self, owned: bool) -> Self {
        self.raw.FontDataOwnedByAtlas = owned;
        self
    }

    /// Set font loader flags for this specific font
    ///
    /// These flags override the global atlas flags for this font.
    pub fn font_loader_flags(mut self, flags: FontLoaderFlags) -> Self {
        self.raw.FontLoaderFlags = flags.0;
        self
    }

    /// Set inclusive glyph ranges to exclude from this font.
    ///
    /// The input is a slice of `(start, end)` pairs. It is converted to Dear ImGui's
    /// `[start, end, ..., 0]` format.
    pub fn glyph_exclude_ranges(mut self, ranges: &[(u32, u32)]) -> Self {
        if ranges.is_empty() {
            self.raw.GlyphExcludeRanges = ptr::null();
            self.glyph_exclude_ranges = None;
            return self;
        }

        const IMWCHAR_MAX: u32 = if std::mem::size_of::<sys::ImWchar>() == 2 {
            0xFFFF
        } else {
            0x10FFFF
        };
        let mut converted: Vec<sys::ImWchar> = Vec::with_capacity(ranges.len() * 2 + 1);
        for &(start, end) in ranges {
            assert!(
                start <= end,
                "glyph_exclude_ranges range start must be <= end: {start:#x}..={end:#x}"
            );
            assert!(
                start <= IMWCHAR_MAX,
                "glyph_exclude_ranges value out of range for ImWchar (max {IMWCHAR_MAX:#x}): {start:#x}"
            );
            assert!(
                end <= IMWCHAR_MAX,
                "glyph_exclude_ranges value out of range for ImWchar (max {IMWCHAR_MAX:#x}): {end:#x}"
            );
            let start = sys::ImWchar::try_from(start).unwrap_or_else(|_| {
                panic!("glyph_exclude_ranges value {start:#x} was not representable as ImWchar")
            });
            let end = sys::ImWchar::try_from(end).unwrap_or_else(|_| {
                panic!("glyph_exclude_ranges value {end:#x} was not representable as ImWchar")
            });
            converted.push(start);
            converted.push(end);
        }
        if converted.last().copied() != Some(0) {
            converted.push(0);
        }

        self.raw.GlyphExcludeRanges = converted.as_ptr();
        self.glyph_exclude_ranges = Some(converted);
        self
    }

    /// Set a custom font loader for this font.
    ///
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*` in the
    /// atlas font source.
    pub fn font_loader(mut self, loader: &'static FontLoader) -> Self {
        self.raw.FontLoader = loader.as_ptr();
        self
    }

    /// Set the font name for debugging
    pub fn name(mut self, name: &str) -> Self {
        let name_bytes = name.as_bytes();
        let copy_len = std::cmp::min(name_bytes.len(), self.raw.Name.len() - 1);

        // Clear the array first
        for i in 0..self.raw.Name.len() {
            self.raw.Name[i] = 0;
        }

        // Copy the name
        for (i, &byte) in name_bytes.iter().take(copy_len).enumerate() {
            self.raw.Name[i] = byte as c_char;
        }

        self
    }

    /// Set glyph offset for this font
    pub fn glyph_offset(mut self, offset: [f32; 2]) -> Self {
        assert_finite_vec2("FontConfig::glyph_offset()", "offset", offset);
        self.raw.GlyphOffset.x = offset[0];
        self.raw.GlyphOffset.y = offset[1];
        self
    }

    /// Set minimum advance X for glyphs
    pub fn glyph_min_advance_x(mut self, advance: f32) -> Self {
        assert_non_negative_f32("FontConfig::glyph_min_advance_x()", "advance", advance);
        assert!(
            advance <= self.raw.GlyphMaxAdvanceX,
            "FontConfig::glyph_min_advance_x() advance must be less than or equal to current glyph_max_advance_x"
        );
        self.raw.GlyphMinAdvanceX = advance;
        self
    }

    /// Set maximum advance X for glyphs
    pub fn glyph_max_advance_x(mut self, advance: f32) -> Self {
        assert_non_negative_f32("FontConfig::glyph_max_advance_x()", "advance", advance);
        assert!(
            advance >= self.raw.GlyphMinAdvanceX,
            "FontConfig::glyph_max_advance_x() advance must be greater than or equal to current glyph_min_advance_x"
        );
        self.raw.GlyphMaxAdvanceX = advance;
        self
    }

    /// Set extra advance X for glyphs (spacing between characters)
    pub fn glyph_extra_advance_x(mut self, advance: f32) -> Self {
        assert_finite_f32("FontConfig::glyph_extra_advance_x()", "advance", advance);
        self.raw.GlyphExtraAdvanceX = advance;
        self
    }

    /// Set rasterizer multiply factor
    pub fn rasterizer_multiply(mut self, multiply: f32) -> Self {
        assert_non_negative_f32("FontConfig::rasterizer_multiply()", "multiply", multiply);
        assert!(
            multiply <= RASTERIZER_MULTIPLY_MAX,
            "FontConfig::rasterizer_multiply() multiply must be less than or equal to {RASTERIZER_MULTIPLY_MAX}"
        );
        self.raw.RasterizerMultiply = multiply;
        self
    }

    /// Set rasterizer density for DPI scaling
    pub fn rasterizer_density(mut self, density: f32) -> Self {
        assert_positive_f32("FontConfig::rasterizer_density()", "density", density);
        self.raw.RasterizerDensity = density;
        self
    }

    /// Set pixel snap horizontally
    pub fn pixel_snap_h(mut self, snap: bool) -> Self {
        self.raw.PixelSnapH = snap;
        self
    }

    /// Set horizontal oversampling
    pub fn oversample_h(mut self, oversample: i8) -> Self {
        assert_non_negative_i8("FontConfig::oversample_h()", "oversample", oversample);
        self.raw.OversampleH = oversample;
        self
    }

    /// Set vertical oversampling
    pub fn oversample_v(mut self, oversample: i8) -> Self {
        assert_non_negative_i8("FontConfig::oversample_v()", "oversample", oversample);
        self.raw.OversampleV = oversample;
        self
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self::new()
    }
}
