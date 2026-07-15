use crate::sys;

/// Borrowed handle to a complete Dear ImGui font-loader callback table.
///
/// Safe code can obtain the built-in stb_truetype loader. Custom loaders must
/// be created in native code and enter through [`FontLoader::from_raw`], whose
/// safety contract covers every callback and stored pointer.
#[repr(transparent)]
pub struct FontLoader(sys::ImFontLoader);

impl FontLoader {
    /// Return Dear ImGui's built-in stb_truetype loader.
    #[doc(alias = "ImFontAtlasGetFontLoaderForStbTruetype")]
    pub fn stb_truetype() -> &'static Self {
        let raw = unsafe { sys::igImFontAtlasGetFontLoaderForStbTruetype() };
        assert!(
            !raw.is_null(),
            "Dear ImGui returned a null stb_truetype font loader"
        );
        unsafe { &*raw.cast::<Self>() }
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
        &self.0
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

#[cfg(test)]
mod tests {
    #[test]
    fn builtin_font_loader_has_required_callbacks() {
        let loader = super::FontLoader::stb_truetype();
        let raw = loader.as_ptr();
        assert!(!raw.is_null());
        assert!(!unsafe { (*raw).Name }.is_null());
        assert!(unsafe { (*raw).FontBakedLoadGlyph }.is_some());
    }
}
