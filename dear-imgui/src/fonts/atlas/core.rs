use std::marker::PhantomData;
use std::ptr;

use crate::fonts::Font;
use crate::sys;

use super::config::FontConfig;
use super::id::{FontId, validate_font_for_atlas};
use super::loader::{FontLoader, FontLoaderFlags};
use super::source::FontSource;
use super::state::{bump_font_atlas_generation, font_atlas_state, forget_font_atlas_generation};
use super::validation::{
    frame_count_to_i32, validate_font_size_pixels, validate_font_size_pixels_option,
};

/// Font atlas that manages multiple fonts and their texture data
///
/// The font atlas is responsible for:
/// - Loading and managing multiple fonts
/// - Packing font glyphs into texture atlases
/// - Providing texture data for rendering
#[derive(Debug)]
pub struct FontAtlas {
    raw: *mut sys::ImFontAtlas,
    owned: bool,
    _phantom: PhantomData<*mut sys::ImFontAtlas>,
}

/// Shared view of a font atlas.
///
/// This type allows read-only atlas inspection without exposing safe font mutation from
/// `Context::font_atlas()`.
#[derive(Debug, Clone, Copy)]
pub struct FontAtlasRef<'atlas> {
    raw: *const sys::ImFontAtlas,
    _phantom: PhantomData<&'atlas sys::ImFontAtlas>,
}

impl<'atlas> FontAtlasRef<'atlas> {
    pub(crate) unsafe fn from_raw(raw: *const sys::ImFontAtlas) -> Self {
        assert!(
            !raw.is_null(),
            "FontAtlasRef::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw.cast_mut());
        Self {
            raw,
            _phantom: PhantomData,
        }
    }

    /// Returns the raw ImFontAtlas pointer.
    pub fn raw(&self) -> *const sys::ImFontAtlas {
        self.raw
    }

    /// Gets the current font loader flags.
    pub fn font_loader_flags(&self) -> FontLoaderFlags {
        unsafe { FontLoaderFlags((*self.raw).FontLoaderFlags) }
    }

    /// Check if the texture is built.
    pub fn is_built(&self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        unsafe { (*self.raw).TexIsBuilt }
    }

    /// Get texture data information.
    pub fn get_tex_data_info(&self) -> Option<(u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let min_width = (*self.raw).TexMinWidth as u32;
                let min_height = (*self.raw).TexMinHeight as u32;
                Some((min_width, min_height))
            } else {
                None
            }
        }
    }

    /// Get raw texture data pointer and dimensions.
    ///
    /// # Safety
    /// The returned pointer is only valid while the FontAtlas exists and the texture is built.
    /// The caller must ensure proper lifetime management.
    pub unsafe fn get_tex_data_ptr(&self) -> Option<(*const u8, u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let tex_data = (*self.raw).TexData;
                if !tex_data.is_null() {
                    let width = (*tex_data).Width as u32;
                    let height = (*tex_data).Height as u32;
                    let pixels = (*tex_data).Pixels;
                    if !pixels.is_null() {
                        Some((pixels, width, height))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    /// Get texture reference for the font atlas.
    pub fn get_tex_ref(&self) -> sys::ImTextureRef {
        unsafe { (*self.raw).TexRef }
    }

    /// Get texture data pointer.
    pub fn get_tex_data(&self) -> *mut sys::ImTextureData {
        unsafe { (*self.raw).TexData }
    }

    /// Get a shared view of the atlas texture data, if available.
    pub fn tex_data(&self) -> Option<&crate::texture::TextureData> {
        let ptr = unsafe { (*self.raw).TexData };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { crate::texture::TextureData::from_raw_ref(ptr) })
        }
    }

    /// Get texture UV scale.
    pub fn get_tex_uv_scale(&self) -> [f32; 2] {
        unsafe {
            let scale = (*self.raw).TexUvScale;
            [scale.x, scale.y]
        }
    }

    /// Get texture UV white pixel coordinates.
    pub fn get_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            let pixel = (*self.raw).TexUvWhitePixel;
            [pixel.x, pixel.y]
        }
    }
}

impl FontAtlas {
    /// Creates a new font atlas with default settings
    pub fn new() -> Self {
        unsafe {
            let raw = sys::ImFontAtlas_ImFontAtlas();
            if raw.is_null() {
                panic!("ImFontAtlas_ImFontAtlas() returned null");
            }
            font_atlas_state(raw);
            Self {
                raw,
                owned: true,
                _phantom: PhantomData,
            }
        }
    }

    /// Creates a new font atlas with a custom font loader.
    ///
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*`.
    pub fn with_font_loader(loader: &'static FontLoader) -> Self {
        let mut atlas = Self::new();
        atlas.set_font_loader(loader);
        atlas
    }

    /// Creates a FontAtlas wrapper from a raw ImFontAtlas pointer
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and points to a valid ImFontAtlas
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImFontAtlas) -> Self {
        assert!(
            !raw.is_null(),
            "FontAtlas::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw);
        Self {
            raw,
            owned: false,
            _phantom: PhantomData,
        }
    }

    /// Returns the raw ImFontAtlas pointer
    pub fn raw(&self) -> *mut sys::ImFontAtlas {
        self.raw
    }

    pub(crate) fn font_id_for_raw(&self, font: *mut sys::ImFont) -> FontId {
        FontId::from_raw_parts(font, self.raw)
    }

    /// Sets the font loader for this atlas.
    ///
    /// This allows using custom font backends like FreeType with additional features.
    /// Must be called before adding any fonts.
    /// The loader must be static because Dear ImGui stores the raw `ImFontLoader*`.
    pub fn set_font_loader(&mut self, loader: &'static FontLoader) {
        unsafe {
            sys::ImFontAtlas_SetFontLoader(self.raw, loader.as_ptr());
        }
    }

    // Note: switching to the FreeType loader at runtime requires access to the
    // C++ symbol ImGuiFreeType_GetFontLoader(), which may not be available in
    // prebuilt dear-imgui-sys distributions. If needed, prefer configuring the
    // loader from the sys layer or ensure the symbol is exported, then add a
    // thin wrapper here.

    /// Sets global font loader flags
    ///
    /// These flags apply to all fonts loaded with this atlas unless overridden
    /// in individual FontConfig instances.
    pub fn set_font_loader_flags(&mut self, flags: FontLoaderFlags) {
        unsafe {
            (*self.raw).FontLoaderFlags = flags.0;
        }
    }

    /// Gets the current font loader flags
    pub fn font_loader_flags(&self) -> FontLoaderFlags {
        unsafe { FontLoaderFlags((*self.raw).FontLoaderFlags) }
    }

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

    /// Remove a font from the atlas.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "RemoveFont")]
    pub fn remove_font(&mut self, font: &mut Font) {
        let font = validate_font_for_atlas(font, self.raw, "FontAtlas::remove_font()");
        unsafe { sys::ImFontAtlas_RemoveFont(self.raw, font) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear all fonts and texture data.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "Clear")]
    pub fn clear(&mut self) {
        unsafe { sys::ImFontAtlas_Clear(self.raw) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear only the fonts (keep texture data).
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&mut self) {
        unsafe { sys::ImFontAtlas_ClearFonts(self.raw) }
        bump_font_atlas_generation(self.raw);
    }

    /// Clear only the texture data (keep fonts)
    #[doc(alias = "ClearTexData")]
    pub fn clear_tex_data(&mut self) {
        unsafe { sys::ImFontAtlas_ClearTexData(self.raw) }
    }

    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default_ptr(&self) -> *const sys::ImWchar {
        if self.raw.is_null() {
            return std::ptr::null();
        }
        unsafe { sys::ImFontAtlas_GetGlyphRangesDefault(self.raw) }
    }

    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    ///
    /// The returned slice is terminated by a `0` sentinel, matching Dear ImGui's
    /// `ImWchar` range list format: `[start, end, start, end, ..., 0]`.
    ///
    /// Prefer [`Self::get_glyph_ranges_default_ptr`] when passing glyph ranges
    /// back into FFI, to avoid any scanning on the Rust side.
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default(&self) -> &[sys::ImWchar] {
        unsafe {
            let ptr = self.get_glyph_ranges_default_ptr();
            if ptr.is_null() {
                &[]
            } else {
                // Count the ranges (terminated by 0). Dear ImGui stores the list as
                // pairs: [start, end, start, end, ..., 0].
                //
                // This assumes the pointer points to a valid, null-terminated
                // static array as provided by Dear ImGui.
                const MAX_WORDS: usize = 2048;
                let mut i = 0usize;
                while i < MAX_WORDS {
                    if *ptr.add(i) == 0 {
                        return std::slice::from_raw_parts(ptr, i + 1);
                    }
                    i = i.saturating_add(2);
                }

                debug_assert!(
                    false,
                    "ImFontAtlas_GetGlyphRangesDefault() did not terminate within MAX_WORDS"
                );
                &[]
            }
        }
    }

    /// Build the font atlas texture
    ///
    /// This is a simplified build process. For more control, use the individual build functions.
    ///
    /// Note: with Dear ImGui 1.92+ "new backend" texture system, you should generally
    /// not call `build()` manually. The renderer should set `ImGuiBackendFlags_RendererHasTextures`
    /// and the atlas will be built/updated on demand.
    ///
    /// In particular, calling `build()` before the renderer sets `RendererHasTextures`
    /// may cause Dear ImGui to assert on the next frame.
    #[doc(alias = "Build")]
    pub fn build(&mut self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        // NOTE: In Dear ImGui, `ImFontAtlasBuildMain()` will call `ImFontAtlasBuildInit()`
        // lazily if needed (Builder == NULL). Calling BuildInit unconditionally would leak
        // the builder and is not idempotent.
        unsafe {
            sys::igImFontAtlasBuildMain(self.raw);
            (*self.raw).TexIsBuilt
        }
    }

    /// Discard baked font caches.
    ///
    /// This clears cached glyph data (including cached "not found" entries) so that
    /// newly added font sources (e.g. merged CJK/emoji fonts) can take effect.
    ///
    /// Pass `unused_frames = 0` to discard everything (recommended after font merging).
    ///
    /// Notes:
    /// - Only call this when the atlas is not locked (typically before `Context::frame()`).
    /// - No-op if the atlas builder hasn't been created yet.
    #[doc(alias = "ImFontAtlasBuildDiscardBakes")]
    pub fn discard_bakes(&mut self, unused_frames: usize) {
        if self.raw.is_null() {
            return;
        }
        let unused_frames =
            frame_count_to_i32("FontAtlas::discard_bakes()", "unused_frames", unused_frames);
        unsafe {
            if (*self.raw).Builder.is_null() {
                return;
            }
            sys::igImFontAtlasBuildDiscardBakes(self.raw, unused_frames);
        }
    }

    /// Check if the texture is built
    pub fn is_built(&self) -> bool {
        if self.raw.is_null() {
            return false;
        }
        unsafe { (*self.raw).TexIsBuilt }
    }

    /// Get texture data information
    ///
    /// Returns (min_width, min_height) if texture is built
    /// Note: Our Dear ImGui version uses a different texture management system
    pub fn get_tex_data_info(&self) -> Option<(u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let min_width = (*self.raw).TexMinWidth as u32;
                let min_height = (*self.raw).TexMinHeight as u32;
                Some((min_width, min_height))
            } else {
                None
            }
        }
    }

    /// Get raw texture data pointer and dimensions
    ///
    /// # Safety
    /// The returned pointer is only valid while the FontAtlas exists and the texture is built.
    /// The caller must ensure proper lifetime management.
    pub unsafe fn get_tex_data_ptr(&self) -> Option<(*const u8, u32, u32)> {
        if self.raw.is_null() {
            return None;
        }
        unsafe {
            if (*self.raw).TexIsBuilt {
                let tex_data = (*self.raw).TexData;
                if !tex_data.is_null() {
                    let width = (*tex_data).Width as u32;
                    let height = (*tex_data).Height as u32;
                    let pixels = (*tex_data).Pixels;
                    if !pixels.is_null() {
                        Some((pixels, width, height))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    /// Get texture reference for the font atlas
    ///
    /// Note: Our Dear ImGui version uses ImTextureRef instead of a simple texture ID
    pub fn get_tex_ref(&self) -> sys::ImTextureRef {
        unsafe { (*self.raw).TexRef }
    }

    /// Set texture reference for the font atlas
    pub fn set_tex_ref(&mut self, tex_ref: sys::ImTextureRef) {
        unsafe {
            (*self.raw).TexRef = tex_ref;
        }
    }

    /// Get a mutable view of the atlas texture data, if available
    pub fn tex_data_mut(&mut self) -> Option<&mut crate::texture::TextureData> {
        let ptr = unsafe { (*self.raw).TexData };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { crate::texture::TextureData::from_raw(ptr) })
        }
    }

    /// Convenience: set atlas texture id and mark status OK
    /// Also updates TexRef so draw commands continue to follow the managed
    /// `ImTextureData` when one is available.
    pub fn set_texture_id(&mut self, tex_id: crate::texture::TextureId) {
        let tex_ref = if let Some(td) = self.tex_data_mut() {
            td.set_tex_id(tex_id);
            td.set_status(crate::texture::TextureStatus::OK);
            td.texture_ref().raw()
        } else {
            sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: tex_id.id() as sys::ImTextureID,
            }
        };

        self.set_tex_ref(tex_ref);
    }

    /// Get texture data pointer
    ///
    /// Returns the current texture data used by the atlas
    pub fn get_tex_data(&self) -> *mut sys::ImTextureData {
        unsafe { (*self.raw).TexData }
    }

    /// Get texture UV scale
    pub fn get_tex_uv_scale(&self) -> [f32; 2] {
        unsafe {
            let scale = (*self.raw).TexUvScale;
            [scale.x, scale.y]
        }
    }

    /// Get texture UV white pixel coordinates
    pub fn get_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            let pixel = (*self.raw).TexUvWhitePixel;
            [pixel.x, pixel.y]
        }
    }
}

impl Default for FontAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FontAtlas {
    fn drop(&mut self) {
        if self.owned && !self.raw.is_null() {
            unsafe {
                forget_font_atlas_generation(self.raw);
                sys::ImFontAtlas_destroy(self.raw);
            }
        }
    }
}

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.
