use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default_ptr(&self) -> *const sys::ImWchar {
        if self.raw.is_null() {
            return std::ptr::null();
        }
        unsafe { sys::ImFontAtlas_GetGlyphRangesDefault(self.raw) }
    }

    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    ///
    /// The returned slice is terminated by a `0` sentinel, matching Dear ImGui's
    /// `ImWchar` range list format: `[start, end, start, end, ..., 0]`.
    ///
    /// Prefer [`Self::get_glyph_ranges_default_ptr`] when passing glyph ranges
    /// back into FFI, to avoid any scanning on the Rust side.
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default(&self) -> &[sys::ImWchar] {
        unsafe {
            let ptr = self.get_glyph_ranges_default_ptr();
            if ptr.is_null() {
                &[]
            } else {
                // Count the ranges (terminated by 0). Dear ImGui stores the list as
                // pairs: [start, end, start, end, ..., 0].
                //
                // This assumes the pointer points to a valid, null-terminated
                // static array as provided by Dear ImGui.
                const MAX_WORDS: usize = 2048;
                let mut i = 0usize;
                while i < MAX_WORDS {
                    if *ptr.add(i) == 0 {
                        return std::slice::from_raw_parts(ptr, i + 1);
                    }
                    i = i.saturating_add(2);
                }

                debug_assert!(
                    false,
                    "ImFontAtlas_GetGlyphRangesDefault() did not terminate within MAX_WORDS"
                );
                &[]
            }
        }
    }
}
