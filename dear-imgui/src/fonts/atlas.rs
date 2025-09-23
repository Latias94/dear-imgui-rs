//! Font atlas management for Dear ImGui v1.92+
//!
//! This module provides a modern, type-safe interface to Dear ImGui's dynamic font system.
//! Key features:
//! - Dynamic glyph loading (no need to pre-specify glyph ranges)
//! - Runtime font size adjustment
//! - Custom font loaders
//! - Incremental texture updates

use crate::fonts::Font;
use crate::sys;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;

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

/// A font identifier
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FontId(pub(crate) *const Font);

/// Font loader interface for custom font backends
///
/// This provides a safe Rust interface to Dear ImGui's ImFontLoader system,
/// allowing custom font loading implementations.
pub struct FontLoader {
    raw: sys::ImFontLoader,
    name: CString,
}

impl FontLoader {
    /// Creates a new font loader with the given name
    pub fn new(name: &str) -> Result<Self, std::ffi::NulError> {
        let name_cstring = CString::new(name)?;
        let mut raw = sys::ImFontLoader::default();
        raw.Name = name_cstring.as_ptr();

        Ok(Self {
            raw,
            name: name_cstring,
        })
    }

    /// Returns a pointer to the raw ImFontLoader
    pub(crate) fn as_ptr(&self) -> *const sys::ImFontLoader {
        &self.raw
    }

    /// Sets the loader initialization callback
    pub fn with_loader_init<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut FontAtlas) -> bool + 'static,
    {
        // Note: For now, we'll use the default STB TrueType loader
        // Custom callbacks would require more complex lifetime management
        self
    }
}

/// Font loader flags for controlling font loading behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLoaderFlags(pub u32);

impl FontLoaderFlags {
    /// No special flags
    pub const NONE: Self = Self(0);

    /// Load color glyphs (requires FreeType backend)
    pub const LOAD_COLOR: Self = Self(1 << 0);

    /// Force auto-hinting
    pub const FORCE_AUTOHINT: Self = Self(1 << 1);

    /// Disable hinting
    pub const NO_HINTING: Self = Self(1 << 2);

    /// Disable auto-hinting
    pub const NO_AUTOHINT: Self = Self(1 << 3);
}

impl std::ops::BitOr for FontLoaderFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for FontLoaderFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A shared font atlas that can be used across multiple contexts
///
/// This allows multiple ImGui contexts to share the same font atlas,
/// which is useful for applications with multiple windows or contexts.
#[derive(Debug, Clone)]
pub struct SharedFontAtlas(pub(crate) Rc<*mut sys::ImFontAtlas>);

impl SharedFontAtlas {
    /// Creates a new shared font atlas
    pub fn create() -> SharedFontAtlas {
        unsafe {
            let raw_atlas = sys::ImFontAtlas_ImFontAtlas();
            SharedFontAtlas(Rc::new(raw_atlas))
        }
    }

    /// Returns a raw pointer to the underlying ImFontAtlas
    pub(crate) fn as_ptr(&self) -> *const sys::ImFontAtlas {
        *self.0
    }

    /// Returns a mutable raw pointer to the underlying ImFontAtlas
    pub(crate) fn as_ptr_mut(&mut self) -> *mut sys::ImFontAtlas {
        *self.0
    }
}

impl Drop for SharedFontAtlas {
    fn drop(&mut self) {
        // Only drop if this is the last reference
        if Rc::strong_count(&self.0) == 1 {
            unsafe {
                let atlas_ptr = *self.0;
                if !atlas_ptr.is_null() {
                    sys::ImFontAtlas_destroy(atlas_ptr);
                }
            }
        }
    }
}

impl FontAtlas {
    /// Creates a new font atlas with default settings
    pub fn new() -> Self {
        unsafe {
            let raw = sys::ImFontAtlas_ImFontAtlas();
            Self {
                raw,
                owned: true,
                _phantom: PhantomData,
            }
        }
    }

    /// Creates a new font atlas with a custom font loader
    pub fn with_font_loader(loader: &FontLoader) -> Self {
        let mut atlas = Self::new();
        atlas.set_font_loader(loader);
        atlas
    }

    /// Creates a FontAtlas wrapper from a raw ImFontAtlas pointer
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and points to a valid ImFontAtlas
    pub unsafe fn from_raw(raw: *mut sys::ImFontAtlas) -> Self {
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

    /// Sets the font loader for this atlas
    ///
    /// This allows using custom font backends like FreeType with additional features.
    /// Must be called before adding any fonts.
    pub fn set_font_loader(&mut self, loader: &FontLoader) {
        unsafe {
            sys::ImFontAtlas_SetFontLoader(self.raw, loader.as_ptr());
        }
    }

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
        let (head, tail) = font_sources.split_first().unwrap();
        let font_id = self.add_font_internal(head, false);
        for font in tail {
            self.add_font_internal(font, true);
        }
        font_id
    }

    fn add_font_internal(
        &mut self,
        font_source: &FontSource<'_>,
        _merge_mode: bool,
    ) -> crate::fonts::FontId {
        match font_source {
            FontSource::DefaultFontData {
                size_pixels,
                config,
            } => {
                // For v1.92+, we can use dynamic sizing by passing 0.0
                let size = size_pixels.unwrap_or(0.0);
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                let font = self.add_font_default(Some(&cfg));
                font.id()
            }
            FontSource::TtfData {
                data,
                size_pixels,
                config,
            } => {
                let size = size_pixels.unwrap_or(0.0);
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                let font = self
                    .add_font_from_memory_ttf(data, size, Some(&cfg), None)
                    .expect("Failed to add TTF font from memory");
                font.id()
            }
            FontSource::TtfFile {
                path,
                size_pixels,
                config,
            } => {
                let size = size_pixels.unwrap_or(0.0);
                let mut cfg = config.clone().unwrap_or_default();
                if size > 0.0 {
                    cfg = cfg.size_pixels(size);
                }
                let font = self
                    .add_font_from_file_ttf(path, size, Some(&cfg), None)
                    .expect("Failed to add TTF font from file");
                font.id()
            }
        }
    }

    /// Add a font to the atlas using FontConfig
    #[doc(alias = "AddFont")]
    pub fn add_font_with_config(&mut self, font_cfg: &FontConfig) -> &mut Font {
        unsafe {
            let font_ptr = sys::ImFontAtlas_AddFont(self.raw, font_cfg.raw());
            &mut *(font_ptr as *mut Font)
        }
    }

    /// Add the default font to the atlas
    #[doc(alias = "AddFontDefault")]
    pub fn add_font_default(&mut self, font_cfg: Option<&FontConfig>) -> &mut Font {
        unsafe {
            let cfg_ptr = font_cfg.map_or(ptr::null(), |cfg| cfg.raw());
            let font_ptr = sys::ImFontAtlas_AddFontDefault(self.raw, cfg_ptr);
            &mut *(font_ptr as *mut Font)
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
                Some(&mut *(font_ptr as *mut Font))
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
        unsafe {
            let cfg_ptr = font_cfg.map_or(ptr::null(), |cfg| cfg.raw());
            let ranges_ptr = glyph_ranges.map_or(ptr::null(), |ranges| ranges.as_ptr());

            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw,
                font_data.as_ptr() as *mut std::os::raw::c_void,
                font_data.len() as i32,
                size_pixels,
                cfg_ptr,
                ranges_ptr,
            );

            if font_ptr.is_null() {
                None
            } else {
                Some(&mut *(font_ptr as *mut Font))
            }
        }
    }

    /// Remove a font from the atlas
    #[doc(alias = "RemoveFont")]
    pub fn remove_font(&mut self, font: &mut Font) {
        unsafe { sys::ImFontAtlas_RemoveFont(self.raw, font.raw()) }
    }

    /// Clear all fonts and texture data
    #[doc(alias = "Clear")]
    pub fn clear(&mut self) {
        unsafe { sys::ImFontAtlas_Clear(self.raw) }
    }

    /// Clear only the fonts (keep texture data)
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&mut self) {
        unsafe { sys::ImFontAtlas_ClearFonts(self.raw) }
    }

    /// Clear only the texture data (keep fonts)
    #[doc(alias = "ClearTexData")]
    pub fn clear_tex_data(&mut self) {
        unsafe { sys::ImFontAtlas_ClearTexData(self.raw) }
    }

    /// Get default glyph ranges (Basic Latin + Latin Supplement)
    #[doc(alias = "GetGlyphRangesDefault")]
    pub fn get_glyph_ranges_default(&self) -> &[sys::ImWchar] {
        unsafe {
            let ptr = sys::ImFontAtlas_GetGlyphRangesDefault(self.raw);
            if ptr.is_null() {
                &[]
            } else {
                // Count the ranges (terminated by 0)
                let mut len = 0;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                std::slice::from_raw_parts(ptr, len)
            }
        }
    }

    /// Build the font atlas texture
    ///
    /// This is a simplified build process. For more control, use the individual build functions.
    #[doc(alias = "Build")]
    pub fn build(&mut self) -> bool {
        unsafe {
            // Initialize the build process
            sys::igImFontAtlasBuildInit(self.raw);

            // Perform the main build
            sys::igImFontAtlasBuildMain(self.raw);

            // Update pointers
            sys::igImFontAtlasBuildUpdatePointers(self.raw);

            // Check if build was successful
            (*self.raw).TexIsBuilt
        }
    }

    /// Check if the texture is built
    pub fn is_built(&self) -> bool {
        unsafe { (*self.raw).TexIsBuilt }
    }

    /// Get texture data information
    ///
    /// Returns (min_width, min_height) if texture is built
    /// Note: Our Dear ImGui version uses a different texture management system
    pub fn get_tex_data_info(&self) -> Option<(u32, u32)> {
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
                sys::ImFontAtlas_destroy(self.raw);
            }
        }
    }
}

// FontAtlas is safe to send between threads as long as the ImGui context is not being used
unsafe impl Send for FontAtlas {}
// FontAtlas is safe to share between threads as long as access is synchronized
unsafe impl Sync for FontAtlas {}

/// Font configuration for loading fonts with v1.92+ features
#[derive(Debug, Clone)]
pub struct FontConfig {
    raw: sys::ImFontConfig,
}

impl FontConfig {
    /// Creates a new font configuration with default settings
    pub fn new() -> Self {
        Self {
            raw: Default::default(),
        }
    }

    /// Returns the raw ImFontConfig pointer
    pub(crate) fn raw(&self) -> *const sys::ImFontConfig {
        &self.raw
    }

    /// Set the font size in pixels
    ///
    /// Note: With v1.92+ dynamic fonts, size can be 0.0 to use default sizing
    pub fn size_pixels(mut self, size: f32) -> Self {
        self.raw.SizePixels = size;
        self
    }

    /// Set whether to merge this font with the previous one
    pub fn merge_mode(mut self, merge: bool) -> Self {
        self.raw.MergeMode = merge;
        self
    }

    /// Set font loader flags for this specific font
    ///
    /// These flags override the global atlas flags for this font.
    pub fn font_loader_flags(mut self, flags: FontLoaderFlags) -> Self {
        self.raw.FontLoaderFlags = flags.0;
        self
    }

    /// Set glyph ranges to exclude from this font
    ///
    /// Useful when merging fonts to avoid overlapping glyphs.
    pub fn glyph_exclude_ranges(mut self, ranges: &[u32]) -> Self {
        self.raw.GlyphExcludeRanges = ranges.as_ptr() as *const sys::ImWchar;
        self
    }

    /// Set a custom font loader for this font
    pub fn font_loader(mut self, loader: &FontLoader) -> Self {
        self.raw.FontLoader = loader.as_ptr();
        self
    }

    /// Set the font name for debugging
    pub fn name(mut self, name: &str) -> Self {
        let name_bytes = name.as_bytes();
        let copy_len = std::cmp::min(name_bytes.len(), self.raw.Name.len() - 1);

        // Clear the array first
        for i in 0..self.raw.Name.len() {
            self.raw.Name[i] = 0;
        }

        // Copy the name
        for (i, &byte) in name_bytes.iter().take(copy_len).enumerate() {
            self.raw.Name[i] = byte as i8;
        }

        self
    }

    /// Set glyph offset for this font
    pub fn glyph_offset(mut self, offset: [f32; 2]) -> Self {
        self.raw.GlyphOffset.x = offset[0];
        self.raw.GlyphOffset.y = offset[1];
        self
    }

    /// Set minimum advance X for glyphs
    pub fn glyph_min_advance_x(mut self, advance: f32) -> Self {
        self.raw.GlyphMinAdvanceX = advance;
        self
    }

    /// Set maximum advance X for glyphs
    pub fn glyph_max_advance_x(mut self, advance: f32) -> Self {
        self.raw.GlyphMaxAdvanceX = advance;
        self
    }

    /// Set extra advance X for glyphs (spacing between characters)
    pub fn glyph_extra_advance_x(mut self, advance: f32) -> Self {
        self.raw.GlyphExtraAdvanceX = advance;
        self
    }

    /// Set rasterizer multiply factor
    pub fn rasterizer_multiply(mut self, multiply: f32) -> Self {
        self.raw.RasterizerMultiply = multiply;
        self
    }

    /// Set rasterizer density for DPI scaling
    pub fn rasterizer_density(mut self, density: f32) -> Self {
        self.raw.RasterizerDensity = density;
        self
    }

    /// Set pixel snap horizontally
    pub fn pixel_snap_h(mut self, snap: bool) -> Self {
        self.raw.PixelSnapH = snap;
        self
    }

    /// Set pixel snap vertically
    pub fn pixel_snap_v(mut self, snap: bool) -> Self {
        self.raw.PixelSnapV = snap;
        self
    }

    /// Set horizontal oversampling
    pub fn oversample_h(mut self, oversample: i8) -> Self {
        self.raw.OversampleH = oversample;
        self
    }

    /// Set vertical oversampling
    pub fn oversample_v(mut self, oversample: i8) -> Self {
        self.raw.OversampleV = oversample;
        self
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self::new()
    }
}

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
            Self::TtfFile { config: cfg, .. } => *cfg = Some(config),
        }
        self
    }
}

/// Handle to a font atlas texture
#[derive(Clone, Debug)]
pub struct FontAtlasTexture<'a> {
    /// Texture width (in pixels)
    pub width: u32,
    /// Texture height (in pixels)
    pub height: u32,
    /// Raw texture data (in bytes).
    ///
    /// The format depends on which function was called to obtain this data:
    /// - For RGBA32: 4 bytes per pixel (R, G, B, A)
    /// - For Alpha8: 1 byte per pixel (Alpha only)
    pub data: &'a [u8],
}
