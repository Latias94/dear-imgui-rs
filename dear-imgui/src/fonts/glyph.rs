//! Font glyph data structures
//!
//! This module provides types for working with individual font glyphs
//! and their rendering data.

use crate::sys;

/// A single font glyph with its rendering data
///
/// This represents a single character glyph with its texture coordinates,
/// advance information, and other rendering data.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    /// The raw ImFontGlyph data
    pub(crate) raw: sys::ImFontGlyph,
}

impl Glyph {
    /// Creates a Glyph from raw ImFontGlyph data
    pub fn from_raw(raw: sys::ImFontGlyph) -> Self {
        Self { raw }
    }

    /// Get the Unicode codepoint for this glyph
    pub fn codepoint(&self) -> u32 {
        self.raw.Codepoint()
    }

    /// Get the visibility flag for this glyph
    pub fn visible(&self) -> bool {
        self.raw.Visible() != 0
    }

    /// Get the advance X value for this glyph
    pub fn advance_x(&self) -> f32 {
        self.raw.AdvanceX
    }

    /// Get the texture coordinates for this glyph
    pub fn tex_coords(&self) -> ([f32; 2], [f32; 2]) {
        ([self.raw.U0, self.raw.V0], [self.raw.U1, self.raw.V1])
    }

    /// Get the glyph position and size
    pub fn position_and_size(&self) -> ([f32; 2], [f32; 2]) {
        ([self.raw.X0, self.raw.Y0], [self.raw.X1, self.raw.Y1])
    }
}

impl From<sys::ImFontGlyph> for Glyph {
    fn from(raw: sys::ImFontGlyph) -> Self {
        Self::from_raw(raw)
    }
}
