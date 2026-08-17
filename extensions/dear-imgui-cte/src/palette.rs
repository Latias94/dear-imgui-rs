use crate::{CteError, CteResult, sys};
use std::ptr::NonNull;

/// Semantic color slots used by ImGuiColorTextEdit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(usize)]
pub enum PaletteColor {
    Text,
    Keyword,
    Declaration,
    Number,
    String,
    Punctuation,
    Preprocessor,
    Identifier,
    KnownIdentifier,
    Comment,
    Background,
    Cursor,
    Selection,
    Whitespace,
    MatchingBracketBackground,
    MatchingBracketActive,
    MatchingBracketLevel1,
    MatchingBracketLevel2,
    MatchingBracketLevel3,
    MatchingBracketError,
    LineNumber,
    CurrentLineNumber,
}

impl PaletteColor {
    pub const COUNT: usize = sys::count as usize;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Text,
        Self::Keyword,
        Self::Declaration,
        Self::Number,
        Self::String,
        Self::Punctuation,
        Self::Preprocessor,
        Self::Identifier,
        Self::KnownIdentifier,
        Self::Comment,
        Self::Background,
        Self::Cursor,
        Self::Selection,
        Self::Whitespace,
        Self::MatchingBracketBackground,
        Self::MatchingBracketActive,
        Self::MatchingBracketLevel1,
        Self::MatchingBracketLevel2,
        Self::MatchingBracketLevel3,
        Self::MatchingBracketError,
        Self::LineNumber,
        Self::CurrentLineNumber,
    ];

    pub(crate) const fn into_raw(self) -> sys::Color {
        self as sys::Color
    }

    pub(crate) const fn into_position(self) -> std::ffi::c_int {
        self as std::ffi::c_int
    }
}

/// An owned copy of all editor palette colors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    colors: [u32; PaletteColor::COUNT],
}

impl Palette {
    /// Copies the built-in dark palette.
    pub fn dark() -> Self {
        unsafe { Self::copy_from_raw(sys::TextEditor_GetDarkPalette()) }
    }

    /// Copies the built-in light palette.
    pub fn light() -> Self {
        unsafe { Self::copy_from_raw(sys::TextEditor_GetLightPalette()) }
    }

    pub const fn from_colors(colors: [u32; PaletteColor::COUNT]) -> Self {
        Self { colors }
    }

    pub const fn colors(&self) -> &[u32; PaletteColor::COUNT] {
        &self.colors
    }

    pub const fn get(&self, color: PaletteColor) -> u32 {
        self.colors[color as usize]
    }

    pub fn set(&mut self, color: PaletteColor, value: u32) {
        self.colors[color as usize] = value;
    }

    pub(crate) unsafe fn copy_from_raw(raw: *const sys::Palette) -> Self {
        assert!(!raw.is_null(), "cimCTE returned a null palette");
        let mut colors = [0; PaletteColor::COUNT];
        for color in PaletteColor::ALL {
            colors[color as usize] = unsafe { sys::Palette_const_get(raw, color.into_raw()) };
        }
        Self { colors }
    }

    pub(crate) fn with_native<R>(&self, f: impl FnOnce(*const sys::Palette) -> R) -> CteResult<R> {
        let raw =
            NonNull::new(unsafe { sys::Palette_Palette() }).ok_or(CteError::CreationFailed {
                object: "temporary Palette",
            })?;
        let native = NativePalette(raw);
        for color in PaletteColor::ALL {
            unsafe {
                sys::Palette_set(
                    native.0.as_ptr(),
                    self.colors[color as usize],
                    color.into_position(),
                );
            }
        }
        Ok(f(native.0.as_ptr()))
    }
}

struct NativePalette(NonNull<sys::Palette>);

impl Drop for NativePalette {
    fn drop(&mut self) {
        unsafe { sys::Palette_destroy(self.0.as_ptr()) };
    }
}
