//! Copied font glyph metrics
//!
//! Texture UVs are intentionally not exposed: loading another glyph can repack the atlas and
//! invalidate copied UV coordinates even within the same frame.

use crate::sys;

/// Copy-out metrics for a single font glyph.
///
/// The value remains valid after its baked-font view expires because it contains no native
/// pointers or atlas-relative texture coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    codepoint: u32,
    visible: bool,
    advance_x: f32,
    min: [f32; 2],
    max: [f32; 2],
}

impl Glyph {
    pub(crate) fn from_raw(raw: sys::ImFontGlyph) -> Self {
        Self {
            codepoint: raw.Codepoint(),
            visible: raw.Visible() != 0,
            advance_x: raw.AdvanceX,
            min: [raw.X0, raw.Y0],
            max: [raw.X1, raw.Y1],
        }
    }

    /// Get the Unicode codepoint for this glyph
    pub fn codepoint(&self) -> u32 {
        self.codepoint
    }

    /// Get the visibility flag for this glyph
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Get the advance X value for this glyph
    pub fn advance_x(&self) -> f32 {
        self.advance_x
    }

    /// Get the glyph position and size
    pub fn position_and_size(&self) -> ([f32; 2], [f32; 2]) {
        (self.min, self.max)
    }
}
