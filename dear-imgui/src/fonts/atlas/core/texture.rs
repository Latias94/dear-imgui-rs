use crate::fonts::atlas::LegacyFontAtlas;
use crate::sys;

use super::FontAtlas;

impl<'atlas> LegacyFontAtlas<'atlas> {
    /// Check if the texture is built
    pub fn is_built(&self) -> bool {
        unsafe { (*self.atlas.raw()).TexIsBuilt }
    }

    /// Resolve the current atlas texture ID.
    pub fn texture_id(&self) -> crate::texture::TextureId {
        self.atlas.texture_id_internal()
    }

    /// Set a legacy renderer-owned atlas texture ID and mark its native status as ready.
    ///
    /// Managed renderers should process the atlas request from `PendingFrame` and return
    /// request-bound `TextureFeedback` instead. This method also updates `TexRef` so legacy draw
    /// commands continue to follow `ImTextureData` when one is available.
    ///
    /// # Safety
    ///
    /// `tex_id` must identify a live texture owned by the active renderer for every draw command
    /// that can observe it. The atlas must be in legacy texture mode and must not be shared with a
    /// Context using managed renderer textures. The caller must clear or replace the binding only
    /// after all GPU use has completed.
    pub unsafe fn set_texture_id(&self, tex_id: crate::texture::TextureId) {
        unsafe {
            let raw = self.atlas.raw();
            let texture = (*raw).TexData;
            (*raw).TexRef = if texture.is_null() {
                sys::ImTextureRef {
                    _TexData: std::ptr::null_mut(),
                    _TexID: sys::ImTextureID::from(tex_id),
                }
            } else {
                sys::ImTextureData_SetTexID(texture, sys::ImTextureID::from(tex_id));
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
    ///
    /// Keep the CPU copy after uploading. Dear ImGui uses it to copy resident glyphs during
    /// repacking. To discard the atlas contents, use [`FontAtlas::clear`] and repopulate the
    /// atlas before rebuilding.
    ///
    /// ```compile_fail,E0599
    /// fn discard_pixels(atlas: &dear_imgui_rs::LegacyFontAtlas<'_>) {
    ///     atlas.clear_cpu_texture_data();
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a context using this atlas has an open frame. During drawing, use
    /// [`Ui::font_atlas_texture`](crate::Ui::font_atlas_texture).
    pub fn tex_data(&self) -> Option<crate::fonts::FontAtlasTexture<'atlas>> {
        self.atlas.tex_data_internal()
    }
}

impl FontAtlas {
    pub(crate) fn texture_id_internal(&self) -> crate::texture::TextureId {
        unsafe { crate::texture::effective_texture_id(&(*self.raw()).TexRef) }
    }
}
