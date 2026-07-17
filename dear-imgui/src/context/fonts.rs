use crate::fonts::{FontAtlas, SharedFontAtlas};

use super::Context;
use super::binding::CTX_MUTEX;

impl Context {
    /// Borrow the font atlas from the IO structure.
    ///
    /// Font-atlas mutation is exposed through this shared view because Dear ImGui permits one
    /// native atlas to be registered with multiple contexts. Mutating methods validate the native
    /// atlas lock before entering FFI.
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

    /// Attempts to clone the interior shared font atlas **if it exists**.
    pub fn clone_shared_font_atlas(&self) -> Option<SharedFontAtlas> {
        self.shared_font_atlas.clone()
    }
}
