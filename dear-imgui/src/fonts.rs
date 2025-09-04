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
}
