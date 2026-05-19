use super::config::FontConfig;

/// A source for font data with v1.92+ dynamic font support
#[derive(Clone, Debug)]
pub enum FontSource<'a> {
    /// Default font included with the library (ProggyClean.ttf)
    ///
    /// With v1.92+, size_pixels can be 0.0 for dynamic sizing
    DefaultFontData {
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Binary TTF/OTF font data
    ///
    /// With v1.92+, size_pixels can be 0.0 for dynamic sizing
    TtfData {
        data: &'a [u8],
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Compressed TTF font data (stb-compressed)
    ///
    /// Dear ImGui decompresses immediately and keeps the decompressed buffer owned by the atlas.
    CompressedTtfData {
        data: &'a [u8],
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Compressed + base85-encoded TTF font data
    ///
    /// The provided string is converted into a NUL-terminated `CString` for Dear ImGui.
    CompressedTtfBase85 {
        data: &'a str,
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },

    /// Font from file path
    ///
    /// With v1.92+, size_pixels can be 0.0 for dynamic sizing
    TtfFile {
        path: &'a str,
        size_pixels: Option<f32>,
        config: Option<FontConfig>,
    },
}

impl<'a> FontSource<'a> {
    /// Creates a default font source with dynamic sizing
    pub fn default_font() -> Self {
        Self::DefaultFontData {
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a default font source with specific size
    pub fn default_font_with_size(size: f32) -> Self {
        Self::DefaultFontData {
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a TTF data source with dynamic sizing
    pub fn ttf_data(data: &'a [u8]) -> Self {
        Self::TtfData {
            data,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a TTF data source with specific size
    pub fn ttf_data_with_size(data: &'a [u8], size: f32) -> Self {
        Self::TtfData {
            data,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a compressed TTF data source with dynamic sizing
    pub fn compressed_ttf_data(data: &'a [u8]) -> Self {
        Self::CompressedTtfData {
            data,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a compressed TTF data source with specific size
    pub fn compressed_ttf_data_with_size(data: &'a [u8], size: f32) -> Self {
        Self::CompressedTtfData {
            data,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a base85 compressed TTF source with dynamic sizing
    pub fn compressed_ttf_base85(data: &'a str) -> Self {
        Self::CompressedTtfBase85 {
            data,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a base85 compressed TTF source with specific size
    pub fn compressed_ttf_base85_with_size(data: &'a str, size: f32) -> Self {
        Self::CompressedTtfBase85 {
            data,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Creates a TTF file source with dynamic sizing
    pub fn ttf_file(path: &'a str) -> Self {
        Self::TtfFile {
            path,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a TTF file source with specific size
    pub fn ttf_file_with_size(path: &'a str, size: f32) -> Self {
        Self::TtfFile {
            path,
            size_pixels: Some(size),
            config: None,
        }
    }

    /// Sets the font configuration for this source
    pub fn with_config(mut self, config: FontConfig) -> Self {
        match &mut self {
            Self::DefaultFontData { config: cfg, .. } => *cfg = Some(config),
            Self::TtfData { config: cfg, .. } => *cfg = Some(config),
            Self::CompressedTtfData { config: cfg, .. } => *cfg = Some(config),
            Self::CompressedTtfBase85 { config: cfg, .. } => *cfg = Some(config),
            Self::TtfFile { config: cfg, .. } => *cfg = Some(config),
        }
        self
    }
}
