#![allow(deprecated)]
//! Glyph ranges for font loading
//!
//! **⚠️ DEPRECATED with Dear ImGui 1.92+**: With Dear ImGui 1.92+, glyph ranges are no longer needed
//! for most use cases. The new dynamic font system loads glyphs on-demand automatically.
//!
//! This module is kept for backward compatibility and special use cases where you need
//! to explicitly control which glyphs are loaded (e.g., for memory-constrained environments).
//!
//! **Recommended approach with Dear ImGui 1.92+**: Use `FontSource` without specifying glyph ranges.
//! The system will automatically load any Unicode character as needed.
//!
//! This module provides utilities for specifying which character ranges
//! should be loaded when adding fonts to the atlas.

use crate::sys;

/// Builder for creating custom glyph ranges
///
/// **⚠️ DEPRECATED with Dear ImGui 1.92+**: Consider using the new dynamic font system instead.
///
/// This allows you to specify exactly which characters should be loaded
/// from a font, which can save memory and improve performance.
///
/// **Note**: With Dear ImGui 1.92+ dynamic fonts, this is only needed for special cases
/// where you want to explicitly control memory usage or exclude certain ranges.
#[derive(Debug)]
#[deprecated(
    since = "0.1.0",
    note = "Use dynamic font loading instead. Glyphs are now loaded on-demand automatically with Dear ImGui 1.92+."
)]
pub struct GlyphRangesBuilder {
    raw: *mut sys::ImFontGlyphRangesBuilder,
}

impl GlyphRangesBuilder {
    /// Creates a new glyph ranges builder
    pub fn new() -> Self {
        unsafe {
            let raw = sys::ImFontGlyphRangesBuilder_ImFontGlyphRangesBuilder();
            if raw.is_null() {
                panic!("ImFontGlyphRangesBuilder_ImFontGlyphRangesBuilder() returned null");
            }
            Self { raw }
        }
    }

    /// Add text to the builder (all characters in the text will be included)
    #[doc(alias = "AddText")]
    pub fn add_text(&mut self, text: &str) {
        unsafe {
            let text_start = text.as_ptr() as *const std::os::raw::c_char;
            let text_end = text_start.add(text.len());
            sys::ImFontGlyphRangesBuilder_AddText(self.raw, text_start, text_end);
        }
    }

    /// Add a range of characters
    #[doc(alias = "AddRanges")]
    pub fn add_ranges(&mut self, ranges: &[u32]) {
        let mut tmp: Vec<sys::ImWchar> = Vec::with_capacity(ranges.len());
        for &v in ranges {
            assert!(
                v <= u32::from(u16::MAX),
                "glyph range value {v:#X} exceeded ImWchar16 max (0xFFFF)"
            );
            tmp.push(v as sys::ImWchar);
        }
        unsafe { sys::ImFontGlyphRangesBuilder_AddRanges(self.raw, tmp.as_ptr()) };
    }

    /// Build the final ranges array
    #[doc(alias = "BuildRanges")]
    pub fn build_ranges(&mut self) -> Vec<u32> {
        unsafe {
            let mut out_ranges = std::mem::MaybeUninit::<sys::ImVector_ImWchar>::uninit();
            sys::ImVector_ImWchar_Init(out_ranges.as_mut_ptr());
            let mut out_ranges = out_ranges.assume_init();
            sys::ImFontGlyphRangesBuilder_BuildRanges(self.raw, &mut out_ranges);

            // Convert ImVector to Vec
            let len = out_ranges.Size as usize;
            let mut result: Vec<u32> = Vec::with_capacity(len);
            if len > 0 && !out_ranges.Data.is_null() {
                for i in 0..len {
                    result.push(*out_ranges.Data.add(i) as u32);
                }
            }
            sys::ImVector_ImWchar_UnInit(&mut out_ranges);
            result
        }
    }
}

impl Default for GlyphRangesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GlyphRangesBuilder {
    fn drop(&mut self) {
        unsafe {
            if !self.raw.is_null() {
                sys::ImFontGlyphRangesBuilder_destroy(self.raw);
                self.raw = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "exceeded ImWchar16 max")]
    fn add_ranges_rejects_non_bmp_codepoints() {
        let mut b = GlyphRangesBuilder::new();
        b.add_ranges(&[0x1_0000, 0]);
    }
}

/// Predefined glyph ranges for common character sets
///
/// **Note**: These ranges are still useful with Dear ImGui 1.92+ for:
/// - Memory-constrained environments where you want to limit loaded glyphs
/// - Font merging with `glyph_exclude_ranges()` to prevent conflicts
/// - Explicit control over which characters are available
///
/// For most use cases, you can now omit glyph ranges and let the dynamic
/// font system load glyphs on-demand.
pub struct GlyphRanges;

impl GlyphRanges {
    /// Basic Latin + Latin Supplement (default)
    pub const DEFAULT: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0,
    ];

    /// Korean characters
    pub const KOREAN: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x3131, 0x3163, // Korean alphabets
        0xAC00, 0xD7A3, // Korean characters
        0,
    ];

    /// Japanese Hiragana + Katakana + Half-Width characters
    pub const JAPANESE: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x3000, 0x30FF, // CJK Symbols and Punctuations, Hiragana, Katakana
        0x31F0, 0x31FF, // Katakana Phonetic Extensions
        0xFF00, 0xFFEF, // Half-width characters
        0,
    ];

    /// Chinese Simplified common characters
    pub const CHINESE_SIMPLIFIED_COMMON: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x2000, 0x206F, // General Punctuation
        0x3000, 0x30FF, // CJK Symbols and Punctuations, Hiragana, Katakana
        0x31F0, 0x31FF, // Katakana Phonetic Extensions
        0xFF00, 0xFFEF, // Half-width characters
        0x4E00, 0x9FAF, // CJK Ideograms
        0,
    ];

    /// Chinese Traditional common characters
    pub const CHINESE_TRADITIONAL_COMMON: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x2000, 0x206F, // General Punctuation
        0x3000, 0x30FF, // CJK Symbols and Punctuations, Hiragana, Katakana
        0x31F0, 0x31FF, // Katakana Phonetic Extensions
        0xFF00, 0xFFEF, // Half-width characters
        0x4E00, 0x9FAF, // CJK Ideograms
        0,
    ];

    /// Cyrillic characters
    pub const CYRILLIC: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x0400, 0x052F, // Cyrillic + Cyrillic Supplement
        0x2DE0, 0x2DFF, // Cyrillic Extended-A
        0xA640, 0xA69F, // Cyrillic Extended-B
        0,
    ];

    /// Thai characters
    pub const THAI: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x0E00, 0x0E7F, // Thai
        0,
    ];

    /// Vietnamese characters
    pub const VIETNAMESE: &'static [u32] = &[
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x0102, 0x0103, // Ă ă
        0x0110, 0x0111, // Đ đ
        0x0128, 0x0129, // Ĩ ĩ
        0x0168, 0x0169, // Ũ ũ
        0x01A0, 0x01A1, // Ơ ơ
        0x01AF, 0x01B0, // Ư ư
        0x1EA0, 0x1EF9, // Vietnamese Extended
        0,
    ];
}
