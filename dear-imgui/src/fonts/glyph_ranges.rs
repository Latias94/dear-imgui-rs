//! Glyph ranges for font loading
//!
//! This module provides utilities for specifying which character ranges
//! should be loaded when adding fonts to the atlas.

use crate::sys;

/// Builder for creating custom glyph ranges
///
/// This allows you to specify exactly which characters should be loaded
/// from a font, which can save memory and improve performance.
#[derive(Debug)]
pub struct GlyphRangesBuilder {
    raw: sys::ImFontGlyphRangesBuilder,
}

impl GlyphRangesBuilder {
    /// Creates a new glyph ranges builder
    pub fn new() -> Self {
        Self {
            raw: sys::ImFontGlyphRangesBuilder::default(),
        }
    }

    /// Add text to the builder (all characters in the text will be included)
    #[doc(alias = "AddText")]
    pub fn add_text(&mut self, text: &str) {
        unsafe {
            let text_start = text.as_ptr() as *const std::os::raw::c_char;
            let text_end = text_start.add(text.len());
            self.raw.AddText(text_start, text_end);
        }
    }

    /// Add a range of characters
    #[doc(alias = "AddRanges")]
    pub fn add_ranges(&mut self, ranges: &[u32]) {
        unsafe {
            self.raw.AddRanges(ranges.as_ptr());
        }
    }

    /// Build the final ranges array
    #[doc(alias = "BuildRanges")]
    pub fn build_ranges(&mut self) -> Vec<u32> {
        unsafe {
            let mut out_ranges = sys::ImVector::<u32>::default();
            self.raw.BuildRanges(&mut out_ranges);

            // Convert ImVector to Vec
            let len = out_ranges.Size as usize;
            let mut result = Vec::with_capacity(len);
            for i in 0..len {
                result.push(*out_ranges.Data.add(i));
            }
            result
        }
    }
}

impl Default for GlyphRangesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Predefined glyph ranges for common character sets
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
