use std::ffi::CString;
use std::ptr;

use crate::fonts::atlas::config::FontConfig;
use crate::fonts::atlas::loader::FontLoader;
use crate::fonts::atlas::source::{FontSource, FontSourceKind, assert_stb_truetype_config};
use crate::fonts::atlas::state::font_atlas_contains_font;
use crate::fonts::atlas::validation::{
    validate_font_size_pixels, validate_font_size_pixels_option,
};
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Adds one font, optionally assembled from multiple merged sources.
    ///
    /// The first source honors its [`FontConfig::merge_mode`] value. Every
    /// later source is merged, regardless of its configured value.
    #[doc(alias = "AddFont")]
    pub fn add_font(&self, font_sources: &[FontSource<'_>]) -> crate::fonts::FontId {
        const CALLER: &str = "FontAtlas::add_font()";
        self.assert_mutation_allowed(CALLER);
        let Some((head, tail)) = font_sources.split_first() else {
            panic!("{CALLER} requires at least one FontSource");
        };

        for (index, source) in font_sources.iter().enumerate() {
            Self::validate_font_source(source, index != 0);
        }
        self.validate_merged_sources(head, tail);

        let font_id = self.add_font_internal(head, false);
        for source in tail {
            self.add_font_internal(source, true);
        }
        font_id
    }

    fn validate_font_source(font_source: &FontSource<'_>, merge_mode: bool) {
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
                validate_ttf_data_length(CALLER, data.as_bytes());
            }
            FontSourceKind::TtfData(data) => {
                config.validate_for_add_font_with_size(CALLER, size);
                validate_ttf_data_length(CALLER, data);
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
        if !matches!(
            &source.kind,
            FontSourceKind::Default | FontSourceKind::DefaultVector | FontSourceKind::DefaultBitmap
        ) {
            return false;
        }
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

        let destination = self.merge_destination(config, caller);
        unsafe {
            assert!(
                merge_input_size == 0.0
                    || ((*destination).Flags & sys::ImFontFlags_ImplicitRefSize) == 0,
                "{caller} cannot merge a source with an explicit reference size into a destination font that uses an implicit reference size"
            );
        }
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
        unsafe { ((*destination).Flags & sys::ImFontFlags_ImplicitRefSize) != 0 }
    }

    fn add_font_internal(
        &self,
        font_source: &FontSource<'_>,
        merge_mode: bool,
    ) -> crate::fonts::FontId {
        let size = validate_font_size_pixels_option(
            "FontAtlas::add_font()",
            "size_pixels",
            font_source.size_pixels,
        );
        let mut config = configured_font_source(font_source.config.as_ref(), size, merge_mode);
        if matches!(&font_source.kind, FontSourceKind::StbTrueType(_)) {
            assert_stb_truetype_config(&config, "FontAtlas::add_font()");
            config.raw.FontLoader = FontLoader::stb_truetype().as_ptr();
            config.raw.FontNo = 0;
        }

        match &font_source.kind {
            FontSourceKind::Default => self.add_font_default_internal(Some(&config)),
            FontSourceKind::DefaultVector => self.add_font_default_vector_internal(Some(&config)),
            FontSourceKind::DefaultBitmap => self.add_font_default_bitmap_internal(Some(&config)),
            FontSourceKind::StbTrueType(data) => unsafe {
                self.add_font_from_memory_ttf_internal(data.as_bytes(), size, Some(&config))
                    .expect("FontAtlas::add_font() failed to add validated stb_truetype data")
            },
            FontSourceKind::TtfData(data) => unsafe {
                self.add_font_from_memory_ttf_internal(data, size, Some(&config))
                    .expect("FontAtlas::add_font() failed to add TTF font data")
            },
            FontSourceKind::CompressedTtfData(data) => unsafe {
                self.add_font_from_memory_compressed_ttf_internal(data, size, Some(&config))
                    .expect("FontAtlas::add_font() failed to add compressed TTF font data")
            },
            FontSourceKind::CompressedTtfBase85(data) => unsafe {
                self.add_font_from_memory_compressed_base85_ttf_internal(data, size, Some(&config))
                    .expect("FontAtlas::add_font() failed to add base85-compressed TTF font data")
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
    /// `font_data` must contain a complete font that is valid for the selected
    /// loader. Dear ImGui's native font parsers may otherwise read past the
    /// allocated buffer. The slice itself does not need to outlive this call.
    #[doc(alias = "AddFontFromMemoryTTF")]
    unsafe fn add_font_from_memory_ttf_internal(
        &self,
        font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font()";
        self.assert_mutation_allowed(CALLER);
        validate_font_size_pixels(CALLER, "size_pixels", size_pixels);
        if let Some(config) = font_cfg {
            config.validate_for_add_font_with_size(CALLER, size_pixels);
        }
        let font_data_len = validate_ttf_data_length_fallible(font_data)?;

        let config = font_cfg.cloned().unwrap_or_default();
        let effective_size = effective_font_size(&config, size_pixels);
        self.validate_merge_target(&config.raw, effective_size, CALLER);
        unsafe {
            let memory = sys::igMemAlloc(font_data.len());
            if memory.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(font_data.as_ptr(), memory.cast::<u8>(), font_data.len());

            let mut raw_config = Self::font_config_for_ffi(&config);
            raw_config.FontDataOwnedByAtlas = true;
            let is_merge = raw_config.MergeMode;

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw(),
                memory,
                font_data_len,
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

fn validate_ttf_data_length(caller: &str, data: &[u8]) {
    assert!(
        validate_ttf_data_length_fallible(data).is_some(),
        "{caller} TTF/OTF data must contain more than 100 bytes and fit Dear ImGui's i32 length"
    );
}

fn validate_ttf_data_length_fallible(data: &[u8]) -> Option<i32> {
    if data.len() <= 100 {
        return None;
    }
    i32::try_from(data.len()).ok()
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
