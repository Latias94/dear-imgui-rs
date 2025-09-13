//! Font system for Dear ImGui
//!
//! This module provides font management functionality including font atlases,
//! individual fonts, glyph ranges, and font configuration.

pub mod atlas;
pub mod font;
pub mod glyph;
pub mod glyph_ranges;

pub use atlas::*;
pub use font::*;
pub use glyph::*;
pub use glyph_ranges::*;

use crate::Ui;

/// # Fonts
impl Ui {
    /// Returns the current font
    #[doc(alias = "GetFont")]
    pub fn current_font(&self) -> &Font {
        unsafe { Font::from_raw(crate::sys::ImGui_GetFont()) }
    }

    /// Returns the current font size (= height in pixels) with font scale applied
    #[doc(alias = "GetFontSize")]
    pub fn current_font_size(&self) -> f32 {
        unsafe { crate::sys::ImGui_GetFontSize() }
    }

    /// Push a font with dynamic size support (v1.92+ feature)
    ///
    /// This allows changing font size at runtime without pre-loading different sizes.
    /// Pass None for font to use the current font with the new size.
    pub fn push_font_with_size(&self, font: Option<&Font>, size: f32) {
        unsafe {
            let font_ptr = font.map_or(std::ptr::null_mut(), |f| f as *const Font as *mut crate::sys::ImFont);
            crate::sys::ImGui_PushFont(font_ptr, size);
        }
    }

    /// Execute a closure with a specific font and size (v1.92+ dynamic fonts)
    pub fn with_font_and_size<F, R>(&self, font: Option<&Font>, size: f32, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.push_font_with_size(font, size);
        let result = f();
        unsafe {
            crate::sys::ImGui_PopFont();
        }
        result
    }

    /// Returns the UV coordinate for a white pixel.
    ///
    /// Useful for drawing custom shapes with the draw list API.
    #[doc(alias = "GetFontTexUvWhitePixel")]
    pub fn font_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let uv_rr = crate::sys::ImGui_GetFontTexUvWhitePixel();
                let uv: crate::sys::ImVec2 = uv_rr.into();
                [uv.x, uv.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let uv = crate::sys::ImGui_GetFontTexUvWhitePixel();
                [uv.x, uv.y]
            }
        }
    }

    /// Sets the font scale of the current window
    ///
    /// Note: This function is not available in our Dear ImGui version.
    /// Font scaling should be handled through font size instead.
    #[doc(alias = "SetWindowFontScale")]
    pub fn set_window_font_scale(&self, _scale: f32) {
        // TODO: Implement when SetWindowFontScale is available in our Dear ImGui version
        // unsafe { crate::sys::ImGui_SetWindowFontScale(scale) }
    }
}
