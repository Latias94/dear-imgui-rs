//! Font management for Dear ImGui

use crate::Result;
use dear_imgui_sys as sys;

/// Handle to a font atlas texture
#[derive(Clone, Debug)]
pub struct FontAtlasTexture {
    /// Texture width (in pixels)
    pub width: u32,
    /// Texture height (in pixels)
    pub height: u32,
    /// Raw texture data (in RGBA32 format)
    pub data: Vec<u8>,
}

/// Font atlas manager
pub struct FontAtlas {
    raw: *mut sys::ImFontAtlas,
}

impl FontAtlas {
    /// Create a new FontAtlas from a raw pointer
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid and lives as long as this FontAtlas
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImFontAtlas) -> Self {
        Self { raw }
    }

    /// Build RGBA32 texture data from the font atlas
    pub fn build_rgba32_texture(&mut self) -> FontAtlasTexture {
        unsafe {
            let mut pixels: *mut u8 = std::ptr::null_mut();
            let mut width: i32 = 0;
            let mut height: i32 = 0;
            let mut bytes_per_pixel: i32 = 0;

            // Get texture data from Dear ImGui
            sys::ImFontAtlas_GetTexDataAsRGBA32(
                self.raw,
                &mut pixels,
                &mut width,
                &mut height,
                &mut bytes_per_pixel,
            );

            if !pixels.is_null() && width > 0 && height > 0 {
                let data_size = (width * height * bytes_per_pixel) as usize;
                let data = std::slice::from_raw_parts(pixels, data_size).to_vec();

                FontAtlasTexture {
                    width: width as u32,
                    height: height as u32,
                    data,
                }
            } else {
                FontAtlasTexture {
                    width: 0,
                    height: 0,
                    data: Vec::new(),
                }
            }
        }
    }

    /// Get the current texture ID
    pub fn tex_id(&self) -> u64 {
        unsafe { (*self.raw).__bindgen_anon_1.TexID._TexID as u64 }
    }

    /// Set the texture ID
    pub fn set_tex_id(&mut self, tex_id: u64) {
        unsafe {
            (*self.raw).__bindgen_anon_1.TexID = sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: tex_id,
            };
        }
    }

    /// Clear texture data to save memory
    pub fn clear_tex_data(&mut self) {
        unsafe {
            sys::ImFontAtlas_ClearTexData(self.raw);
        }
    }

    /// Check if the font atlas is built
    pub fn is_built(&self) -> bool {
        unsafe { (*self.raw).TexIsBuilt }
    }

    /// Get the raw pointer to the ImFontAtlas
    pub fn raw(&self) -> *mut sys::ImFontAtlas {
        self.raw
    }

    /// Add a font from TTF/OTF file data
    ///
    /// # Arguments
    ///
    /// * `font_data` - Font file data (TTF/OTF)
    /// * `size_pixels` - Font size in pixels
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let font_data = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
    /// let mut atlas = ctx.fonts();
    /// let font = atlas.add_font_from_memory(font_data, 16.0);
    /// ```
    pub fn add_font_from_memory(&mut self, font_data: &[u8], size_pixels: f32) -> Option<Font> {
        unsafe {
            let font_ptr = sys::ImFontAtlas_AddFontFromMemoryTTF(
                self.raw,
                font_data.as_ptr() as *mut std::os::raw::c_void,
                font_data.len() as i32,
                size_pixels,
                std::ptr::null(),
                std::ptr::null(),
            );

            if font_ptr.is_null() {
                None
            } else {
                Some(Font::from_raw(font_ptr))
            }
        }
    }

    /// Add the default font
    ///
    /// # Arguments
    ///
    /// * `size_pixels` - Font size in pixels (optional, uses default if None)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// let mut atlas = ctx.fonts();
    /// let font = atlas.add_font_default(Some(16.0));
    /// ```
    pub fn add_font_default(&mut self, size_pixels: Option<f32>) -> Font {
        unsafe {
            let config = if let Some(size) = size_pixels {
                let mut config = sys::ImFontConfig::default();
                config.SizePixels = size;
                &config as *const _
            } else {
                std::ptr::null()
            };

            let font_ptr = sys::ImFontAtlas_AddFontDefault(self.raw, config);
            Font::from_raw(font_ptr)
        }
    }

    /// Get the number of fonts in the atlas
    pub fn font_count(&self) -> i32 {
        unsafe { (*self.raw).Fonts.Size }
    }

    /// Get a font by index
    pub fn get_font(&self, index: i32) -> Option<Font> {
        unsafe {
            if index >= 0 && index < self.font_count() {
                let font_ptr = *(*self.raw).Fonts.Data.add(index as usize);
                Some(Font::from_raw(font_ptr))
            } else {
                None
            }
        }
    }
}

/// Handle to a font
#[derive(Clone, Debug)]
pub struct Font {
    raw: *mut sys::ImFont,
}

impl Font {
    /// Create a Font from a raw pointer
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid and lives as long as this Font
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImFont) -> Self {
        Self { raw }
    }

    /// Get the font size in pixels
    pub fn font_size(&self) -> f32 {
        // Note: FontSize field might not be available in all ImGui versions
        // Return a default size for now
        16.0
    }

    /// Get the font scale
    pub fn scale(&self) -> f32 {
        // Note: Scale field might not be available in all ImGui versions
        // Return a default scale for now
        1.0
    }

    /// Set the font scale
    pub fn set_scale(&mut self, _scale: f32) {
        // Note: Scale field might not be available in all ImGui versions
        // This is a placeholder implementation
    }

    /// Check if the font is loaded
    pub fn is_loaded(&self) -> bool {
        !self.raw.is_null()
    }

    /// Get the raw pointer to the ImFont
    pub fn raw(&self) -> *mut sys::ImFont {
        self.raw
    }

    /// Calculate text size with this font
    ///
    /// # Arguments
    ///
    /// * `text` - Text to measure
    /// * `wrap_width` - Maximum width for text wrapping (0.0 for no wrapping)
    ///
    /// # Returns
    ///
    /// Text size as (width, height)
    pub fn calc_text_size(&self, text: &str, wrap_width: f32) -> (f32, f32) {
        let c_text = std::ffi::CString::new(text).unwrap_or_default();
        unsafe {
            let size = sys::ImFont_CalcTextSizeA(
                self.raw,
                self.font_size(),
                f32::MAX,
                wrap_width,
                c_text.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            (size.x, size.y)
        }
    }
}
