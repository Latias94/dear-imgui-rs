//! Font runtime data and operations
//!
//! This module provides the Font type which represents a single font instance
//! with its associated runtime data and rendering operations.

use super::FontId;
use crate::sys;
use std::cell::UnsafeCell;

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
pub struct Font(UnsafeCell<sys::ImFont>);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImFont>()] = [(); std::mem::size_of::<Font>()];
const _: [(); std::mem::align_of::<sys::ImFont>()] = [(); std::mem::align_of::<Font>()];

impl Font {
    #[inline]
    fn inner(&self) -> &sys::ImFont {
        // Safety: this wrapper is a view into an ImGui-owned `ImFont`. Dear ImGui may mutate the
        // font data while Rust holds `&Font`, so we store the value behind `UnsafeCell` to make
        // that interior mutability explicit.
        unsafe { &*self.0.get() }
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut sys::ImFont {
        // Safety: caller has `&mut Font`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.0.get() }
    }

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
        self.0.get()
    }

    /// Check if a glyph is available in this font
    #[doc(alias = "IsGlyphInFont")]
    pub fn is_glyph_in_font(&self, c: char) -> bool {
        let codepoint = c as u32;
        if std::mem::size_of::<sys::ImWchar>() == 2 && codepoint > 0xFFFF {
            return false;
        }
        unsafe { sys::ImFont_IsGlyphInFont(self.raw(), codepoint as sys::ImWchar) }
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
            let mut out_remaining: *const std::os::raw::c_char = std::ptr::null();
            let out = sys::ImFont_CalcTextSizeA(
                self.raw(),
                size,
                max_width,
                wrap_width,
                text_start,
                text_end,
                &mut out_remaining,
            );
            [out.x, out.y]
        }
    }

    /// Calculate word wrap position for the given text
    #[doc(alias = "CalcWordWrapPosition")]
    pub fn calc_word_wrap_position(&self, size: f32, text: &str, wrap_width: f32) -> usize {
        unsafe {
            let text_start = text.as_ptr() as *const std::os::raw::c_char;
            let text_end = text_start.add(text.len());
            let wrap_pos = sys::ImFont_CalcWordWrapPosition(
                self.raw(),
                size,
                text_start,
                text_end,
                wrap_width,
            );
            let off = wrap_pos.offset_from(text_start);
            if off <= 0 {
                0
            } else {
                (off as usize).min(text.len())
            }
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
        let from = from as u32;
        let to = to as u32;
        if std::mem::size_of::<sys::ImWchar>() == 2 && (from > 0xFFFF || to > 0xFFFF) {
            return;
        }
        unsafe { sys::ImFont_AddRemapChar(self.raw(), from as sys::ImWchar, to as sys::ImWchar) }
    }

    /// Check if a glyph range is unused
    #[doc(alias = "IsGlyphRangeUnused")]
    pub fn is_glyph_range_unused(&self, c_begin: u32, c_last: u32) -> bool {
        const IMWCHAR_MAX: u32 = if std::mem::size_of::<sys::ImWchar>() == 2 {
            0xFFFF
        } else {
            0x10FFFF
        };
        if c_begin > IMWCHAR_MAX {
            return true;
        }
        let c_last = c_last.min(IMWCHAR_MAX);
        unsafe {
            sys::ImFont_IsGlyphRangeUnused(
                self.raw(),
                c_begin as sys::ImWchar,
                c_last as sys::ImWchar,
            )
        }
    }
}

// NOTE: Do not mark Font as Send/Sync. It refers to memory owned by ImGui
// context and is not thread-safe to move/share across threads.
