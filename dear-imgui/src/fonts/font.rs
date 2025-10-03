//! Font runtime data and operations
//!
//! This module provides the Font type which represents a single font instance
//! with its associated runtime data and rendering operations.

use super::FontId;
use crate::sys;

/// A font instance with runtime data
///
/// This represents a single font that can be used for text rendering.
/// Fonts are managed by the FontAtlas and should not be created directly.
///
/// TODO: Currently using pointer wrapper approach for simplicity.
/// Future improvement: Implement complete field mapping like imgui-rs for better type safety.
///
/// Note: Our dear-imgui-sys uses newer ImGui version with ImFontBaked architecture,
/// while imgui-rs uses older version with direct field mapping. This difference
/// requires careful consideration when implementing full mapping.
#[repr(transparent)]
#[derive(Debug)]
pub struct Font(sys::ImFont);

impl Font {
    /// Constructs a shared reference from a raw pointer.
    ///
    /// Safety: caller guarantees the pointer is valid for the returned lifetime.
    pub(crate) unsafe fn from_raw<'a>(raw: *const sys::ImFont) -> &'a Self {
        unsafe { &*(raw as *const Self) }
    }

    /// Constructs a mutable reference from a raw pointer.
    ///
    /// Safety: caller guarantees the pointer is valid and uniquely borrowed for the returned lifetime.
    pub(crate) unsafe fn from_raw_mut<'a>(raw: *mut sys::ImFont) -> &'a mut Self {
        unsafe { &mut *(raw as *mut Self) }
    }

    /// Returns the identifier of this font
    pub fn id(&self) -> FontId {
        FontId(self.raw() as *const sys::ImFont)
    }

    /// Returns the raw ImFont pointer
    pub fn raw(&self) -> *mut sys::ImFont {
        self as *const Self as *mut sys::ImFont
    }

    /// Check if a glyph is available in this font
    #[doc(alias = "IsGlyphInFont")]
    pub fn is_glyph_in_font(&self, c: char) -> bool {
        unsafe { sys::ImFont_IsGlyphInFont(self.raw(), c as u16) }
    }

    /// Calculate text size for the given text
    #[doc(alias = "CalcTextSizeA")]
    pub fn calc_text_size(
        &self,
        size: f32,
        max_width: f32,
        wrap_width: f32,
        text: &str,
    ) -> [f32; 2] {
        unsafe {
            let text_cstr = std::ffi::CString::new(text).unwrap();
            let mut out = sys::ImVec2 { x: 0.0, y: 0.0 };
            let mut out_remaining: *const std::os::raw::c_char = std::ptr::null();
            sys::ImFont_CalcTextSizeA_Str(
                &mut out,
                self.raw(),
                size,
                max_width,
                wrap_width,
                text_cstr.as_ptr(),
                &mut out_remaining,
            );
            [out.x, out.y]
        }
    }

    /// Calculate word wrap position for the given text
    #[doc(alias = "CalcWordWrapPosition")]
    pub fn calc_word_wrap_position(&self, size: f32, text: &str, wrap_width: f32) -> usize {
        unsafe {
            let text_cstr = std::ffi::CString::new(text).unwrap();
            let text_start = text_cstr.as_ptr();
            let wrap_pos = sys::ImFont_CalcWordWrapPosition_Str(
                self.raw(),
                size,
                text_start,
                wrap_width,
            );
            wrap_pos.offset_from(text_start) as usize
        }
    }

    /// Clear output data (glyphs storage, UV coordinates)
    #[doc(alias = "ClearOutputData")]
    pub fn clear_output_data(&mut self) {
        unsafe { sys::ImFont_ClearOutputData(self.raw()) }
    }

    /// Add character remapping
    #[doc(alias = "AddRemapChar")]
    pub fn add_remap_char(&mut self, from: char, to: char) {
        unsafe { sys::ImFont_AddRemapChar(self.raw(), from as u16, to as u16) }
    }

    /// Check if a glyph range is unused
    #[doc(alias = "IsGlyphRangeUnused")]
    pub fn is_glyph_range_unused(&self, c_begin: u32, c_last: u32) -> bool {
        unsafe { sys::ImFont_IsGlyphRangeUnused(self.raw(), c_begin, c_last) }
    }
}

// NOTE: Do not mark Font as Send/Sync. It refers to memory owned by ImGui
// context and is not thread-safe to move/share across threads.
