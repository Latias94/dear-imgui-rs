//! Structural validation for example-owned font files before crossing the unsafe font-loader API.

/// The native loader selected for an example font.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LoaderKind {
    /// Dear ImGui's built-in stb_truetype loader, which requires TrueType outlines.
    StbTrueType,
    /// Dear ImGui's FreeType loader, which also supports CFF and bitmap/color fonts.
    FreeType,
}

/// Validate that `data` is a structurally complete font supported by `loader`.
///
/// `ttf-parser` bounds-checks the sfnt container and parses the mandatory face tables. The stb
/// route is intentionally narrower than FreeType: it accepts only fonts with parsed TrueType
/// outlines, because color-only and CFF fonts are not supported by the default loader.
pub fn validate_font_data(data: &[u8], loader: LoaderKind) -> Result<(), String> {
    let face = ttf_parser::Face::parse(data, 0)
        .map_err(|error| format!("invalid OpenType font structure: {error:?}"))?;
    let tables = face.tables();

    if tables.cmap.is_none() {
        return Err("font is missing a parsed character map".to_owned());
    }
    if face.number_of_glyphs() == 0 {
        return Err("font declares no glyphs".to_owned());
    }
    if loader == LoaderKind::StbTrueType && tables.glyf.is_none() {
        return Err(
            "the stb_truetype loader requires parsed TrueType glyf outlines; enable FreeType for CFF or bitmap/color fonts"
                .to_owned(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LoaderKind, validate_font_data};

    #[test]
    fn accepts_the_bundled_truetype_font() {
        let font = include_bytes!(
            "../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
        );
        assert!(validate_font_data(font, LoaderKind::StbTrueType).is_ok());
    }

    #[test]
    fn rejects_truncated_font_data() {
        assert!(validate_font_data(&[0; 12], LoaderKind::StbTrueType).is_err());
    }
}
