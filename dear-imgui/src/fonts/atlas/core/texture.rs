use crate::sys;

use super::FontAtlas;

impl FontAtlas {
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
