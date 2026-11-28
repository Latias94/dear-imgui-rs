use std::ffi::CString;
use std::ptr;

use crate::FontError;
use crate::sys;

use super::config::{validate_font_size_pixels, validate_font_size_pixels_option};
use super::state::font_atlas_contains_font;
use super::validated::{StbTrueTypeFontData, validate_font};
use super::{FontAtlas, FontConfig, FontId, FontLoader};

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
    kind: FontSourceKind<'a>,
    size_pixels: Option<f32>,
    config: Option<FontConfig>,
}

#[derive(Clone, Debug)]
enum FontSourceKind<'a> {
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
    /// The built-in stb_truetype path checks the standalone TrueType subset described by
    /// [`StbTrueTypeFontData`], including the exact character map and glyph data used by that
    /// loader. On wasm32, this is the available loader for checked bytes, and
    /// [`crate::Context::font_atlas`] requires `wasm-font-atlas-experimental`.
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

fn assert_stb_truetype_config(config: &FontConfig, caller: &str) {
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

impl FontAtlas {
    /// Adds one font, optionally assembled from multiple merged sources.
    ///
    /// The first source honors its [`FontConfig::merge_mode`] value. Every
    /// later source is merged, regardless of its configured value.
    /// Panics if loading fails. Use [`Self::try_add_font`] for external font bytes.
    #[doc(alias = "AddFont")]
    pub fn add_font(&self, font_sources: &[FontSource<'_>]) -> FontId {
        self.try_add_font(font_sources)
            .unwrap_or_else(|error| panic!("FontAtlas::add_font() failed: {error}"))
    }

    /// Adds one font and returns an error if its data is invalid or a native loader rejects it.
    ///
    /// Rust-side checks run for all sources before the atlas changes. A native error in a
    /// later source does not roll back earlier sources that were already added.
    ///
    /// # Panics
    ///
    /// Panics if atlas mutation is not allowed or a source has an invalid font or merge
    /// configuration.
    pub fn try_add_font(&self, font_sources: &[FontSource<'_>]) -> Result<FontId, FontError> {
        const CALLER: &str = "FontAtlas::add_font()";
        self.assert_mutation_allowed(CALLER);
        let Some((head, tail)) = font_sources.split_first() else {
            return Err(FontError::NoSources);
        };

        let checked_loader = font_sources
            .iter()
            .any(|source| matches!(source.kind, FontSourceKind::TtfBytes { .. }))
            .then(|| FontBytesLoader::for_atlas(self))
            .transpose()?;
        for (index, source) in font_sources.iter().enumerate() {
            Self::validate_font_source(source, index != 0, checked_loader)?;
        }
        self.validate_merged_sources(head, tail);

        let font_id = self.add_font_internal(head, false, checked_loader)?;
        for source in tail {
            self.add_font_internal(source, true, checked_loader)?;
        }
        Ok(font_id)
    }

    fn validate_font_source(
        font_source: &FontSource<'_>,
        merge_mode: bool,
        checked_loader: Option<FontBytesLoader>,
    ) -> Result<(), FontError> {
        const CALLER: &str = "FontAtlas::add_font()";
        let size = validate_font_size_pixels_option(CALLER, "size_pixels", font_source.size_pixels);
        let config = configured_font_source(font_source.config.as_ref(), size, merge_mode);

        match &font_source.kind {
            FontSourceKind::Default
            | FontSourceKind::DefaultVector
            | FontSourceKind::DefaultBitmap => config.validate_for_add_font_default(CALLER),
            FontSourceKind::StbTrueType(data) => {
                assert_stb_truetype_config(&config, CALLER);
                config.validate_for_add_font_with_size(CALLER, size);
                validate_ttf_data_length(data.as_bytes())?;
            }
            FontSourceKind::TtfBytes { data, font_index } => {
                config.validate_for_add_font_with_size(CALLER, size);
                validate_ttf_data_length(data)?;
                checked_loader
                    .expect("checked font sources require a built-in loader")
                    .validate(data, *font_index, &config)?;
            }
            FontSourceKind::TtfData(data) => {
                config.validate_for_add_font_with_size(CALLER, size);
                validate_ttf_data_length(data)?;
            }
            FontSourceKind::CompressedTtfData(data) => {
                config.validate_for_add_font_with_size(CALLER, size);
                validate_compressed_ttf_length(CALLER, data);
            }
            FontSourceKind::CompressedTtfBase85(data) => {
                config.validate_for_add_font_with_size(CALLER, size);
                validate_base85_compressed_ttf(CALLER, data);
            }
        }
        Ok(())
    }

    fn validate_merged_sources(&self, head: &FontSource<'_>, tail: &[FontSource<'_>]) {
        let size = head.size_pixels.unwrap_or(0.0);
        let head_config = configured_font_source(head.config.as_ref(), size, false);
        let destination_uses_implicit_size = if head_config.raw.MergeMode {
            self.validate_merge_target(
                &head_config.raw,
                Self::font_source_merge_input_size(head),
                "FontAtlas::add_font()",
            );
            self.merge_destination_uses_implicit_size(&head_config.raw, "FontAtlas::add_font()")
        } else {
            Self::font_source_creates_implicit_reference_size(head)
        };

        if !destination_uses_implicit_size {
            return;
        }

        for source in tail {
            assert!(
                Self::font_source_merge_input_size(source) == 0.0,
                "FontAtlas::add_font() cannot merge a source with an explicit reference size into a destination font that uses an implicit reference size"
            );
        }
    }

    fn font_source_creates_implicit_reference_size(source: &FontSource<'_>) -> bool {
        let size = source.size_pixels.unwrap_or(0.0);
        configured_font_source(source.config.as_ref(), size, false)
            .raw
            .SizePixels
            <= 0.0
    }

    fn font_source_merge_input_size(source: &FontSource<'_>) -> f32 {
        let size = source.size_pixels.unwrap_or(0.0);
        let config = configured_font_source(source.config.as_ref(), size, true);
        if matches!(
            &source.kind,
            FontSourceKind::Default | FontSourceKind::DefaultVector | FontSourceKind::DefaultBitmap
        ) && config.raw.SizePixels <= 0.0
        {
            13.0
        } else {
            config.raw.SizePixels
        }
    }

    fn font_config_for_ffi(config: &FontConfig) -> sys::ImFontConfig {
        let mut raw = config.raw;
        raw.GlyphExcludeRanges = config
            .owned_glyph_exclude_ranges()
            .map_or(ptr::null(), |ranges| ranges.as_ptr());
        raw
    }

    fn validate_merge_target(
        &self,
        config: &sys::ImFontConfig,
        merge_input_size: f32,
        caller: &str,
    ) {
        if !config.MergeMode {
            return;
        }

        assert!(
            !self.merge_destination_uses_implicit_size(config, caller) || merge_input_size == 0.0,
            "{caller} cannot merge a source with an explicit reference size into a destination font that uses an implicit reference size"
        );
    }

    fn merge_destination(&self, config: &sys::ImFontConfig, caller: &str) -> *mut sys::ImFont {
        let atlas = self.raw();
        unsafe {
            let fonts = &(*atlas).Fonts;
            assert!(
                fonts.Size > 0 && !fonts.Data.is_null(),
                "{caller} cannot use merge mode for the first font in an atlas"
            );
            let destination = if config.DstFont.is_null() {
                *fonts.Data.add((fonts.Size - 1) as usize)
            } else {
                config.DstFont
            };
            assert!(
                font_atlas_contains_font(atlas, destination),
                "{caller} merge destination does not belong to this font atlas"
            );
            destination
        }
    }

    fn merge_destination_uses_implicit_size(
        &self,
        config: &sys::ImFontConfig,
        caller: &str,
    ) -> bool {
        let destination = self.merge_destination(config, caller);
        unsafe {
            (*destination).LegacySize == 0.0
                || (*destination).Flags & sys::ImFontFlags_ImplicitRefSize != 0
        }
    }

    fn add_font_internal(
        &self,
        font_source: &FontSource<'_>,
        merge_mode: bool,
        checked_loader: Option<FontBytesLoader>,
    ) -> Result<FontId, FontError> {
        let size = validate_font_size_pixels_option(
            "FontAtlas::add_font()",
            "size_pixels",
            font_source.size_pixels,
        );
        let mut config = configured_font_source(font_source.config.as_ref(), size, merge_mode);
        match &font_source.kind {
            FontSourceKind::Default => Ok(self.add_font_default_internal(Some(&config))),
            FontSourceKind::DefaultVector => {
                Ok(self.add_font_default_vector_internal(Some(&config)))
            }
            FontSourceKind::DefaultBitmap => {
                Ok(self.add_font_default_bitmap_internal(Some(&config)))
            }
            FontSourceKind::StbTrueType(data) => {
                FontBytesLoader::StbTrueType.configure(&mut config, 0);
                // SAFETY: The owned bytes satisfy the pinned stb loader's read contract for face 0.
                unsafe { self.add_font_from_memory_ttf_internal(data.as_bytes(), &config) }
            }
            FontSourceKind::TtfBytes { data, font_index } => {
                checked_loader
                    .expect("checked font sources require a built-in loader")
                    .configure(&mut config, *font_index);
                if !config.raw.MergeMode && config.raw.SizePixels == 0.0 {
                    config.raw.SizePixels = 13.0;
                    config.raw.Flags |= sys::ImFontFlags_ImplicitRefSize;
                }
                // SAFETY: Preflight checked this face for the same built-in loader. stb uses
                // the bounded-read proof, and FreeType receives the exact byte length.
                unsafe { self.add_font_from_memory_ttf_internal(data, &config) }
            }
            FontSourceKind::TtfData(data) => unsafe {
                self.add_font_from_memory_ttf_internal(data, &config)
            },
            FontSourceKind::CompressedTtfData(data) => unsafe {
                self.add_font_from_memory_compressed_ttf_internal(data, size, Some(&config))
                    .ok_or(FontError::NativeRejected)
            },
            FontSourceKind::CompressedTtfBase85(data) => unsafe {
                self.add_font_from_memory_compressed_base85_ttf_internal(data, size, Some(&config))
                    .ok_or(FontError::NativeRejected)
            },
        }
    }

    fn add_font_default_internal(&self, font_cfg: Option<&FontConfig>) -> crate::fonts::FontId {
        self.add_embedded_default_font(
            font_cfg,
            "FontAtlas::add_font()",
            sys::ImFontAtlas_AddFontDefault,
        )
    }

    fn add_font_default_vector_internal(
        &self,
        font_cfg: Option<&FontConfig>,
    ) -> crate::fonts::FontId {
        self.add_embedded_default_font(
            font_cfg,
            "FontAtlas::add_font()",
            sys::ImFontAtlas_AddFontDefaultVector,
        )
    }

    fn add_font_default_bitmap_internal(
        &self,
        font_cfg: Option<&FontConfig>,
    ) -> crate::fonts::FontId {
        self.add_embedded_default_font(
            font_cfg,
            "FontAtlas::add_font()",
            sys::ImFontAtlas_AddFontDefaultBitmap,
        )
    }

    /// Adds a font from TTF/OTF data copied into Dear ImGui-owned memory.
    ///
    /// # Safety
    ///
    /// The bytes and configuration must satisfy the selected loader's native read contract.
    /// An unbounded loader such as stb_truetype requires a complete font validated for the
    /// selected face. On success, the atlas owns a copy of the slice.
    #[doc(alias = "AddFontFromMemoryTTF")]
    unsafe fn add_font_from_memory_ttf_internal(
        &self,
        font_data: &[u8],
        config: &FontConfig,
    ) -> Result<FontId, FontError> {
        const CALLER: &str = "FontAtlas::add_font()";
        self.assert_mutation_allowed(CALLER);
        let font_data_len = validate_ttf_data_length(font_data)?;
        self.validate_merge_target(&config.raw, config.raw.SizePixels, CALLER);
        unsafe {
            let memory = sys::igMemAlloc(font_data.len());
            if memory.is_null() {
                return Err(FontError::AllocationFailed {
                    resource: "the font source",
                });
            }
            ptr::copy_nonoverlapping(font_data.as_ptr(), memory.cast::<u8>(), font_data.len());

            let mut raw_config = Self::font_config_for_ffi(config);
            raw_config.FontDataOwnedByAtlas = true;
            let is_merge = raw_config.MergeMode;

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw(),
                memory,
                font_data_len,
                raw_config.SizePixels,
                &raw_config,
                ptr::null(),
            );

            if font_ptr.is_null() {
                Err(FontError::NativeRejected)
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Ok(self.font_id_for_raw(font_ptr))
            }
        }
    }

    /// Adds complete stb-compressed TTF data.
    ///
    /// # Safety
    ///
    /// `compressed_font_data` must be the complete, unmodified output of Dear
    /// ImGui's `binary_to_compressed_c` tool, and its decompressed payload must
    /// be a complete font that is valid for the selected loader. The native stb
    /// decompressor ignores the supplied input length, and the font parser may
    /// otherwise read past its allocated buffer.
    #[doc(alias = "AddFontFromMemoryCompressedTTF")]
    unsafe fn add_font_from_memory_compressed_ttf_internal(
        &self,
        compressed_font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font()";
        self.assert_mutation_allowed(CALLER);
        validate_font_size_pixels(CALLER, "size_pixels", size_pixels);
        if let Some(config) = font_cfg {
            config.validate_for_add_font_with_size(CALLER, size_pixels);
        }
        let compressed_len = validate_compressed_ttf_length_fallible(compressed_font_data)?;

        let config = font_cfg.cloned().unwrap_or_default();
        let effective_size = effective_font_size(&config, size_pixels);
        self.validate_merge_target(&config.raw, effective_size, CALLER);

        unsafe {
            let raw_config = Self::font_config_for_ffi(&config);
            let is_merge = raw_config.MergeMode;
            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedTTF(
                self.raw(),
                compressed_font_data.as_ptr().cast(),
                compressed_len,
                size_pixels,
                &raw_config,
                ptr::null(),
            );

            if font_ptr.is_null() {
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(self.font_id_for_raw(font_ptr))
            }
        }
    }

    /// Adds complete base85-encoded stb-compressed TTF data.
    ///
    /// # Safety
    ///
    /// `compressed_font_data_base85` must be the complete, unmodified base85
    /// output of Dear ImGui's `binary_to_compressed_c` tool, and its decoded and
    /// decompressed payload must be a complete font that is valid for the
    /// selected loader. The native decoder assumes complete groups and an
    /// internally terminated compressed stream.
    #[doc(alias = "AddFontFromMemoryCompressedBase85TTF")]
    unsafe fn add_font_from_memory_compressed_base85_ttf_internal(
        &self,
        compressed_font_data_base85: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font()";
        self.assert_mutation_allowed(CALLER);
        validate_font_size_pixels(CALLER, "size_pixels", size_pixels);
        if let Some(config) = font_cfg {
            config.validate_for_add_font_with_size(CALLER, size_pixels);
        }
        validate_base85_compressed_ttf_fallible(compressed_font_data_base85)?;
        let base85 = CString::new(compressed_font_data_base85).ok()?;

        let config = font_cfg.cloned().unwrap_or_default();
        let effective_size = effective_font_size(&config, size_pixels);
        self.validate_merge_target(&config.raw, effective_size, CALLER);

        unsafe {
            let raw_config = Self::font_config_for_ffi(&config);
            let is_merge = raw_config.MergeMode;
            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedBase85TTF(
                self.raw(),
                base85.as_ptr(),
                size_pixels,
                &raw_config,
                ptr::null(),
            );

            if font_ptr.is_null() {
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(self.font_id_for_raw(font_ptr))
            }
        }
    }

    fn add_embedded_default_font(
        &self,
        font_cfg: Option<&FontConfig>,
        caller: &str,
        add_font: unsafe extern "C" fn(
            *mut sys::ImFontAtlas,
            *const sys::ImFontConfig,
        ) -> *mut sys::ImFont,
    ) -> crate::fonts::FontId {
        self.assert_mutation_allowed(caller);
        if let Some(config) = font_cfg {
            config.validate_for_add_font_default(caller);
            let merge_input_size = if config.raw.SizePixels > 0.0 {
                config.raw.SizePixels
            } else {
                13.0
            };
            self.validate_merge_target(&config.raw, merge_input_size, caller);
        }

        unsafe {
            let raw_config = font_cfg.map(Self::font_config_for_ffi);
            let config_ptr = raw_config
                .as_ref()
                .map_or(ptr::null(), |config| config as *const _);
            let font_ptr = add_font(self.raw(), config_ptr);
            assert!(
                !font_ptr.is_null(),
                "{caller} failed to add the embedded font"
            );
            if raw_config.as_ref().is_some_and(|config| config.MergeMode) {
                self.discard_bakes(0);
            }
            self.font_id_for_raw(font_ptr)
        }
    }
}

fn configured_font_source(
    config: Option<&FontConfig>,
    size_pixels: f32,
    merge_mode: bool,
) -> FontConfig {
    let mut config = config.cloned().unwrap_or_default();
    if size_pixels > 0.0 {
        config = config.size_pixels(size_pixels);
    }
    if merge_mode {
        config = config.merge_mode(true);
    }
    config
}

fn effective_font_size(config: &FontConfig, size_pixels: f32) -> f32 {
    if size_pixels > 0.0 {
        size_pixels
    } else {
        config.raw.SizePixels
    }
}

fn validate_ttf_data_length(data: &[u8]) -> Result<i32, FontError> {
    match i32::try_from(data.len()) {
        Ok(length) if length > 100 => Ok(length),
        _ => Err(FontError::InvalidDataLength { length: data.len() }),
    }
}

fn validate_compressed_ttf_length(caller: &str, data: &[u8]) {
    assert!(
        validate_compressed_ttf_length_fallible(data).is_some(),
        "{caller} compressed TTF data must contain a complete header and fit Dear ImGui's i32 length"
    );
}

fn validate_compressed_ttf_length_fallible(data: &[u8]) -> Option<i32> {
    if data.len() < 16 {
        return None;
    }
    i32::try_from(data.len()).ok()
}

fn validate_base85_compressed_ttf(caller: &str, data: &str) {
    assert!(
        validate_base85_compressed_ttf_fallible(data).is_some(),
        "{caller} base85 compressed TTF data must be NUL-free, contain at least four complete groups, and have a byte length divisible by five"
    );
}

fn validate_base85_compressed_ttf_fallible(data: &str) -> Option<()> {
    let bytes = data.as_bytes();
    if bytes.len() < 20 || !bytes.len().is_multiple_of(5) || bytes.contains(&0) {
        return None;
    }
    Some(())
}

#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
unsafe extern "C" {
    #[link_name = "ImGuiFreeType_GetFontLoader"]
    fn native_freetype_loader() -> *const crate::sys::ImFontLoader;
}

#[derive(Clone, Copy)]
enum FontBytesLoader {
    StbTrueType,
    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    FreeType,
}

impl FontBytesLoader {
    fn for_atlas(atlas: &FontAtlas) -> Result<Self, FontError> {
        let loader = unsafe { (*atlas.raw()).FontLoader };
        #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
        if loader.is_null() || loader == unsafe { native_freetype_loader() } {
            return Ok(Self::FreeType);
        }
        if loader.is_null() || loader == FontLoader::stb_truetype().as_ptr() {
            return Ok(Self::StbTrueType);
        }
        Err(FontError::UnsupportedAtlasLoader)
    }

    fn validate(self, data: &[u8], index: u32, config: &FontConfig) -> Result<(), FontError> {
        if !config.raw.FontLoader.is_null() {
            return Err(FontError::LoaderOverride);
        }
        match self {
            Self::StbTrueType => {
                if index != 0 {
                    return Err(FontError::InvalidFontIndex { index });
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
    fn configure(self, config: &mut FontConfig, index: u32) {
        config.raw.FontNo = index;
        match self {
            Self::StbTrueType => config.raw.FontLoader = FontLoader::stb_truetype().as_ptr(),
            #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
            Self::FreeType => {}
        }
    }
}

/// FreeType receives the actual byte length and validates glyphs itself. Check the face
/// selector and reject glyph formats the pinned ImGui wrapper cannot consume.
#[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
fn validate_freetype_face(data: &[u8], index: u32) -> Result<(), FontError> {
    use ttf_parser::{RawFace, fonts_in_collection};

    // FreeType uses higher bits in the face index for variable-font named instances.
    u16::try_from(index).map_err(|_| FontError::InvalidFontIndex { index })?;
    if data.starts_with(b"ttcf") {
        if !matches!(data.get(4..8), Some(b"\0\x01\0\0" | b"\0\x02\0\0")) {
            return Err(FontError::InvalidSfnt);
        }
        let face_count = fonts_in_collection(data).ok_or(FontError::InvalidSfnt)?;
        if index >= face_count {
            return Err(FontError::InvalidFontIndex { index });
        }
        // Bound the offset array before RawFace multiplies the untrusted face count.
        if face_count as usize > (data.len() - 12) / 4 {
            return Err(FontError::InvalidSfnt);
        }
    } else if index != 0 {
        return Err(FontError::InvalidFontIndex { index });
    }
    let face = RawFace::parse(data, index).map_err(|_| FontError::InvalidSfnt)?;
    for record in face.table_records {
        let format = match &record.tag.to_bytes() {
            b"SVG " => "OpenType SVG",
            b"EBLC" | b"EBDT" | b"CBLC" | b"CBDT" | b"bloc" | b"bdat" | b"sbix" => {
                "embedded bitmap"
            }
            _ => continue,
        };
        return Err(FontError::UnsupportedFreeTypeGlyphs { format });
    }
    Ok(())
}
