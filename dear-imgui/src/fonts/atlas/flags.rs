use bitflags::bitflags;

use super::FontAtlas;
use crate::sys;

bitflags! {
    /// Atlas texture options, configured before its first build.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
    pub struct FontAtlasFlags: i32 {
        const NONE = sys::ImFontAtlasFlags_None as i32;
        /// Do not round the texture height to a power of two.
        const NO_POWER_OF_TWO_HEIGHT = sys::ImFontAtlasFlags_NoPowerOfTwoHeight as i32;
        /// Omit software mouse cursor images.
        const NO_MOUSE_CURSORS = sys::ImFontAtlasFlags_NoMouseCursors as i32;
        /// Omit line textures. This also disables line and line-end anti-aliasing.
        const NO_BAKED_LINES = sys::ImFontAtlasFlags_NoBakedLines as i32;
        /// Omit round-corner textures, using tessellated geometry instead.
        const NO_BAKED_ROUND_CORNERS = sys::ImFontAtlasFlags_NoBakedRoundCorners as i32;
    }
}

impl FontAtlas {
    /// Returns atlas texture options.
    pub fn flags(&self) -> FontAtlasFlags {
        FontAtlasFlags::from_bits_retain(unsafe { (*self.raw()).Flags })
    }

    /// Configures atlas texture options before its first build.
    ///
    /// # Panics
    ///
    /// Panics during an open frame, while texture data is borrowed, for unknown flags,
    /// or after building has started. Call [`Self::clear`] and repopulate the atlas
    /// before changing options on an existing atlas.
    pub fn set_flags(&self, flags: FontAtlasFlags) {
        self.assert_mutation_allowed("FontAtlas::set_flags()");
        assert!(
            flags.difference(FontAtlasFlags::all()).is_empty(),
            "FontAtlas::set_flags() requires known atlas flags"
        );
        let raw = self.raw();
        assert!(
            unsafe { (*raw).Builder.is_null() && !(*raw).TexIsBuilt },
            "FontAtlas::set_flags() requires an atlas that has not started building; clear and repopulate it first"
        );
        unsafe { (*raw).Flags = flags.bits() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_options_apply_before_build_and_reject_late_changes() {
        let mut ctx = crate::Context::create();
        let flags = FontAtlasFlags::NO_BAKED_LINES | FontAtlasFlags::NO_BAKED_ROUND_CORNERS;
        ctx.font_atlas().set_flags(flags);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        assert_eq!(ctx.font_atlas().flags(), flags);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ctx.font_atlas().set_flags(FontAtlasFlags::NONE);
        }));
        assert!(result.is_err());
        assert_eq!(ctx.font_atlas().flags(), flags);

        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.frame().text("atlas options");
        assert!(!ctx.style().anti_aliased_lines());
        assert!(!ctx.style().anti_aliased_line_ends());
        let _ = ctx.render_legacy();

        ctx.font_atlas().clear();
        ctx.font_atlas().set_flags(FontAtlasFlags::NO_MOUSE_CURSORS);
        ctx.font_atlas()
            .add_font(&[crate::FontSource::default_font()]);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        assert_eq!(ctx.font_atlas().flags(), FontAtlasFlags::NO_MOUSE_CURSORS);
    }
}
