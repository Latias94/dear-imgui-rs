use std::cell::UnsafeCell;
use std::ffi::c_char;
use std::ptr;

use crate::FontError;
use crate::context::binding::ContextThreadLease;
use crate::sys;
use crate::validation::{assert_finite_f32, assert_finite_vec2};

use super::FontAtlas;

/// Font configuration for loading fonts with v1.92+ features
#[derive(Debug)]
pub struct FontConfig {
    pub(super) raw: sys::ImFontConfig,
    glyph_exclude_ranges: Option<Vec<sys::ImWchar>>,
}

impl Clone for FontConfig {
    fn clone(&self) -> Self {
        let mut raw = self.raw;
        let glyph_exclude_ranges = self.glyph_exclude_ranges.clone();
        if let Some(ref ranges) = glyph_exclude_ranges {
            raw.GlyphExcludeRanges = ranges.as_ptr();
        }
        Self {
            raw,
            glyph_exclude_ranges,
        }
    }
}

impl FontConfig {
    /// Creates a font configuration with Dear ImGui's defaults on any thread.
    pub fn new() -> Self {
        // ImFontConfig::ImFontConfig zeroes the struct and sets these five fields.
        // The cimgui heap constructor would also touch the current Context's allocator state.
        Self {
            raw: sys::ImFontConfig {
                FontDataOwnedByAtlas: true,
                ExtraSizeScale: 1.0,
                GlyphMaxAdvanceX: f32::MAX,
                RasterizerMultiply: 1.0,
                RasterizerDensity: 1.0,
                ..Default::default()
            },
            glyph_exclude_ranges: None,
        }
    }

    pub(super) fn owned_glyph_exclude_ranges(&self) -> Option<&[sys::ImWchar]> {
        self.glyph_exclude_ranges.as_deref()
    }

    fn has_reference_size_dependent_metrics(&self) -> bool {
        self.raw.GlyphOffset.x != 0.0
            || self.raw.GlyphOffset.y != 0.0
            || self.raw.GlyphMinAdvanceX != 0.0
            || self.raw.GlyphMaxAdvanceX != f32::MAX
    }

    fn validate_common(&self, caller: &str) {
        validate_font_size_pixels(caller, "SizePixels", self.raw.SizePixels);
        assert_finite_vec2(
            caller,
            "GlyphOffset",
            [self.raw.GlyphOffset.x, self.raw.GlyphOffset.y],
        );
        assert_non_negative_f32(caller, "GlyphMinAdvanceX", self.raw.GlyphMinAdvanceX);
        assert_non_negative_f32(caller, "GlyphMaxAdvanceX", self.raw.GlyphMaxAdvanceX);
        assert!(
            self.raw.GlyphMinAdvanceX <= self.raw.GlyphMaxAdvanceX,
            "{caller} GlyphMinAdvanceX must be less than or equal to GlyphMaxAdvanceX"
        );
        assert_finite_f32(caller, "GlyphExtraAdvanceX", self.raw.GlyphExtraAdvanceX);
        assert_non_negative_f32(caller, "RasterizerMultiply", self.raw.RasterizerMultiply);
        assert!(
            self.raw.RasterizerMultiply <= RASTERIZER_MULTIPLY_MAX,
            "{caller} RasterizerMultiply must be less than or equal to {RASTERIZER_MULTIPLY_MAX}"
        );
        assert_positive_f32(caller, "RasterizerDensity", self.raw.RasterizerDensity);
        assert_non_negative_i8(caller, "OversampleH", self.raw.OversampleH);
        assert_non_negative_i8(caller, "OversampleV", self.raw.OversampleV);
    }

    pub(super) fn validate_for_add_font_default(&self, caller: &str) {
        self.validate_common(caller);
    }

    pub(super) fn validate_for_add_font_with_size(&self, caller: &str, size_pixels: f32) {
        self.validate_common(caller);
        let effective_size_pixels = if size_pixels > 0.0 {
            size_pixels
        } else {
            self.raw.SizePixels
        };
        assert_reference_font_size_for_metrics(
            caller,
            effective_size_pixels,
            self.has_reference_size_dependent_metrics(),
        );
    }

    /// Set the font reference size in pixels.
    ///
    /// With v1.92+ dynamic fonts, `0.0` leaves the reference size implicit;
    /// the font can still be selected at any positive runtime size.
    pub fn size_pixels(mut self, size: f32) -> Self {
        validate_font_size_pixels("FontConfig::size_pixels()", "size", size);
        self.raw.SizePixels = size;
        self
    }

    /// Set whether to merge this font with the previous one
    pub fn merge_mode(mut self, merge: bool) -> Self {
        self.raw.MergeMode = merge;
        self
    }

    /// Set font loader flags for this specific font
    ///
    /// These flags override the global atlas flags for this font.
    pub fn font_loader_flags(mut self, flags: FontLoaderFlags) -> Self {
        self.raw.FontLoaderFlags = flags.0;
        self
    }

    /// Set inclusive glyph ranges to exclude from this font.
    ///
    /// The input is a slice of `(start, end)` pairs. It is converted to Dear ImGui's
    /// `[start, end, ..., 0]` format.
    pub fn glyph_exclude_ranges(mut self, ranges: &[(u32, u32)]) -> Self {
        if ranges.is_empty() {
            self.raw.GlyphExcludeRanges = ptr::null();
            self.glyph_exclude_ranges = None;
            return self;
        }

        assert!(
            ranges.len() <= 32,
            "FontConfig::glyph_exclude_ranges() accepts at most 32 ranges"
        );
        let converted = encode_glyph_ranges("FontConfig::glyph_exclude_ranges()", ranges);

        self.raw.GlyphExcludeRanges = converted.as_ptr();
        self.glyph_exclude_ranges = Some(converted);
        self
    }

    /// Set a custom font loader for this font.
    ///
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*` in the
    /// atlas font source. A [`StbTrueTypeFontData`](crate::StbTrueTypeFontData) source rejects
    /// any loader other than the built-in stb_truetype loader covered by its validation proof.
    pub fn font_loader(mut self, loader: &'static FontLoader) -> Self {
        self.raw.FontLoader = loader.as_ptr();
        self
    }

    /// Set the font name for debugging
    pub fn name(mut self, name: &str) -> Self {
        let name_bytes = name.as_bytes();
        let copy_len = std::cmp::min(name_bytes.len(), self.raw.Name.len() - 1);

        // Clear the array first
        for i in 0..self.raw.Name.len() {
            self.raw.Name[i] = 0;
        }

        // Copy the name
        for (i, &byte) in name_bytes.iter().take(copy_len).enumerate() {
            self.raw.Name[i] = byte as c_char;
        }

        self
    }

    /// Set glyph offset for this font
    pub fn glyph_offset(mut self, offset: [f32; 2]) -> Self {
        assert_finite_vec2("FontConfig::glyph_offset()", "offset", offset);
        self.raw.GlyphOffset.x = offset[0];
        self.raw.GlyphOffset.y = offset[1];
        self
    }

    /// Set minimum advance X for glyphs
    pub fn glyph_min_advance_x(mut self, advance: f32) -> Self {
        assert_non_negative_f32("FontConfig::glyph_min_advance_x()", "advance", advance);
        assert!(
            advance <= self.raw.GlyphMaxAdvanceX,
            "FontConfig::glyph_min_advance_x() advance must be less than or equal to current glyph_max_advance_x"
        );
        self.raw.GlyphMinAdvanceX = advance;
        self
    }

    /// Set maximum advance X for glyphs
    pub fn glyph_max_advance_x(mut self, advance: f32) -> Self {
        assert_non_negative_f32("FontConfig::glyph_max_advance_x()", "advance", advance);
        assert!(
            advance >= self.raw.GlyphMinAdvanceX,
            "FontConfig::glyph_max_advance_x() advance must be greater than or equal to current glyph_min_advance_x"
        );
        self.raw.GlyphMaxAdvanceX = advance;
        self
    }

    /// Set extra advance X for glyphs (spacing between characters)
    pub fn glyph_extra_advance_x(mut self, advance: f32) -> Self {
        assert_finite_f32("FontConfig::glyph_extra_advance_x()", "advance", advance);
        self.raw.GlyphExtraAdvanceX = advance;
        self
    }

    /// Set rasterizer multiply factor
    pub fn rasterizer_multiply(mut self, multiply: f32) -> Self {
        assert_non_negative_f32("FontConfig::rasterizer_multiply()", "multiply", multiply);
        assert!(
            multiply <= RASTERIZER_MULTIPLY_MAX,
            "FontConfig::rasterizer_multiply() multiply must be less than or equal to {RASTERIZER_MULTIPLY_MAX}"
        );
        self.raw.RasterizerMultiply = multiply;
        self
    }

    /// Set rasterizer density for DPI scaling
    pub fn rasterizer_density(mut self, density: f32) -> Self {
        assert_positive_f32("FontConfig::rasterizer_density()", "density", density);
        self.raw.RasterizerDensity = density;
        self
    }

    /// Set pixel snap horizontally
    pub fn pixel_snap_h(mut self, snap: bool) -> Self {
        self.raw.PixelSnapH = snap;
        self
    }

    /// Set horizontal oversampling
    pub fn oversample_h(mut self, oversample: i8) -> Self {
        assert_non_negative_i8("FontConfig::oversample_h()", "oversample", oversample);
        self.raw.OversampleH = oversample;
        self
    }

    /// Set vertical oversampling
    pub fn oversample_v(mut self, oversample: i8) -> Self {
        assert_non_negative_i8("FontConfig::oversample_v()", "oversample", oversample);
        self.raw.OversampleV = oversample;
        self
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Borrowed handle to a complete Dear ImGui font-loader callback table.
///
/// Safe code can obtain the built-in stb_truetype loader. Custom loaders must
/// be created in native code and enter through [`FontLoader::from_raw`], whose
/// safety contract covers every callback and stored pointer.
#[repr(transparent)]
pub struct FontLoader(UnsafeCell<sys::ImFontLoader>);

impl FontLoader {
    /// Return the built-in stb_truetype loader, panicking if [`Self::try_stb_truetype`] fails.
    #[doc(alias = "ImFontAtlasGetFontLoaderForStbTruetype")]
    pub fn stb_truetype() -> &'static Self {
        Self::try_stb_truetype().expect("Failed to get Dear ImGui's stb_truetype font loader")
    }

    /// Return the built-in loader on the Dear ImGui owner thread, or any thread while unowned.
    ///
    /// Returns [`FontError::Context`] if the thread lease cannot be acquired.
    pub fn try_stb_truetype() -> Result<&'static Self, FontError> {
        // The upstream getter writes its static callback table on every call, including during
        // native atlas builds. Existing handles view that storage through UnsafeCell.
        let _lease = ContextThreadLease::acquire()?;
        let raw = unsafe { sys::igImFontAtlasGetFontLoaderForStbTruetype() };
        unsafe { raw.cast::<Self>().as_ref() }.ok_or(FontError::BuiltInLoaderUnavailable)
    }

    /// Borrow an externally owned font loader.
    ///
    /// # Safety
    ///
    /// `raw` must point to a fully initialized `ImFontLoader`. Its name and all
    /// callback pointers, callback userdata, and referenced native state must
    /// remain valid for `'a`. Every callback must obey Dear ImGui's ABI, must not
    /// unwind across FFI, and must satisfy the callback-specific ownership rules.
    pub unsafe fn from_raw<'a>(raw: *const sys::ImFontLoader) -> &'a Self {
        assert!(
            !raw.is_null(),
            "FontLoader::from_raw() received a null pointer"
        );
        unsafe { &*raw.cast::<Self>() }
    }

    /// Return the borrowed raw loader pointer.
    pub(crate) fn as_ptr(&self) -> *const sys::ImFontLoader {
        self.0.get().cast_const()
    }
}

/// Font loader flags for controlling font loading behavior.
///
/// These bits mirror Dear ImGui's `ImGuiFreeTypeLoaderFlags` (see
/// `misc/freetype/imgui_freetype.h`) and are only interpreted by the
/// FreeType font backend. When using the stb_truetype backend, they
/// are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLoaderFlags(pub u32);

impl FontLoaderFlags {
    /// No special flags
    pub const NONE: Self = Self(0);

    /// Disable hinting (more faithful to the original glyph shapes, but blurrier)
    pub const NO_HINTING: Self = Self(1 << 0);

    /// Disable auto-hinter (prefer the font's native hinter only)
    pub const NO_AUTOHINT: Self = Self(1 << 1);

    /// Prefer auto-hinter over the font's native hinter
    pub const FORCE_AUTOHINT: Self = Self(1 << 2);

    /// Light hinting (often closer to Windows ClearType appearance)
    pub const LIGHT_HINTING: Self = Self(1 << 3);

    /// Strong/mono hinting (intended for monochrome outputs)
    pub const MONO_HINTING: Self = Self(1 << 4);

    /// Artificially embolden the font
    pub const BOLD: Self = Self(1 << 5);

    /// Artificially slant the font (oblique)
    pub const OBLIQUE: Self = Self(1 << 6);

    /// Disable anti-aliasing (combine with `MONO_HINTING` for best results)
    pub const MONOCHROME: Self = Self(1 << 7);

    /// Enable color-layered glyphs (e.g. color emoji)
    pub const LOAD_COLOR: Self = Self(1 << 8);

    /// Enable FreeType bitmap glyphs
    pub const BITMAP: Self = Self(1 << 9);
}

impl std::ops::BitOr for FontLoaderFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for FontLoaderFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl FontAtlas {
    /// Sets the font loader for this atlas.
    ///
    /// Use [`FontLoader::stb_truetype`] to select stb_truetype even in a FreeType build.
    /// Returns an error if any font source has already been added, before native state changes.
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*`.
    pub fn set_font_loader(&self, loader: &'static FontLoader) -> Result<(), FontError> {
        self.assert_mutation_allowed("FontAtlas::set_font_loader()");
        let source_count = unsafe { (*self.raw()).Sources.Size };
        assert!(
            source_count >= 0,
            "FontAtlas::set_font_loader() observed a negative native source count"
        );
        if source_count != 0 {
            return Err(FontError::SourcesAlreadyAdded {
                source_count: source_count as usize,
            });
        }
        unsafe {
            sys::ImFontAtlas_SetFontLoader(self.raw(), loader.as_ptr());
        }
        Ok(())
    }

    /// Sets global font loader flags
    ///
    /// These flags apply to all fonts loaded with this atlas unless overridden
    /// in individual FontConfig instances.
    pub fn set_font_loader_flags(&self, flags: FontLoaderFlags) {
        self.assert_mutation_allowed("FontAtlas::set_font_loader_flags()");
        unsafe {
            (*self.raw()).FontLoaderFlags = flags.0;
        }
    }

    /// Gets the current font loader flags
    pub fn font_loader_flags(&self) -> FontLoaderFlags {
        unsafe { FontLoaderFlags((*self.raw()).FontLoaderFlags) }
    }
}

pub(super) const RASTERIZER_MULTIPLY_MAX: f32 = 16_000_000.0;

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn assert_non_negative_i8(caller: &str, name: &str, value: i8) {
    assert!(value >= 0, "{caller} {name} must be non-negative");
}

pub(super) fn validate_font_size_pixels(caller: &str, name: &str, size_pixels: f32) -> f32 {
    assert_non_negative_f32(caller, name, size_pixels);
    size_pixels
}

pub(super) fn validate_font_size_pixels_option(
    caller: &str,
    name: &str,
    size_pixels: Option<f32>,
) -> f32 {
    let size_pixels = size_pixels.unwrap_or(0.0);
    validate_font_size_pixels(caller, name, size_pixels)
}

fn encode_glyph_ranges(caller: &str, ranges: &[(u32, u32)]) -> Vec<sys::ImWchar> {
    if ranges.is_empty() {
        return Vec::new();
    }
    const IMWCHAR_MAX: u32 = if std::mem::size_of::<sys::ImWchar>() == 2 {
        0xFFFF
    } else {
        0x10FFFF
    };
    let capacity = ranges
        .len()
        .checked_mul(2)
        .and_then(|length| length.checked_add(1))
        .expect("glyph range count overflowed usize");
    let mut encoded = Vec::with_capacity(capacity);
    for &(start, end) in ranges {
        assert!(start != 0, "{caller} glyph range cannot start at U+0000");
        assert!(
            start <= end,
            "{caller} glyph range start must be less than or equal to its end"
        );
        assert!(
            end <= IMWCHAR_MAX,
            "{caller} glyph range exceeds ImWchar's maximum codepoint {IMWCHAR_MAX:#x}"
        );
        encoded.push(start as sys::ImWchar);
        encoded.push(end as sys::ImWchar);
    }
    encoded.push(0);
    encoded
}

fn assert_reference_font_size_for_metrics(
    caller: &str,
    size_pixels: f32,
    has_reference_size_dependent_metrics: bool,
) {
    assert!(
        !has_reference_size_dependent_metrics || size_pixels > 0.0,
        "{caller} glyph offset/advance overrides require a positive reference size"
    );
}
