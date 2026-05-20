use std::ptr;

use crate::fonts::Font;
use crate::fonts::atlas::config::FontConfig;
use crate::fonts::atlas::source::FontSource;
use crate::fonts::atlas::validation::{
    validate_font_size_pixels, validate_font_size_pixels_option,
};
use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Add a font to the atlas using FontSource
    #[doc(alias = "AddFont")]
    pub fn add_font(&mut self, font_sources: &[FontSource<'_>]) -> crate::fonts::FontId {
        let Some((head, tail)) = font_sources.split_first() else {
            panic!("FontAtlas::add_font requires at least one FontSource");
        };
        let font_id = self.add_font_internal(head, false);
        for font in tail {
            self.add_font_internal(font, true);
        }
        font_id
    }

    fn add_font_internal(
        &mut self,
        font_source: &FontSource<'_>,
        merge_mode: bool,
    ) -> crate::fonts::FontId {
        match font_source {
            FontSource::DefaultFontData {
                size_pixels,
                config,
            } => {
                // For v1.92+, we can use dynamic sizing by passing 0.0
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self.add_font_default(Some(&cfg)).raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::TtfData {
                data,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_memory_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add TTF font from memory")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::CompressedTtfData {
                data,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_memory_compressed_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add compressed TTF font from memory")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::CompressedTtfBase85 {
                data,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_memory_compressed_base85_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add base85 compressed TTF font from memory")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
            FontSource::TtfFile {
                path,
                size_pixels,
                config,
            } => {
                let size = validate_font_size_pixels_option(
                    "FontAtlas::add_font()",
                    "size_pixels",
                    *size_pixels,
                );
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                if merge_mode {
                    cfg = cfg.merge_mode(true);
                }
                let font_ptr = self
                    .add_font_from_file_ttf(path, size, Some(&cfg), None)
                    .expect("Failed to add TTF font from file")
                    .raw();
                self.font_id_for_raw(font_ptr)
            }
        }
    }

    /// Add a font to the atlas using FontConfig
    #[doc(alias = "AddFont")]
    pub fn add_font_with_config(&mut self, font_cfg: &FontConfig) -> &mut Font {
        font_cfg.validate_for_add_font("FontAtlas::add_font_with_config()");
        unsafe {
            let font_ptr = sys::ImFontAtlas_AddFont(self.raw, font_cfg.raw());
            if font_cfg.raw.MergeMode {
                self.discard_bakes(0);
            }
            Font::from_raw_mut(font_ptr)
        }
    }

    /// Add the default font to the atlas
    #[doc(alias = "AddFontDefault")]
    pub fn add_font_default(&mut self, font_cfg: Option<&FontConfig>) -> &mut Font {
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_default("FontAtlas::add_font_default()");
        }
        unsafe {
            let cfg_ptr = font_cfg.map_or(ptr::null(), |cfg| cfg.raw());
            let font_ptr = sys::ImFontAtlas_AddFontDefault(self.raw, cfg_ptr);
            if let Some(cfg) = font_cfg {
                if cfg.raw.MergeMode {
                    self.discard_bakes(0);
                }
            }
            Font::from_raw_mut(font_ptr)
        }
    }

    /// Add a font from a TTF file
    #[doc(alias = "AddFontFromFileTTF")]
    pub fn add_font_from_file_ttf(
        &mut self,
        filename: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_file_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size("FontAtlas::add_font_from_file_ttf()", size_pixels);
        }
        unsafe {
            let filename_cstr = std::ffi::CString::new(filename).ok()?;
            let cfg_ptr = font_cfg.map_or(ptr::null(), |cfg| cfg.raw());
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromFileTTF(
                self.raw,
                filename_cstr.as_ptr(),
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                if let Some(cfg) = font_cfg {
                    if cfg.raw.MergeMode {
                        self.discard_bakes(0);
                    }
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Add a font from memory (TTF data)
    #[doc(alias = "AddFontFromMemoryTTF")]
    pub fn add_font_from_memory_ttf(
        &mut self,
        font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_memory_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size(
                "FontAtlas::add_font_from_memory_ttf()",
                size_pixels,
            );
        }
        // Dear ImGui asserts on suspiciously small buffers to catch common mistakes.
        // Mirror that behavior by returning `None` instead of panicking/aborting in debug builds.
        if font_data.len() <= 100 {
            return None;
        }
        let font_data_len = i32::try_from(font_data.len()).ok()?;
        unsafe {
            // SAFETY: `AddFontFromMemoryTTF()` stores the pointer for (potential) rebuilds and may
            // free it later depending on `FontDataOwnedByAtlas`. Never pass a pointer into
            // Rust-owned stack/Vec memory here.
            //
            // Allocate and copy the bytes using Dear ImGui's allocator, then let the atlas own it.
            // This avoids use-after-free, double-free, and leaking uninitialized padding bytes
            // across the C++ boundary.
            let mem = sys::igMemAlloc(font_data.len());
            if mem.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(font_data.as_ptr(), mem as *mut u8, font_data.len());

            let cfg = font_cfg
                .cloned()
                .unwrap_or_default()
                .font_data_owned_by_atlas(true);
            let is_merge = cfg.raw.MergeMode;
            let cfg_ptr = cfg.raw();
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw,
                mem,
                font_data_len,
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                sys::igMemFree(mem);
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Add a font from memory (compressed TTF data).
    ///
    /// Dear ImGui will decompress the data immediately and keep the decompressed buffer alive
    /// (owned by the atlas), so the `compressed_font_data` slice does not need to outlive this call.
    #[doc(alias = "AddFontFromMemoryCompressedTTF")]
    pub fn add_font_from_memory_compressed_ttf(
        &mut self,
        compressed_font_data: &[u8],
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_memory_compressed_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size(
                "FontAtlas::add_font_from_memory_compressed_ttf()",
                size_pixels,
            );
        }
        if compressed_font_data.is_empty() {
            return None;
        }
        let compressed_len = i32::try_from(compressed_font_data.len()).ok()?;

        unsafe {
            let cfg = font_cfg.cloned().unwrap_or_default();
            let is_merge = cfg.raw.MergeMode;
            let cfg_ptr = cfg.raw();
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedTTF(
                self.raw,
                compressed_font_data.as_ptr() as *const std::os::raw::c_void,
                compressed_len,
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }

    /// Add a font from memory (compressed + base85-encoded TTF data).
    ///
    /// The input string must be NUL-terminated for Dear ImGui; this wrapper allocates a `CString`
    /// and passes it to the backend.
    #[doc(alias = "AddFontFromMemoryCompressedBase85TTF")]
    pub fn add_font_from_memory_compressed_base85_ttf(
        &mut self,
        compressed_font_data_base85: &str,
        size_pixels: f32,
        font_cfg: Option<&FontConfig>,
        glyph_ranges: Option<&[sys::ImWchar]>,
    ) -> Option<&mut Font> {
        validate_font_size_pixels(
            "FontAtlas::add_font_from_memory_compressed_base85_ttf()",
            "size_pixels",
            size_pixels,
        );
        if let Some(cfg) = font_cfg {
            cfg.validate_for_add_font_with_size(
                "FontAtlas::add_font_from_memory_compressed_base85_ttf()",
                size_pixels,
            );
        }
        if compressed_font_data_base85.is_empty() {
            return None;
        }
        let base85 = std::ffi::CString::new(compressed_font_data_base85).ok()?;

        unsafe {
            let cfg = font_cfg.cloned().unwrap_or_default();
            let is_merge = cfg.raw.MergeMode;
            let cfg_ptr = cfg.raw();
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryCompressedBase85TTF(
                self.raw,
                base85.as_ptr(),
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                if is_merge {
                    self.discard_bakes(0);
                }
                Some(Font::from_raw_mut(font_ptr))
            }
        }
    }
}
