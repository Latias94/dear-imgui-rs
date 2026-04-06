//! Font system for Dear ImGui
//!
//! This module provides font management functionality including font atlases,
//! individual fonts, glyph ranges, and font configuration.

pub mod atlas;
pub mod font;
pub mod glyph;
/// Deprecated glyph ranges helpers.
///
/// With Dear ImGui 1.92+, fonts are dynamically sized and glyphs are loaded on demand.
/// In most cases you no longer need to specify glyph ranges. Keep using this module
/// only for legacy code or very constrained environments where you explicitly want to
/// limit the character set.
#[deprecated(
    since = "0.2.0",
    note = "ImGui 1.92+ recommends dynamic fonts with on-demand glyph loading; glyph ranges are kept for legacy compatibility"
)]
pub mod glyph_ranges;

pub use atlas::*;
pub use font::*;
pub use glyph::*;
#[allow(deprecated)]
pub use glyph_ranges::*;

use crate::Ui;

/// # Fonts
impl Ui {
    /// Returns the current font
    #[doc(alias = "GetFont")]
    pub fn current_font(&self) -> &Font {
        unsafe { Font::from_raw(crate::sys::igGetFont() as *const _) }
    }

    /// Returns the current font size (= height in pixels) with font scale applied
    #[doc(alias = "GetFontSize")]
    pub fn current_font_size(&self) -> f32 {
        unsafe { crate::sys::igGetFontSize() }
    }

    /// Push a font with dynamic size support (v1.92+ feature)
    ///
    /// This allows changing font size at runtime without pre-loading different sizes.
    /// Pass None for font to use the current font with the new size.
    pub fn push_font_with_size(&self, font: Option<&Font>, size: f32) {
        unsafe {
            let font_ptr = font.map_or(std::ptr::null_mut(), |f| f.raw());
            crate::sys::igPushFont(font_ptr, size);
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
            crate::sys::igPopFont();
        }
        result
    }

    /// Returns the UV coordinate for a white pixel.
    ///
    /// Useful for drawing custom shapes with the draw list API.
    #[doc(alias = "GetFontTexUvWhitePixel")]
    pub fn font_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            let uv = crate::sys::igGetFontTexUvWhitePixel();
            [uv.x, uv.y]
        }
    }

    /// Sets the legacy per-window font scale of the current window.
    ///
    /// Prefer [`Ui::push_font_with_size`] or `style.FontScaleMain` for new code.
    #[doc(alias = "SetWindowFontScale")]
    pub fn set_window_font_scale(&self, scale: f32) {
        assert!(scale > 0.0, "window font scale must be positive");

        unsafe {
            let window = crate::sys::igGetCurrentWindow();
            if window.is_null() {
                return;
            }
            (*window).FontWindowScale = scale;
            crate::sys::igUpdateCurrentFontSize(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn set_window_font_scale_updates_current_window_state() {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas_mut().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let ui = ctx.frame();

        ui.window("font_scale_test").build(|| {
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            assert!(!window.is_null());
            assert_eq!(unsafe { (*window).FontWindowScale }, 1.0);

            ui.set_window_font_scale(1.5);

            assert_eq!(unsafe { (*window).FontWindowScale }, 1.5);
        });
    }
}
