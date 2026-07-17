use crate::fonts::atlas::id::FontId;
use crate::fonts::atlas::state::{
    assert_no_font_atlas_texture_borrows, assert_no_open_font_atlas_frames, font_atlas_state,
};
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Creates a shared font-atlas view from a raw pointer.
    ///
    /// # Safety
    /// The pointer must remain valid and immutable for the returned lifetime.
    pub(crate) unsafe fn from_raw<'a>(raw: *const sys::ImFontAtlas) -> &'a Self {
        assert!(
            !raw.is_null(),
            "FontAtlas::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw.cast_mut());
        unsafe { &*raw.cast::<Self>() }
    }

    /// Returns the raw `ImFontAtlas` pointer for explicit FFI interop.
    ///
    /// The pointer is valid only while the atlas owner remains alive. Dereferencing it or passing
    /// it to native code is unsafe; mutating the atlas outside this wrapper can violate its handle,
    /// texture-lease, and frame-lifecycle checks.
    pub fn raw(&self) -> *mut sys::ImFontAtlas {
        self.0.get()
    }

    pub(crate) fn font_id_for_raw(&self, font: *mut sys::ImFont) -> FontId {
        FontId::from_raw_parts(font, self.raw())
    }

    pub(crate) fn assert_mutation_allowed(&self, caller: &str) {
        let raw = self.raw();
        assert_no_font_atlas_texture_borrows(raw, caller);
        assert_no_open_font_atlas_frames(raw, caller);
        assert!(
            !unsafe { (*raw).Locked },
            "{caller} cannot modify a locked font atlas"
        );
    }
}

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.
