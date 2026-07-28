use super::config::FontConfig;

/// A font source with v1.92+ dynamic font support.
///
/// Constructors ending in `_with_size` set the font's reference size. That
/// size is used by [`crate::Ui::push_font`] and reference-size-dependent font
/// metrics, but it does not prevent Dear ImGui from baking the font at other
/// runtime sizes through [`crate::Ui::push_font_with_size`].
///
/// External font parsers used by Dear ImGui do not receive a reliable input
/// boundary for every format. Consequently, raw font sources can only be
/// created through the `unsafe` constructors on this type. The embedded
/// default font remains entirely safe.
#[derive(Clone, Debug)]
pub struct FontSource<'a> {
    pub(super) kind: FontSourceKind<'a>,
    pub(super) size_pixels: Option<f32>,
    pub(super) config: Option<FontConfig>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum FontSourceKind<'a> {
    Default,
    TtfData(&'a [u8]),
    CompressedTtfData(&'a [u8]),
    CompressedTtfBase85(&'a str),
    TtfFile(&'a str),
}

impl<'a> FontSource<'a> {
    /// Creates an embedded default font source with dynamic sizing.
    pub fn default_font() -> Self {
        Self {
            kind: FontSourceKind::Default,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates an embedded default font source with a reference size.
    pub fn default_font_with_size(size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..Self::default_font()
        }
    }

    /// Creates a TTF/OTF memory source with dynamic sizing.
    ///
    /// # Safety
    ///
    /// `data` must contain a complete font that is valid for the loader selected
    /// by this source's eventual [`FontConfig`]. The data must remain unchanged
    /// until it is passed to [`crate::FontAtlas::add_font`]. Native font loaders
    /// may otherwise read beyond the slice boundary.
    pub unsafe fn ttf_data(data: &'a [u8]) -> Self {
        Self {
            kind: FontSourceKind::TtfData(data),
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a TTF/OTF memory source with a reference size.
    ///
    /// # Safety
    ///
    /// The requirements of [`FontSource::ttf_data`] apply.
    pub unsafe fn ttf_data_with_size(data: &'a [u8], size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..unsafe { Self::ttf_data(data) }
        }
    }

    /// Creates an stb-compressed TTF source with dynamic sizing.
    ///
    /// # Safety
    ///
    /// `data` must be the complete, unmodified output of Dear ImGui's
    /// `binary_to_compressed_c` tool. Its decompressed payload must be a complete
    /// font that is valid for the loader selected by this source's eventual
    /// [`FontConfig`]. The data must remain unchanged until it is passed to
    /// [`crate::FontAtlas::add_font`]. Dear ImGui's stb decompressor does not
    /// enforce the supplied input length, and its font parser may not enforce the
    /// decompressed allocation boundary.
    pub unsafe fn compressed_ttf_data(data: &'a [u8]) -> Self {
        Self {
            kind: FontSourceKind::CompressedTtfData(data),
            size_pixels: None,
            config: None,
        }
    }

    /// Creates an stb-compressed TTF source with a reference size.
    ///
    /// # Safety
    ///
    /// The requirements of [`FontSource::compressed_ttf_data`] apply.
    pub unsafe fn compressed_ttf_data_with_size(data: &'a [u8], size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..unsafe { Self::compressed_ttf_data(data) }
        }
    }

    /// Creates a base85-encoded stb-compressed TTF source with dynamic sizing.
    ///
    /// # Safety
    ///
    /// `data` must be the complete, unmodified base85 output of Dear ImGui's
    /// `binary_to_compressed_c` tool. Its decoded and decompressed payload must be
    /// a complete font that is valid for the loader selected by this source's
    /// eventual [`FontConfig`]. The data must remain unchanged until it is passed
    /// to [`crate::FontAtlas::add_font`]. Dear ImGui's decoder assumes complete
    /// five-character groups and an internally terminated compressed stream, and
    /// its font parser may not enforce the decompressed allocation boundary.
    pub unsafe fn compressed_ttf_base85(data: &'a str) -> Self {
        Self {
            kind: FontSourceKind::CompressedTtfBase85(data),
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a base85-encoded stb-compressed TTF source with a reference size.
    ///
    /// # Safety
    ///
    /// The requirements of [`FontSource::compressed_ttf_base85`] apply.
    pub unsafe fn compressed_ttf_base85_with_size(data: &'a str, size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..unsafe { Self::compressed_ttf_base85(data) }
        }
    }

    /// Creates a font-file source with dynamic sizing.
    ///
    /// # Safety
    ///
    /// If `path` exists when [`crate::FontAtlas::add_font`] is called, it must
    /// identify a complete font that is valid for the loader selected by this
    /// source's eventual [`FontConfig`]. The file must not be replaced or
    /// modified while it is being added.
    pub unsafe fn ttf_file(path: &'a str) -> Self {
        Self {
            kind: FontSourceKind::TtfFile(path),
            size_pixels: None,
            config: None,
        }
    }

    /// Creates a font-file source with a reference size.
    ///
    /// # Safety
    ///
    /// The requirements of [`FontSource::ttf_file`] apply.
    pub unsafe fn ttf_file_with_size(path: &'a str, size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..unsafe { Self::ttf_file(path) }
        }
    }

    /// Sets the font configuration for this source.
    pub fn with_config(mut self, config: FontConfig) -> Self {
        self.config = Some(config);
        self
    }
}
