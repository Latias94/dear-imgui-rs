use crate::fonts::atlas::FontLoadError;
use crate::fonts::atlas::config::FontConfig;
use crate::fonts::atlas::loader::FontLoader;
use crate::fonts::atlas::validated::validate_font;

use super::FontAtlas;

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
unsafe extern "C" {
    #[link_name = "ImGuiFreeType_GetFontLoader"]
    fn native_freetype_loader() -> *const crate::sys::ImFontLoader;
}

#[derive(Clone, Copy)]
pub(super) enum FontBytesLoader {
    StbTrueType,
    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    FreeType,
}

impl FontBytesLoader {
    pub(super) fn for_atlas(atlas: &FontAtlas) -> Result<Self, FontLoadError> {
        let loader = unsafe { (*atlas.raw()).FontLoader };
        #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
        if loader.is_null() || loader == unsafe { native_freetype_loader() } {
            return Ok(Self::FreeType);
        }
        if loader.is_null() || loader == FontLoader::stb_truetype().as_ptr() {
            return Ok(Self::StbTrueType);
        }
        Err(FontLoadError::UnsupportedAtlasLoader)
    }

    pub(super) fn validate(
        self,
        data: &[u8],
        index: u32,
        config: &FontConfig,
    ) -> Result<(), FontLoadError> {
        if !config.raw.FontLoader.is_null() {
            return Err(FontLoadError::LoaderOverride);
        }
        match self {
            Self::StbTrueType => {
                if index != 0 {
                    return Err(FontLoadError::InvalidFontIndex { index });
                }
                validate_font(data)?;
                Ok(())
            }
            #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
            Self::FreeType => validate_freetype_face(data, index),
        }
    }

    /// Pin stb per source even when sys dependency feature unification selects a different
    /// native atlas default. FreeType must stay atlas-wide because it uses LoaderInit state.
    pub(super) fn configure(self, config: &mut FontConfig, index: u32) {
        config.raw.FontNo = index;
        match self {
            Self::StbTrueType => config.raw.FontLoader = FontLoader::stb_truetype().as_ptr(),
            #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
            Self::FreeType => {}
        }
    }
}

/// FreeType receives the actual byte length and parses the font itself. This only checks the
/// face selector and rules out glyph formats not handled by the pinned ImGui wrapper.
#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
fn validate_freetype_face(data: &[u8], index: u32) -> Result<(), FontLoadError> {
    let face_index =
        usize::from(u16::try_from(index).map_err(|_| FontLoadError::InvalidFontIndex { index })?);
    let face_offset = if data.starts_with(b"ttcf") {
        if !matches!(read_u32(data, 4), Some(0x0001_0000 | 0x0002_0000)) {
            return Err(FontLoadError::InvalidSfnt);
        }
        let face_count = read_u32(data, 8).ok_or(FontLoadError::InvalidSfnt)?;
        if index >= face_count {
            return Err(FontLoadError::InvalidFontIndex { index });
        }
        read_u32(data, 12 + face_index * 4).ok_or(FontLoadError::InvalidSfnt)? as usize
    } else {
        if index != 0 {
            return Err(FontLoadError::InvalidFontIndex { index });
        }
        0
    };
    let face = data.get(face_offset..).ok_or(FontLoadError::InvalidSfnt)?;
    let header = face.first_chunk::<12>().ok_or(FontLoadError::InvalidSfnt)?;
    if !matches!(&header[..4], b"\0\x01\0\0" | b"OTTO" | b"true") {
        return Err(FontLoadError::InvalidSfnt);
    }
    let count = usize::from(u16::from_be_bytes([header[4], header[5]]));
    let directory = face
        .get(12..12 + count * 16)
        .ok_or(FontLoadError::InvalidSfnt)?;
    for record in directory.as_chunks::<16>().0 {
        let format = match &record[..4] {
            b"SVG " => "OpenType SVG",
            b"EBLC" | b"EBDT" | b"CBLC" | b"CBDT" | b"bloc" | b"bdat" | b"sbix" => {
                "embedded bitmap"
            }
            _ => continue,
        };
        return Err(FontLoadError::UnsupportedFreeTypeGlyphs { format });
    }
    Ok(())
}

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(*data.get(offset..)?.first_chunk()?))
}
