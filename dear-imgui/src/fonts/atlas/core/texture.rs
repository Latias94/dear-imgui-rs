use crate::sys;

use super::FontAtlas;

impl FontAtlas {
    /// Check if the texture is built
    pub fn is_built(&self) -> bool {
        unsafe { (*self.raw()).TexIsBuilt }
    }

    /// Get texture data information
    ///
    /// Returns (min_width, min_height) if texture is built
    /// Note: Our Dear ImGui version uses a different texture management system
    pub fn get_tex_data_info(&self) -> Option<(u32, u32)> {
        let raw = self.raw();
        unsafe {
            if (*raw).TexIsBuilt {
                let min_width = (*raw).TexMinWidth as u32;
                let min_height = (*raw).TexMinHeight as u32;
                Some((min_width, min_height))
            } else {
                None
            }
        }
    }

    /// Resolve the current atlas texture ID.
    pub fn texture_id(&self) -> crate::texture::TextureId {
        unsafe { crate::texture::effective_texture_id(&(*self.raw()).TexRef) }
    }

    /// Convenience: set atlas texture id and mark status OK
    /// Also updates TexRef so draw commands continue to follow the managed
    /// `ImTextureData` when one is available.
    pub fn set_texture_id(&self, tex_id: crate::texture::TextureId) {
        unsafe {
            let raw = self.raw();
            let texture = (*raw).TexData;
            (*raw).TexRef = if texture.is_null() {
                sys::ImTextureRef {
                    _TexData: std::ptr::null_mut(),
                    _TexID: tex_id.id() as sys::ImTextureID,
                }
            } else {
                sys::ImTextureData_SetTexID(texture, tex_id.id() as sys::ImTextureID);
                sys::ImTextureData_SetStatus(texture, sys::ImTextureStatus_OK);
                sys::ImTextureData_GetTexRef(texture)
            };
        }
    }

    /// Lease the current atlas texture data, if available.
    ///
    /// Pixel slices borrowed from this view remain valid until the lease is dropped. While it is
    /// alive, safe atlas mutation and frame advancement reject operations that could invalidate
    /// the texture.
    pub fn tex_data(&self) -> Option<crate::fonts::FontAtlasTexture<'_>> {
        let raw = self.raw();
        let texture = unsafe { (*raw).TexData };
        unsafe { crate::fonts::FontAtlasTexture::from_raw(raw, texture) }
    }

    /// Get texture UV scale
    pub fn get_tex_uv_scale(&self) -> [f32; 2] {
        unsafe {
            let scale = (*self.raw()).TexUvScale;
            [scale.x, scale.y]
        }
    }

    /// Get texture UV white pixel coordinates
    pub fn get_tex_uv_white_pixel(&self) -> [f32; 2] {
        unsafe {
            let pixel = (*self.raw()).TexUvWhitePixel;
            [pixel.x, pixel.y]
        }
    }
}
