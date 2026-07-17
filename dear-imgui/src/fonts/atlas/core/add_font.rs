use std::ffi::CString;
use std::ptr;

use crate::fonts::atlas::config::FontConfig;
use crate::fonts::atlas::source::{FontSource, FontSourceKind};
use crate::fonts::atlas::state::{font_atlas_contains_font, store_font_atlas_glyph_ranges};
use crate::fonts::atlas::validation::{
    encode_glyph_ranges, validate_font_size_pixels, validate_font_size_pixels_option,
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

        let loaded_files: Vec<Option<Vec<u8>>> = font_sources
            .iter()
            .map(|source| Self::load_font_file(source, CALLER))
            .collect();

        let font_id = self.add_font_internal(head, false, loaded_files[0].as_deref());
        for (source, loaded_file) in tail.iter().zip(loaded_files[1..].iter()) {
            self.add_font_internal(source, true, loaded_file.as_deref());
        }
        font_id
    }

    fn validate_font_source(font_source: &FontSource<'_>, merge_mode: bool) {
        const CALLER: &str = "FontAtlas::add_font()";
        let size = validate_font_size_pixels_option(CALLER, "size_pixels", font_source.size_pixels);
        let config = configured_font_source(font_source.config.as_ref(), size, merge_mode);

        match font_source.kind {
            FontSourceKind::Default => config.validate_for_add_font_default(CALLER),
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
            FontSourceKind::TtfFile(_) => {
                config.validate_for_add_font_with_size(CALLER, size);
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
        if !matches!(source.kind, FontSourceKind::Default) {
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
        if matches!(source.kind, FontSourceKind::Default) && config.raw.SizePixels <= 0.0 {
            13.0
        } else {
            config.raw.SizePixels
        }
    }

    fn load_font_file(source: &FontSource<'_>, caller: &str) -> Option<Vec<u8>> {
        let FontSourceKind::TtfFile(path) = source.kind else {
            return None;
        };
        let data = std::fs::read(path)
            .unwrap_or_else(|error| panic!("{caller} failed to read font file {path:?}: {error}"));
        validate_ttf_data_length(caller, &data);
        Some(data)
    }

    fn store_glyph_ranges(
        &self,
        ranges: Option<&[(u32, u32)]>,
        caller: &str,
    ) -> *const sys::ImWchar {
        ranges.map_or(ptr::null(), |ranges| {
            store_font_atlas_glyph_ranges(self.raw(), encode_glyph_ranges(caller, ranges))
        })
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
        loaded_file: Option<&[u8]>,
    ) -> crate::fonts::FontId {
        let size = validate_font_size_pixels_option(
            "FontAtlas::add_font()",
            "size_pixels",
            font_source.size_pixels,
        );
        let config = configured_font_source(font_source.config.as_ref(), size, merge_mode);
        let config = match font_source.kind {
            FontSourceKind::TtfFile(path) => with_default_file_name(config, path),
            _ => config,
        };

        match font_source.kind {
            FontSourceKind::Default => self.add_font_default(Some(&config)),
            FontSourceKind::TtfData(data) => unsafe {
                self.add_font_from_memory_ttf(data, size, Some(&config), None)
                    .expect("FontAtlas::add_font() failed to add TTF font data")
            },
            FontSourceKind::CompressedTtfData(data) => unsafe {
                self.add_font_from_memory_compressed_ttf(data, size, Some(&config), None)
                    .expect("FontAtlas::add_font() failed to add compressed TTF font data")
            },
            FontSourceKind::CompressedTtfBase85(data) => unsafe {
                self.add_font_from_memory_compressed_base85_ttf(data, size, Some(&config), None)
                    .expect("FontAtlas::add_font() failed to add base85-compressed TTF font data")
            },
            FontSourceKind::TtfFile(path) => unsafe {
                self.add_font_from_memory_ttf(
                    loaded_file.unwrap_or_else(|| {
                        panic!("FontAtlas::add_font() did not preload font file {path:?}")
                    }),
                    size,
                    Some(&config),
                    None,
                )
                .expect("FontAtlas::add_font() failed to add preloaded TTF font data")
            },
        }
    }

    /// Adds a font using a fully configured native font source.
    #[doc(alias = "AddFont")]
    pub fn add_font_with_config(&self, font_cfg: &FontConfig) -> crate::fonts::FontId {
        const CALLER: &str = "FontAtlas::add_font_with_config()";
        self.assert_mutation_allowed(CALLER);
        font_cfg.validate_for_add_font(CALLER);
        self.validate_merge_target(&font_cfg.raw, font_cfg.raw.SizePixels, CALLER);

        unsafe {
            let raw_config = Self::font_config_for_ffi(font_cfg);
            let font_ptr = sys::ImFontAtlas_AddFont(self.raw(), &raw_config);
            assert!(!font_ptr.is_null(), "{CALLER} failed to add the font");
            if raw_config.MergeMode {
                self.discard_bakes(0);
            }
            self.font_id_for_raw(font_ptr)
        }
    }

    /// Adds Dear ImGui's embedded default font.
    #[doc(alias = "AddFontDefault")]
    pub fn add_font_default(&self, font_cfg: Option<&FontConfig>) -> crate::fonts::FontId {
        self.add_embedded_default_font(
            font_cfg,
            "FontAtlas::add_font_default()",
            sys::ImFontAtlas_AddFontDefault,
        )
    }

    /// Adds Dear ImGui's scalable embedded default font.
    #[doc(alias = "AddFontDefaultVector")]
    pub fn add_font_default_vector(&self, font_cfg: Option<&FontConfig>) -> crate::fonts::FontId {
        self.add_embedded_default_font(
            font_cfg,
            "FontAtlas::add_font_default_vector()",
            sys::ImFontAtlas_AddFontDefaultVector,
        )
    }

    /// Adds Dear ImGui's pixel-clean embedded default font.
    #[doc(alias = "AddFontDefaultBitmap")]
    pub fn add_font_default_bitmap(&self, font_cfg: Option<&FontConfig>) -> crate::fonts::FontId {
        self.add_embedded_default_font(
            font_cfg,
            "FontAtlas::add_font_default_bitmap()",
            sys::ImFontAtlas_AddFontDefaultBitmap,
        )
    }

    /// Adds a font from a TTF/OTF file.
    ///
    /// The file is read into Rust-owned memory before the atlas is modified. A
    /// missing or unreadable file returns `None` without calling native code.
    ///
    /// # Safety
    ///
    /// If the file exists, it must contain a complete font that is valid for the
    /// selected loader. Dear ImGui's native font parsers may otherwise read past
    /// the allocated buffer.
    #[doc(alias = "AddFontFromFileTTF")]
    pub unsafe fn add_font_from_file_ttf(
        &self,
        filename: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[(u32, u32)]>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font_from_file_ttf()";
        self.assert_mutation_allowed(CALLER);
        validate_font_size_pixels(CALLER, "size_pixels", size_pixels);
        if let Some(config) = font_cfg {
            config.validate_for_add_font_with_size(CALLER, size_pixels);
            let effective_size = effective_font_size(config, size_pixels);
            self.validate_merge_target(&config.raw, effective_size, CALLER);
        }

        let font_data = std::fs::read(filename).ok()?;
        validate_ttf_data_length_fallible(&font_data)?;
        let config = with_default_file_name(font_cfg.cloned().unwrap_or_default(), filename);
        unsafe {
            self.add_font_from_memory_ttf(&font_data, size_pixels, Some(&config), glyph_ranges)
        }
    }

    /// Adds a font from TTF/OTF data copied into Dear ImGui-owned memory.
    ///
    /// # Safety
    ///
    /// `font_data` must contain a complete font that is valid for the selected
    /// loader. Dear ImGui's native font parsers may otherwise read past the
    /// allocated buffer. The slice itself does not need to outlive this call.
    #[doc(alias = "AddFontFromMemoryTTF")]
    pub unsafe fn add_font_from_memory_ttf(
        &self,
        font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[(u32, u32)]>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font_from_memory_ttf()";
        self.assert_mutation_allowed(CALLER);
        validate_font_size_pixels(CALLER, "size_pixels", size_pixels);
        if let Some(config) = font_cfg {
            config.validate_for_add_font_with_size(CALLER, size_pixels);
        }
        let font_data_len = validate_ttf_data_length_fallible(font_data)?;

        let config = font_cfg.cloned().unwrap_or_default();
        let effective_size = effective_font_size(&config, size_pixels);
        self.validate_merge_target(&config.raw, effective_size, CALLER);
        let encoded_ranges = glyph_ranges.map(|ranges| encode_glyph_ranges(CALLER, ranges));

        unsafe {
            let memory = sys::igMemAlloc(font_data.len());
            if memory.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(font_data.as_ptr(), memory.cast::<u8>(), font_data.len());

            let mut raw_config = Self::font_config_for_ffi(&config);
            raw_config.FontDataOwnedByAtlas = true;
            let is_merge = raw_config.MergeMode;
            let ranges_ptr = encoded_ranges.map_or(ptr::null(), |ranges| {
                store_font_atlas_glyph_ranges(self.raw(), ranges)
            });

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw(),
                memory,
                font_data_len,
                size_pixels,
                &raw_config,
                ranges_ptr,
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
    pub unsafe fn add_font_from_memory_compressed_ttf(
        &self,
        compressed_font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[(u32, u32)]>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font_from_memory_compressed_ttf()";
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
            let ranges_ptr = self.store_glyph_ranges(glyph_ranges, CALLER);
            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedTTF(
                self.raw(),
                compressed_font_data.as_ptr().cast(),
                compressed_len,
                size_pixels,
                &raw_config,
                ranges_ptr,
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
    pub unsafe fn add_font_from_memory_compressed_base85_ttf(
        &self,
        compressed_font_data_base85: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[(u32, u32)]>,
    ) -> Option<crate::fonts::FontId> {
        const CALLER: &str = "FontAtlas::add_font_from_memory_compressed_base85_ttf()";
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
            let ranges_ptr = self.store_glyph_ranges(glyph_ranges, CALLER);
            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedBase85TTF(
                self.raw(),
                base85.as_ptr(),
                size_pixels,
                &raw_config,
                ranges_ptr,
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

fn with_default_file_name(mut config: FontConfig, filename: &str) -> FontConfig {
    if config.raw.Name[0] == 0 {
        let basename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
        config = config.name(basename);
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
