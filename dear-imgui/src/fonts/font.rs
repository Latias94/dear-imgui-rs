//! Validated font source metadata.
//!
//! Size- and density-specific runtime data lives in [`BakedFont`](super::BakedFont).

use super::{FontId, atlas::validate_font_id};
use crate::sys;

impl FontId {
    /// Return whether this font has loaded runtime data.
    #[doc(alias = "IsLoaded")]
    pub fn is_loaded(self) -> bool {
        let raw = validate_font_id(self, "FontId::is_loaded()");
        unsafe { sys::ImFont_IsLoaded(raw) }
    }

    /// Return an owned copy of the name used by Dear ImGui's debug tools.
    #[doc(alias = "GetDebugName")]
    pub fn debug_name(self) -> String {
        let raw = validate_font_id(self, "FontId::debug_name()");
        unsafe {
            let name = sys::ImFont_GetDebugName(raw);
            if name.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(name)
                    .to_string_lossy()
                    .into_owned()
            }
        }
    }

    /// Return the number of source configurations merged into this font.
    pub fn source_count(self) -> usize {
        let raw = validate_font_id(self, "FontId::source_count()");
        usize::try_from(unsafe { (*raw).Sources.Size }).unwrap_or(0)
    }

    /// Check whether a glyph is available in this font.
    #[doc(alias = "IsGlyphInFont")]
    pub fn is_glyph_in_font(self, c: char) -> bool {
        let codepoint = c as u32;
        if std::mem::size_of::<sys::ImWchar>() == 2 && codepoint > u16::MAX as u32 {
            return false;
        }
        let raw = validate_font_id(self, "FontId::is_glyph_in_font()");
        unsafe { sys::ImFont_IsGlyphInFont(raw, codepoint as sys::ImWchar) }
    }

    /// Check whether a glyph range is unused by this font.
    #[doc(alias = "IsGlyphRangeUnused")]
    pub fn is_glyph_range_unused(self, c_begin: u32, c_last: u32) -> bool {
        const IMWCHAR_MAX: u32 = if std::mem::size_of::<sys::ImWchar>() == 2 {
            u16::MAX as u32
        } else {
            0x10FFFF
        };
        if c_begin > IMWCHAR_MAX {
            return true;
        }
        let raw = validate_font_id(self, "FontId::is_glyph_range_unused()");
        let c_last = c_last.min(IMWCHAR_MAX);
        unsafe {
            sys::ImFont_IsGlyphRangeUnused(raw, c_begin as sys::ImWchar, c_last as sys::ImWchar)
        }
    }
}

#[cfg(test)]
mod tests {
    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx
    }

    #[test]
    fn loaded_font_id_exposes_owned_metadata() {
        let mut ctx = setup_context();
        let ui = ctx.frame();
        let font = ui.current_font();

        assert!(font.is_loaded());
        assert!(!font.debug_name().is_empty());
    }
}
