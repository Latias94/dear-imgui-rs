use crate::fonts::FontId;
use crate::fonts::atlas::id::validate_font_id_for_atlas;
use crate::fonts::atlas::state::{
    bump_custom_rect_generation, bump_font_atlas_generation, reset_font_atlas_mode_after_full_clear,
};
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Remove a font from the atlas.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "RemoveFont")]
    pub fn remove_font(&self, font: FontId) {
        self.assert_mutation_allowed("FontAtlas::remove_font()");
        let raw = self.raw();
        let font = validate_font_id_for_atlas(font, raw, "FontAtlas::remove_font()");
        unsafe { sys::ImFontAtlas_RemoveFont(raw, font) }
        bump_font_atlas_generation(raw);
    }

    /// Clear all fonts and texture data.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    /// A legacy renderer claim is released only when no [`LegacyFontAtlas`](crate::LegacyFontAtlas)
    /// capability remains alive. Drop every legacy capability before calling this method when
    /// preparing the atlas for a managed renderer.
    #[doc(alias = "Clear")]
    pub fn clear(&self) {
        self.assert_mutation_allowed("FontAtlas::clear()");
        let raw = self.raw();
        unsafe { sys::ImFontAtlas_Clear(raw) }
        bump_font_atlas_generation(raw);
        bump_custom_rect_generation(raw);
        reset_font_atlas_mode_after_full_clear(raw);
    }

    /// Clear only the fonts (keep texture data).
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&self) {
        self.assert_mutation_allowed("FontAtlas::clear_fonts()");
        let raw = self.raw();
        unsafe { sys::ImFontAtlas_ClearFonts(raw) }
        bump_font_atlas_generation(raw);
        bump_custom_rect_generation(raw);
    }
}

impl crate::fonts::atlas::LegacyFontAtlas<'_> {
    /// Clear only the CPU atlas texture data while keeping font sources.
    ///
    /// This does not release any renderer-owned GPU texture. The legacy renderer remains
    /// responsible for retiring that resource after its last use.
    #[doc(alias = "ClearTexData")]
    pub fn clear_cpu_texture_data(&self) {
        self.atlas
            .assert_mutation_allowed("LegacyFontAtlas::clear_cpu_texture_data()");
        unsafe { sys::ImFontAtlas_ClearTexData(self.atlas.raw()) }
    }
}
