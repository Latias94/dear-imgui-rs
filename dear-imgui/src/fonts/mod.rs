//! Font system for Dear ImGui
//!
//! This module provides font management functionality including font atlases,
//! individual fonts, and font configuration.
//!
//! Dear ImGui 1.92 loads glyphs on demand, so the deprecated `GlyphRanges` and
//! `GlyphRangesBuilder` compatibility helpers are intentionally unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::fonts::GlyphRangesBuilder;
//! ```

pub mod atlas;
mod baked;
pub mod font;
pub mod glyph;

pub use atlas::*;
pub use baked::*;
pub use glyph::*;

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
    /// Return the persistent, atlas-validated ID of the current font.
    #[doc(alias = "GetFont")]
    pub fn current_font(&self) -> FontId {
        self.run_with_bound_context(|| unsafe {
            FontId::from_font(crate::sys::igGetFont(), "Ui::current_font()")
        })
    }

    /// Returns the current font size (= height in pixels) with font scale applied
    #[doc(alias = "GetFontSize")]
    pub fn current_font_size(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { crate::sys::igGetFontSize() })
    }

    /// Push a font with dynamic size support (v1.92+ feature).
    ///
    /// This allows changing font size at runtime without pre-loading different sizes.
    /// Pass `None` to keep the current font. A size of `0.0` keeps the current
    /// size, so `push_font_with_size(Some(font), 0.0)` changes only the font.
    /// A non-zero size is the base size before Dear ImGui applies global and DPI
    /// font scaling; [`Ui::current_font_size`] already includes those scales.
    ///
    /// Returns a `FontStackToken` that pops the font stack when dropped or when
    /// [`crate::FontStackToken::pop`] is called.
    #[doc(alias = "PushFont")]
    pub fn push_font_with_size(
        &self,
        font: Option<FontId>,
        size: f32,
    ) -> crate::FontStackToken<'_> {
        assert_non_negative_finite_f32("Ui::push_font_with_size()", "size", size);
        self.run_with_bound_context(|| unsafe {
            let font_ptr = font.map_or(std::ptr::null_mut(), |id| {
                crate::fonts::validate_font_id_for_current_context(id, "Ui::push_font_with_size()")
            });
            crate::sys::igPushFont(font_ptr, size);
        });
        crate::FontStackToken::new(self)
    }

    /// Execute a closure with a specific font and size (v1.92+ dynamic fonts)
    pub fn with_font_and_size<F, R>(&self, font: Option<FontId>, size: f32, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let token = self.push_font_with_size(font, size);
        let result = f();
        drop(token);
        result
    }

    /// Returns the UV coordinate for a white pixel.
    ///
    /// Useful for drawing custom shapes with the draw list API.
    #[doc(alias = "GetFontTexUvWhitePixel")]
    pub fn font_tex_uv_white_pixel(&self) -> [f32; 2] {
        self.run_with_bound_context(|| unsafe {
            let uv = crate::sys::igGetFontTexUvWhitePixel();
            [uv.x, uv.y]
        })
    }

    /// Sets the legacy per-window font scale of the current window.
    ///
    /// Prefer [`Ui::push_font_with_size`] or `style.FontScaleMain` for new code.
    #[doc(alias = "SetWindowFontScale")]
    pub fn set_window_font_scale(&self, scale: f32) {
        assert_positive_finite_f32("Ui::set_window_font_scale()", "scale", scale);

        self.run_with_bound_context(|| unsafe {
            let window = crate::sys::igGetCurrentWindow();
            if window.is_null() {
                return;
            }
            (*window).FontWindowScale = scale;
            crate::sys::igUpdateCurrentFontSize(0.0);
        });
    }
}

#[cfg(test)]
mod tests {
    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
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
        let _ = ctx.render_legacy();

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

        let _ = ctx.render_legacy();
    }

    #[test]
    fn push_font_with_size_distinguishes_preserved_and_overridden_sizes() {
        let mut ctx = crate::Context::create();
        let small = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(13.0)]);
        let large = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(29.0)]);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);

        let ui = ctx.frame();
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);

        {
            let _font = ui.push_font_with_size(Some(large), 0.0);
            assert_eq!(ui.current_font(), large);
            assert_eq!(ui.current_font_size(), 13.0);
        }
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);

        {
            let _font = ui.push_font_with_size(Some(large), 37.0);
            assert_eq!(ui.current_font(), large);
            assert_eq!(ui.current_font_size(), 37.0);
        }
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);
    }
}
