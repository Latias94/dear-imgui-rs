//! Frame-bound baked font data.

use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::fonts::{FontId, Glyph, validate_font_id_for_current_context};
use crate::{Ui, sys};

fn validate_positive_finite(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn validate_font_size(caller: &str, size: f32) {
    validate_positive_finite(caller, "size", size);
    assert!(
        size <= 512.0,
        "{caller} size must not exceed Dear ImGui's 512px font-size limit"
    );
}

fn wchar(c: char) -> Option<sys::ImWchar> {
    let codepoint = c as u32;
    if std::mem::size_of::<sys::ImWchar>() == 2 && codepoint > u16::MAX as u32 {
        None
    } else {
        Some(codepoint as sys::ImWchar)
    }
}

/// Runtime font data baked for one size and rasterizer density in the current frame.
///
/// Dear ImGui may compact and move baked-font storage at the next frame boundary. This view is
/// therefore tied to the [`Ui`] borrow that created it and cannot be retained across rendering or
/// a subsequent frame. Glyph queries return owned [`Glyph`] metric copies because lazy loading may
/// reallocate the native glyph vector; atlas-relative UVs are deliberately omitted because a
/// later glyph load may repack them within the same frame.
#[derive(Debug)]
pub struct BakedFont<'ui> {
    ui: &'ui Ui,
    font: FontId,
    size: f32,
    rasterizer_density: f32,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl<'ui> BakedFont<'ui> {
    unsafe fn from_raw(ui: &'ui Ui, font: FontId, raw: *mut sys::ImFontBaked) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        Some(Self {
            ui,
            font,
            size: unsafe { (*raw).Size },
            rasterizer_density: unsafe { (*raw).RasterizerDensity },
            _not_send_sync: PhantomData,
        })
    }

    fn raw(&self) -> *mut sys::ImFontBaked {
        self.ui.run_with_bound_context(|| {
            let font = validate_font_id_for_current_context(self.font, "BakedFont");
            let raw = unsafe { sys::ImFont_GetFontBaked(font, self.size, self.rasterizer_density) };
            assert!(
                !raw.is_null(),
                "BakedFont could not resolve its validated font, size, and rasterizer density"
            );
            raw
        })
    }

    /// Baked character height in logical pixels.
    pub fn size(&self) -> f32 {
        self.size
    }

    /// Rasterizer density used for this baked data.
    pub fn rasterizer_density(&self) -> f32 {
        self.rasterizer_density
    }

    /// Font ascent for this baked size.
    pub fn ascent(&self) -> f32 {
        unsafe { (*self.raw()).Ascent }
    }

    /// Font descent for this baked size.
    pub fn descent(&self) -> f32 {
        unsafe { (*self.raw()).Descent }
    }

    /// Approximate texture surface occupied by the loaded glyphs.
    pub fn metrics_total_surface(&self) -> u32 {
        unsafe { (*self.raw()).MetricsTotalSurface() }
    }

    /// Persistent ID of the font that owns this frame-local baked data.
    pub fn font_id(&self) -> FontId {
        self.font
    }

    /// Returns whether a glyph is already loaded without requesting a new glyph.
    #[doc(alias = "IsGlyphLoaded")]
    pub fn is_glyph_loaded(&self, c: char) -> bool {
        let Some(c) = wchar(c) else {
            return false;
        };
        unsafe { sys::ImFontBaked_IsGlyphLoaded(self.raw(), c) }
    }

    /// Find a glyph, falling back to the font's replacement glyph when necessary.
    ///
    /// This may lazily load glyph data and update the managed atlas texture.
    #[doc(alias = "FindGlyph")]
    pub fn glyph_or_fallback(&mut self, c: char) -> Option<Glyph> {
        let c = wchar(c)?;
        let glyph = unsafe { sys::ImFontBaked_FindGlyph(self.raw(), c) };
        NonNull::new(glyph).map(|glyph| Glyph::from_raw(unsafe { *glyph.as_ptr() }))
    }

    /// Find a glyph without using the replacement glyph.
    ///
    /// This may lazily load glyph data and update the managed atlas texture.
    #[doc(alias = "FindGlyphNoFallback")]
    pub fn glyph(&mut self, c: char) -> Option<Glyph> {
        let c = wchar(c)?;
        let glyph = unsafe { sys::ImFontBaked_FindGlyphNoFallback(self.raw(), c) };
        NonNull::new(glyph).map(|glyph| Glyph::from_raw(unsafe { *glyph.as_ptr() }))
    }

    /// Return the horizontal advance for a character.
    ///
    /// This may lazily load glyph metrics.
    #[doc(alias = "GetCharAdvance")]
    pub fn char_advance(&mut self, c: char) -> Option<f32> {
        let c = wchar(c)?;
        Some(unsafe { sys::ImFontBaked_GetCharAdvance(self.raw(), c) })
    }
}

impl Ui {
    /// Return baked data for the currently bound font, size, and rasterizer density.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::Context;
    /// let baked = {
    ///     let mut ctx = Context::create();
    ///     let ui = ctx.frame();
    ///     ui.current_baked_font()
    /// };
    /// baked.size();
    /// ```
    #[doc(alias = "GetFontBaked")]
    pub fn current_baked_font(&self) -> BakedFont<'_> {
        self.run_with_bound_context(|| {
            let font = unsafe { FontId::from_font(sys::igGetFont(), "Ui::current_baked_font()") };
            let raw = unsafe { sys::igGetFontBaked() };
            unsafe { BakedFont::from_raw(self, font, raw) }
                .expect("Ui::current_baked_font() requires an open frame with a current font")
        })
    }

    /// Return baked data for a font at the requested size and its current rasterizer density.
    ///
    /// Returns `None` for a legacy renderer while the atlas is locked, because creating an
    /// arbitrary baked size during that frame is unsupported by Dear ImGui.
    #[doc(alias = "ImFont::GetFontBaked")]
    pub fn baked_font(&self, font: FontId, size: f32) -> Option<BakedFont<'_>> {
        validate_font_size("Ui::baked_font()", size);
        self.baked_font_impl(font, size, -1.0)
    }

    /// Return baked data for a font at an explicit size and rasterizer density.
    ///
    /// Returns `None` for a legacy renderer while the atlas is locked.
    #[doc(alias = "ImFont::GetFontBaked")]
    pub fn baked_font_with_density(
        &self,
        font: FontId,
        size: f32,
        density: f32,
    ) -> Option<BakedFont<'_>> {
        validate_font_size("Ui::baked_font_with_density()", size);
        validate_positive_finite("Ui::baked_font_with_density()", "density", density);
        self.baked_font_impl(font, size, density)
    }

    fn baked_font_impl(&self, font: FontId, size: f32, density: f32) -> Option<BakedFont<'_>> {
        self.run_with_bound_context(|| {
            let raw_font = validate_font_id_for_current_context(font, "Ui::baked_font()");
            let atlas = unsafe { (*raw_font).OwnerAtlas };
            if atlas.is_null() || unsafe { (*atlas).Locked } {
                return None;
            }
            let raw = unsafe { sys::ImFont_GetFontBaked(raw_font, size, density) };
            unsafe { BakedFont::from_raw(self, font, raw) }
        })
    }
}

#[cfg(test)]
mod tests {
    fn setup_context() -> (crate::Context, crate::FontId) {
        let mut ctx = crate::Context::create();
        let font = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font()]);
        let _ = ctx.font_atlas().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        (ctx, font)
    }

    #[test]
    fn current_baked_font_copies_glyphs_and_exposes_metrics() {
        let (mut ctx, font_id) = setup_context();
        let ui = ctx.frame();
        let mut baked = ui.current_baked_font();

        assert_eq!(baked.font_id(), font_id);
        assert!(baked.size() > 0.0);
        assert!(baked.rasterizer_density() > 0.0);
        assert!(baked.ascent() > baked.descent());
        assert!(baked.char_advance('A').is_some_and(|advance| advance > 0.0));
        let glyph = baked
            .glyph_or_fallback('A')
            .expect("the default font should contain A");
        assert_eq!(glyph.codepoint(), 'A' as u32);
        assert!(glyph.advance_x() > 0.0);

        let debug = format!("{glyph:?}");
        for unstable_field in ["PackId", "U0", "V0", "U1", "V1"] {
            assert!(
                !debug.contains(unstable_field),
                "Glyph::Debug exposed unstable atlas field {unstable_field}: {debug}"
            );
        }
    }

    #[test]
    fn arbitrary_baked_font_is_rejected_while_legacy_atlas_is_locked() {
        let (mut ctx, font_id) = setup_context();
        let ui = ctx.frame();

        assert!(ui.baked_font(font_id, 18.0).is_none());
        assert!(ui.current_baked_font().size() > 0.0);
    }

    #[test]
    fn managed_atlas_can_create_an_arbitrary_baked_size_in_frame() {
        let mut ctx = crate::Context::create();
        let _consumer = ctx
            .create_renderer_consumer()
            .expect("the managed renderer consumer should attach");
        let font_id = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font()]);
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.io_mut()
            .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);

        {
            let ui = ctx.frame();
            let baked = ui
                .baked_font_with_density(font_id, 18.0, 2.0)
                .expect("managed atlases should allow dynamic baked sizes");
            assert_eq!(baked.size(), 18.0);
            assert_eq!(baked.rasterizer_density(), 2.0);
        }
        let _ = ctx.render();
    }

    #[test]
    fn baked_font_validates_size_and_density_before_ffi() {
        let (mut ctx, font_id) = setup_context();
        let ui = ctx.frame();

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.baked_font(font_id, 0.0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.baked_font_with_density(font_id, 13.0, f32::NAN);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.baked_font(font_id, 513.0);
            }))
            .is_err()
        );
    }
}
