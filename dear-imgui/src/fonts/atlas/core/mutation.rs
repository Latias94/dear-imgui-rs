use crate::fonts::FontId;
use crate::fonts::atlas::id::validate_font_id_for_atlas;
use crate::fonts::atlas::state::{
    bump_custom_rect_generation, bump_font_atlas_generation, bump_font_atlas_texture_generation,
    clear_font_atlas_glyph_ranges,
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
    #[doc(alias = "Clear")]
    pub fn clear(&self) {
        self.assert_mutation_allowed("FontAtlas::clear()");
        let raw = self.raw();
        unsafe { sys::ImFontAtlas_Clear(raw) }
        clear_font_atlas_glyph_ranges(raw);
        bump_font_atlas_generation(raw);
        bump_font_atlas_texture_generation(raw);
        bump_custom_rect_generation(raw);
    }

    /// Clear only the fonts (keep texture data).
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&self) {
        self.assert_mutation_allowed("FontAtlas::clear_fonts()");
        let raw = self.raw();
        unsafe { sys::ImFontAtlas_ClearFonts(raw) }
        clear_font_atlas_glyph_ranges(raw);
        bump_font_atlas_generation(raw);
        bump_custom_rect_generation(raw);
    }

    /// Clear only the texture data (keep fonts)
    #[doc(alias = "ClearTexData")]
    pub fn clear_tex_data(&self) {
        self.assert_mutation_allowed("FontAtlas::clear_tex_data()");
        let raw = self.raw();
        assert!(
            !unsafe { (*raw).RendererHasTextures },
            "FontAtlas::clear_tex_data() is only available to legacy renderers without RENDERER_HAS_TEXTURES"
        );
        unsafe { sys::ImFontAtlas_ClearTexData(raw) }
        bump_font_atlas_texture_generation(raw);
    }
}
