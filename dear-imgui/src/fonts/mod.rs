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

fn assert_non_negative_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_positive_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
    assert!(value > 0.0, "{caller} {name} must be positive");
}

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

    /// Push a font with dynamic size support (v1.92+ feature).
    ///
    /// This allows changing font size at runtime without pre-loading different sizes.
    /// Pass None for font to use the current font with the new size.
    ///
    /// Returns a `FontStackToken` that pops the font stack when dropped or when
    /// [`FontStackToken::pop`] is called.
    #[doc(alias = "PushFont")]
    pub fn push_font_with_size(&self, font: Option<&Font>, size: f32) -> crate::FontStackToken<'_> {
        assert_non_negative_finite_f32("Ui::push_font_with_size()", "size", size);
        unsafe {
            let font_ptr = font.map_or(std::ptr::null_mut(), |f| {
                crate::fonts::validate_font_for_current_context(f, "Ui::push_font_with_size()")
            });
            crate::sys::igPushFont(font_ptr, size);
        }
        crate::FontStackToken::new(self)
    }

    /// Execute a closure with a specific font and size (v1.92+ dynamic fonts)
    pub fn with_font_and_size<F, R>(&self, font: Option<&Font>, size: f32, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _token = self.push_font_with_size(font, size);
        f()
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
        assert_positive_finite_f32("Ui::set_window_font_scale()", "scale", scale);

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
    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas_mut().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx
    }

    #[test]
    fn set_window_font_scale_updates_current_window_state() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("font_scale_test").build(|| {
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            assert!(!window.is_null());
            assert_eq!(unsafe { (*window).FontWindowScale }, 1.0);

            ui.set_window_font_scale(1.5);

            assert_eq!(unsafe { (*window).FontWindowScale }, 1.5);
        });
    }

    #[test]
    fn font_runtime_size_setters_validate_before_ffi() {
        let mut ctx = setup_context();
        {
            let ui = ctx.frame();

            ui.window("font_size_token").build(|| {
                let _font = ui.push_font_with_size(None, 18.0);
                ui.text("font token is scoped");
            });

            ui.with_font_and_size(None, 0.0, || {
                ui.text("closure helper is scoped");
            });
        }
        let _ = ctx.render();

        let ui = ctx.frame();

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.push_font_with_size(None, -1.0);
            }))
            .is_err()
        );

        ui.window("font_scale_invalid").build(|| {
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            assert_eq!(unsafe { (*window).FontWindowScale }, 1.0);

            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_window_font_scale(f32::INFINITY);
                }))
                .is_err()
            );
            assert_eq!(unsafe { (*window).FontWindowScale }, 1.0);
        });
    }

    #[test]
    fn with_font_and_size_pops_after_panic() {
        let mut ctx = setup_context();
        {
            let ui = ctx.frame();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.with_font_and_size(None, 18.0, || {
                    panic!("forced panic while font is pushed");
                });
            }));

            assert!(result.is_err());
            ui.text("frame remains balanced after panic");
        }

        let _ = ctx.render();
    }
}
