use std::marker::PhantomData;

use crate::fonts::atlas::loader::FontLoaderFlags;
use crate::fonts::atlas::state::font_atlas_state;
use crate::sys;

use super::FontAtlasRef;

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
