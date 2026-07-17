use crate::fonts::atlas::state::bump_custom_rect_generation;
use crate::fonts::atlas::validation::frame_count_to_i32;
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Build the font atlas texture
    ///
    /// This is a simplified build process. For more control, use the individual build functions.
    ///
    /// Note: with Dear ImGui 1.92+ "new backend" texture system, you should generally
    /// not call `build()` manually. The renderer should set `ImGuiBackendFlags_RendererHasTextures`
    /// and the atlas will be built/updated on demand.
    ///
    /// In particular, calling `build()` before the renderer sets `RendererHasTextures`
    /// may cause Dear ImGui to assert on the next frame.
    #[doc(alias = "Build")]
    pub fn build(&self) -> bool {
        self.assert_mutation_allowed("FontAtlas::build()");
        let raw = self.raw();
        // NOTE: In Dear ImGui, `ImFontAtlasBuildMain()` will call `ImFontAtlasBuildInit()`
        // lazily if needed (Builder == NULL). Calling BuildInit unconditionally would leak
        // the builder and is not idempotent.
        unsafe {
            let rebuilds_builder =
                !(*raw).TexData.is_null() && (*(*raw).TexData).Format != (*raw).TexDesiredFormat;
            sys::igImFontAtlasBuildMain(raw);
            if rebuilds_builder {
                bump_custom_rect_generation(raw);
            }
            (*raw).TexIsBuilt
        }
    }

    /// Discard baked font caches.
    ///
    /// This clears cached glyph data (including cached "not found" entries) so that
    /// newly added font sources (e.g. merged CJK/emoji fonts) can take effect.
    ///
    /// Pass `unused_frames = 0` to discard everything (recommended after font merging).
    ///
    /// Notes:
    /// - Only call this when the atlas is not locked (typically before `Context::frame()`).
    /// - No-op if the atlas builder hasn't been created yet.
    #[doc(alias = "ImFontAtlasBuildDiscardBakes")]
    pub fn discard_bakes(&self, unused_frames: usize) {
        self.assert_mutation_allowed("FontAtlas::discard_bakes()");
        let raw = self.raw();
        let unused_frames =
            frame_count_to_i32("FontAtlas::discard_bakes()", "unused_frames", unused_frames);
        unsafe {
            if (*raw).Builder.is_null() {
                return;
            }
            sys::igImFontAtlasBuildDiscardBakes(raw, unused_frames);
        }
    }

    /// Compact cached glyphs and the current atlas texture.
    ///
    /// This is a no-op before the atlas builder and texture have been created.
    /// It must not be called while the atlas is locked by a frame.
    #[doc(alias = "CompactCache")]
    pub fn compact_cache(&self) {
        self.assert_mutation_allowed("FontAtlas::compact_cache()");
        let raw = self.raw();
        unsafe {
            if (*raw).Builder.is_null() || (*raw).TexData.is_null() {
                return;
            }
            sys::ImFontAtlas_CompactCache(raw);
        }
    }
}
