use std::marker::PhantomData;

use crate::fonts::atlas::id::FontId;
use crate::fonts::atlas::loader::FontLoader;
use crate::fonts::atlas::state::{font_atlas_state, forget_font_atlas_generation};
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Creates a new font atlas with default settings
    pub fn new() -> Self {
        unsafe {
            let raw = sys::ImFontAtlas_ImFontAtlas();
            if raw.is_null() {
                panic!("ImFontAtlas_ImFontAtlas() returned null");
            }
            font_atlas_state(raw);
            Self {
                raw,
                owned: true,
                _phantom: PhantomData,
            }
        }
    }

    /// Creates a new font atlas with a custom font loader.
    ///
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*`.
    pub fn with_font_loader(loader: &'static FontLoader) -> Self {
        let mut atlas = Self::new();
        atlas.set_font_loader(loader);
        atlas
    }

    /// Creates a FontAtlas wrapper from a raw ImFontAtlas pointer
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and points to a valid ImFontAtlas
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImFontAtlas) -> Self {
        assert!(
            !raw.is_null(),
            "FontAtlas::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw);
        Self {
            raw,
            owned: false,
            _phantom: PhantomData,
        }
    }

    /// Returns the raw ImFontAtlas pointer
    pub fn raw(&self) -> *mut sys::ImFontAtlas {
        self.raw
    }

    pub(crate) fn font_id_for_raw(&self, font: *mut sys::ImFont) -> FontId {
        FontId::from_raw_parts(font, self.raw)
    }
}

impl Default for FontAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FontAtlas {
    fn drop(&mut self) {
        if self.owned && !self.raw.is_null() {
            unsafe {
                forget_font_atlas_generation(self.raw);
                sys::ImFontAtlas_destroy(self.raw);
            }
        }
    }
}

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.
