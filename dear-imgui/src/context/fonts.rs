use crate::fonts::{FontAtlas, SharedFontAtlas};

use super::Context;
use super::binding::CTX_MUTEX;

impl Context {
    /// Borrow the font atlas from the IO structure.
    ///
    /// Contexts on the same OS thread may share a [`SharedFontAtlas`]. Configure fonts and
    /// borrow CPU texture data between frames. To draw an atlas image in an open frame, use
    /// [`Ui::font_atlas_texture`](crate::Ui::font_atlas_texture).
    pub fn font_atlas(&self) -> &FontAtlas {
        let _guard = CTX_MUTEX.lock();

        // wasm32 import-style builds keep Dear ImGui state in a separate module
        // and share linear memory. When the experimental font-atlas feature is
        // enabled, we allow direct access to the atlas pointer, assuming the
        // provider has been correctly configured via xtask.
        #[cfg(all(target_arch = "wasm32", feature = "wasm-font-atlas-experimental"))]
        unsafe {
            let io = self.io_ptr("Context::font_atlas()");
            let atlas_ptr = (*io).Fonts;
            assert!(
                !atlas_ptr.is_null(),
                "ImGui IO Fonts pointer is null on wasm; provider not initialized?"
            );
            FontAtlas::from_raw(atlas_ptr)
        }

        // Default wasm path: keep this API disabled to avoid accidental UB.
        #[cfg(all(target_arch = "wasm32", not(feature = "wasm-font-atlas-experimental")))]
        {
            panic!(
                "font_atlas() is not supported on wasm32 targets without \
                 `wasm-font-atlas-experimental` feature; \
                 see docs/WASM.md for current limitations."
            );
        }

        #[cfg(not(target_arch = "wasm32"))]
        unsafe {
            let io = self.io_ptr("Context::font_atlas()");
            let atlas_ptr = (*io).Fonts;
            FontAtlas::from_raw(atlas_ptr)
        }
    }

    /// Clone the shared atlas supplied at creation, if any. The clone keeps this OS thread as
    /// Dear ImGui's owner after the Context is dropped.
    pub fn clone_shared_font_atlas(&self) -> Option<SharedFontAtlas> {
        self.shared_font_atlas.clone()
    }
}
