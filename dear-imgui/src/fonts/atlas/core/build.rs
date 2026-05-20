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
    pub fn build(&mut self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        // NOTE: In Dear ImGui, `ImFontAtlasBuildMain()` will call `ImFontAtlasBuildInit()`
        // lazily if needed (Builder == NULL). Calling BuildInit unconditionally would leak
        // the builder and is not idempotent.
        unsafe {
            sys::igImFontAtlasBuildMain(self.raw);
            (*self.raw).TexIsBuilt
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
    pub fn discard_bakes(&mut self, unused_frames: usize) {
        if self.raw.is_null() {
            return;
        }
        let unused_frames =
            frame_count_to_i32("FontAtlas::discard_bakes()", "unused_frames", unused_frames);
        unsafe {
            if (*self.raw).Builder.is_null() {
                return;
            }
            sys::igImFontAtlasBuildDiscardBakes(self.raw, unused_frames);
        }
    }
}
