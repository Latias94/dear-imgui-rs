use crate::fonts::Font;
use crate::fonts::atlas::id::validate_font_for_atlas;
use crate::fonts::atlas::state::bump_font_atlas_generation;
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Remove a font from the atlas.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "RemoveFont")]
    pub fn remove_font(&mut self, font: &mut Font) {
        let font = validate_font_for_atlas(font, self.raw, "FontAtlas::remove_font()");
        unsafe { sys::ImFontAtlas_RemoveFont(self.raw, font) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear all fonts and texture data.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "Clear")]
    pub fn clear(&mut self) {
        unsafe { sys::ImFontAtlas_Clear(self.raw) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear only the fonts (keep texture data).
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&mut self) {
        unsafe { sys::ImFontAtlas_ClearFonts(self.raw) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear only the texture data (keep fonts)
    #[doc(alias = "ClearTexData")]
    pub fn clear_tex_data(&mut self) {
        unsafe { sys::ImFontAtlas_ClearTexData(self.raw) }
    }
}
