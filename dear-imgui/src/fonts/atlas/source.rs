use super::config::FontConfig;
use super::loader::FontLoader;
use super::validated::StbTrueTypeFontData;

/// A font source with v1.92+ dynamic font support.
///
/// Constructors ending in `_with_size` set the font's reference size. That
/// size is used by [`crate::Ui::push_font`] and reference-size-dependent font
/// metrics, but it does not prevent Dear ImGui from baking the font at other
/// runtime sizes through [`crate::Ui::push_font_with_size`].
/// For merged fonts, the first source establishes this reference size; later
/// source sizes control their metrics relative to that destination font.
///
/// External font parsers used by Dear ImGui do not receive a reliable input
/// boundary for every format. Consequently, raw, compressed, and custom-loader
/// sources can only be created through the `unsafe` constructors on this type.
/// The embedded defaults, [`Self::ttf_bytes`], and [`StbTrueTypeFontData`] path are safe.
#[derive(Clone, Debug)]
pub struct FontSource<'a> {
    pub(super) kind: FontSourceKind<'a>,
    pub(super) size_pixels: Option<f32>,
    pub(super) config: Option<FontConfig>,
}

#[derive(Clone, Debug)]
pub(super) enum FontSourceKind<'a> {
    Default,
    DefaultVector,
    DefaultBitmap,
    StbTrueType(StbTrueTypeFontData),
    TtfBytes { data: &'a [u8], font_index: u32 },
    TtfData(&'a [u8]),
    CompressedTtfData(&'a [u8]),
    CompressedTtfBase85(&'a str),
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

    /// Creates the embedded scalable default font with dynamic sizing.
    pub fn default_vector() -> Self {
        Self {
            kind: FontSourceKind::DefaultVector,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates the embedded scalable default font with a reference size.
    pub fn default_vector_with_size(size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..Self::default_vector()
        }
    }

    /// Creates the embedded pixel-clean default font with dynamic sizing.
    pub fn default_bitmap() -> Self {
        Self {
            kind: FontSourceKind::DefaultBitmap,
            size_pixels: None,
            config: None,
        }
    }

    /// Creates the embedded pixel-clean default font with a reference size.
    pub fn default_bitmap_with_size(size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..Self::default_bitmap()
        }
    }

    /// Creates an owned, validated TrueType source for Dear ImGui's stb_truetype loader.
    ///
    /// This source pins the native loader and standalone font index covered by
    /// [`StbTrueTypeFontData`]'s bounded-read proof, even when the crate is built with the
    /// `freetype` feature.
    pub fn stb_truetype(data: StbTrueTypeFontData) -> Self {
        Self {
            kind: FontSourceKind::StbTrueType(data),
            size_pixels: None,
            config: None,
        }
    }

    /// Creates an owned, validated stb_truetype source with a reference size.
    pub fn stb_truetype_with_size(data: StbTrueTypeFontData, size: f32) -> Self {
        Self {
            size_pixels: Some(size),
            ..Self::stb_truetype(data)
        }
    }

    /// Creates a checked memory font source using the atlas's built-in font loader.
    ///
    /// [`crate::FontAtlas::try_add_font`] validates the selected face and copies the bytes into
    /// atlas-owned memory. The slice can be dropped after the call returns. Per-source loader
    /// overrides are not allowed. A new font without an explicit reference size uses a 13 px
    /// legacy fallback and still supports baking at other runtime sizes.
    ///
    /// With the `freetype` feature on native targets, an atlas using the built-in FreeType
    /// loader accepts outline-based TTF, OTF and TTC faces, including supported outline
    /// color glyphs with [`crate::FontLoaderFlags::LOAD_COLOR`]. OpenType SVG and embedded
    /// bitmap tables are not accepted by this path. FreeType is a native parser, not a sandbox.
    ///
    /// The built-in stb_truetype path accepts only the standalone TrueType subset described by
    /// [`StbTrueTypeFontData`]. On wasm32 only this path is available, and
    /// [`crate::Context::font_atlas`] still requires `wasm-font-atlas-experimental`.
    pub fn ttf_bytes(data: &'a [u8]) -> Self {
        Self {
            kind: FontSourceKind::TtfBytes {
                data,
                font_index: 0,
            },
            size_pixels: None,
            config: None,
        }
    }

    /// Selects the zero-based face for a [`Self::ttf_bytes`] source.
    ///
    /// Nonzero indices require a font collection and the native FreeType loader. This is a
    /// face index, not a FreeType named-instance selector.
    ///
    /// # Panics
    ///
    /// Panics if this source was not created with [`Self::ttf_bytes`].
    pub fn font_index(mut self, index: u32) -> Self {
        match &mut self.kind {
            FontSourceKind::TtfBytes { font_index, .. } => *font_index = index,
            _ => panic!("FontSource::font_index() requires a FontSource::ttf_bytes() source"),
        }
        self
    }

    /// Creates a TTF/OTF memory source with dynamic sizing.
    ///
    /// Prefer [`Self::ttf_bytes`] for checked font loading.
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

    /// Sets the font configuration for this source.
    pub fn with_config(mut self, config: FontConfig) -> Self {
        if matches!(&self.kind, FontSourceKind::StbTrueType(_)) {
            assert_stb_truetype_config(&config, "FontSource::with_config()");
        }
        self.config = Some(config);
        self
    }
}

pub(super) fn assert_stb_truetype_config(config: &FontConfig, caller: &str) {
    assert!(
        config.raw.FontLoader.is_null()
            || config.raw.FontLoader == FontLoader::stb_truetype().as_ptr(),
        "{caller} cannot override a validated stb_truetype source with another font loader"
    );
    assert_eq!(
        config.raw.FontNo, 0,
        "{caller} cannot override the standalone font index of a validated stb_truetype source"
    );
}
