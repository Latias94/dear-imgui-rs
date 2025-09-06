//! Font atlas management
//!
//! This module provides the FontAtlas type which manages a collection of fonts
//! and their texture atlas for efficient rendering.

use crate::fonts::Font;
use crate::sys;
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
            // Create a new ImFontAtlas instance using the proper constructor
            let raw_atlas = Box::into_raw(Box::new(sys::ImFontAtlas::new()));
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
                    // Clean up the atlas
                    drop(Box::from_raw(atlas_ptr));
                }
            }
        }
    }
}

impl FontAtlas {
    /// Creates a new font atlas
    pub fn new() -> Self {
        unsafe {
            let raw = Box::into_raw(Box::new(sys::ImFontAtlas::new()));
            Self {
                raw,
                owned: true,
                _phantom: PhantomData,
            }
        }
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

    fn add_font_internal(&mut self, font_source: &FontSource<'_>, merge_mode: bool) -> crate::fonts::FontId {
        match font_source {
            FontSource::DefaultFontData { config } => {
                let font = self.add_font_default(config.as_ref());
                font.id()
            }
            FontSource::TtfData { data, size_pixels, config } => {
                let font = self.add_font_from_memory_ttf(data, *size_pixels, config.as_ref(), None)
                    .expect("Failed to add TTF font from memory");
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
        glyph_ranges: Option<&[u32]>,
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
        glyph_ranges: Option<&[u32]>,
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
    pub fn get_glyph_ranges_default(&self) -> &[u32] {
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
            sys::ImFontAtlasBuildInit(self.raw);

            // Perform the main build
            sys::ImFontAtlasBuildMain(self.raw);

            // Update pointers
            sys::ImFontAtlasBuildUpdatePointers(self.raw);

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
                (*self.raw).destruct();
                let _ = Box::from_raw(self.raw);
            }
        }
    }
}

// FontAtlas is safe to send between threads as long as the ImGui context is not being used
unsafe impl Send for FontAtlas {}
// FontAtlas is safe to share between threads as long as access is synchronized
unsafe impl Sync for FontAtlas {}

/// Font configuration for loading fonts
#[derive(Debug, Clone)]
pub struct FontConfig {
    raw: sys::ImFontConfig,
}

impl FontConfig {
    /// Creates a new font configuration with default settings
    pub fn new() -> Self {
        Self {
            raw: unsafe { sys::ImFontConfig::new() },
        }
    }

    /// Returns the raw ImFontConfig pointer
    pub(crate) fn raw(&self) -> *const sys::ImFontConfig {
        &self.raw
    }

    /// Set the font size in pixels
    pub fn size_pixels(mut self, size: f32) -> Self {
        // Note: ImFontConfig doesn't have a direct size field in our bindings
        // The size is typically passed to the AddFont functions
        self
    }

    /// Set whether to merge this font with the previous one
    pub fn merge_mode(mut self, merge: bool) -> Self {
        self.raw.MergeMode = merge;
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
}

impl Default for FontConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A source for font data
#[derive(Clone, Debug)]
pub enum FontSource<'a> {
    /// Default font included with the library (ProggyClean.ttf)
    DefaultFontData { config: Option<FontConfig> },
    /// Binary TTF/OTF font data
    TtfData {
        data: &'a [u8],
        size_pixels: f32,
        config: Option<FontConfig>,
    },
}
