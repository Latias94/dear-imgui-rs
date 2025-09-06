//! Font runtime data and operations
//!
//! This module provides the Font type which represents a single font instance
//! with its associated runtime data and rendering operations.

use crate::sys;
use super::FontId;
use std::marker::PhantomData;

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
#[derive(Debug)]
pub struct Font {
    raw: *mut sys::ImFont,
    _phantom: PhantomData<*mut sys::ImFont>,
}

impl Font {
    /// Creates a Font wrapper from a raw ImFont pointer
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and points to a valid ImFont
    pub unsafe fn from_raw(raw: *mut sys::ImFont) -> &'static Self {
        &*(raw as *const Self)
    }

    /// Returns the identifier of this font
    pub fn id(&self) -> FontId {
        FontId(self as *const _)
    }

    /// Returns the raw ImFont pointer
    pub fn raw(&self) -> *mut sys::ImFont {
        self.raw
    }

    /// Check if a glyph is available in this font
    #[doc(alias = "IsGlyphInFont")]
    pub fn is_glyph_in_font(&self, c: char) -> bool {
        unsafe { sys::ImFont_IsGlyphInFont(self.raw, c as u32) }
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
            let text_start = text.as_ptr() as *const std::os::raw::c_char;
            let text_end = text_start.add(text.len());
            let result = sys::ImFont_CalcTextSizeA(
                self.raw,
                size,
                max_width,
                wrap_width,
                text_start,
                text_end,
                std::ptr::null_mut(),
            );
            [result.x, result.y]
        }
    }

    /// Calculate word wrap position for the given text
    #[doc(alias = "CalcWordWrapPosition")]
    pub fn calc_word_wrap_position(&self, size: f32, text: &str, wrap_width: f32) -> usize {
        unsafe {
            let text_start = text.as_ptr() as *const std::os::raw::c_char;
            let text_end = text_start.add(text.len());
            let wrap_pos =
                sys::ImFont_CalcWordWrapPosition(self.raw, size, text_start, text_end, wrap_width);
            wrap_pos.offset_from(text_start) as usize
        }
    }

    /// Clear output data (glyphs storage, UV coordinates)
    #[doc(alias = "ClearOutputData")]
    pub fn clear_output_data(&mut self) {
        unsafe { sys::ImFont_ClearOutputData(self.raw) }
    }

    /// Add character remapping
    #[doc(alias = "AddRemapChar")]
    pub fn add_remap_char(&mut self, from: char, to: char) {
        unsafe { sys::ImFont_AddRemapChar(self.raw, from as u32, to as u32) }
    }

    /// Check if a glyph range is unused
    #[doc(alias = "IsGlyphRangeUnused")]
    pub fn is_glyph_range_unused(&self, c_begin: u32, c_last: u32) -> bool {
        unsafe { sys::ImFont_IsGlyphRangeUnused(self.raw, c_begin, c_last) }
    }
}

// Font is safe to send between threads as long as the ImGui context is not being used
unsafe impl Send for Font {}
// Font is safe to share between threads as long as access is synchronized
unsafe impl Sync for Font {}
