use std::ffi::CString;

use crate::sys;

use super::core::FontAtlas;

/// Font loader interface for custom font backends
///
/// This provides a safe Rust interface to Dear ImGui's ImFontLoader system,
/// allowing custom font loading implementations.
pub struct FontLoader {
    raw: sys::ImFontLoader,
    _name: CString,
}

impl FontLoader {
    /// Creates a new font loader with the given name
    pub fn new(name: &str) -> Result<Self, std::ffi::NulError> {
        let name_cstring = CString::new(name)?;
        // Initialize via ImGui constructor to future-proof defaults
        let mut raw = unsafe {
            let p = sys::ImFontLoader_ImFontLoader();
            if p.is_null() {
                panic!("ImFontLoader_ImFontLoader() returned null");
            }
            let v = *p;
            sys::ImFontLoader_destroy(p);
            v
        };
        raw.Name = name_cstring.as_ptr();

        Ok(Self {
            raw,
            _name: name_cstring,
        })
    }

    /// Returns a pointer to the raw ImFontLoader
    pub(crate) fn as_ptr(&self) -> *const sys::ImFontLoader {
        &self.raw
    }

    /// Sets the loader initialization callback
    pub fn with_loader_init<F>(self, _callback: F) -> Self
    where
        F: Fn(&mut FontAtlas) -> bool + 'static,
    {
        // Note: For now, we'll use the default STB TrueType loader
        // Custom callbacks would require more complex lifetime management
        self
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
