use std::marker::PhantomData;

use crate::sys;

use super::DrawListMut;

/// Tracks a texture pushed to a draw-list texture stack.
///
/// The texture is popped when the token is dropped or when [`Self::pop`] is
/// called explicitly.
#[must_use]
pub struct DrawListTextureToken<'draw_list, 'tex> {
    draw_list: *mut sys::ImDrawList,
    _phantom: PhantomData<(&'draw_list (), &'tex mut crate::texture::TextureData)>,
}

impl<'draw_list, 'tex> DrawListTextureToken<'draw_list, 'tex> {
    fn new(draw_list: *mut sys::ImDrawList) -> Self {
        Self {
            draw_list,
            _phantom: PhantomData,
        }
    }

    /// Pop the texture immediately instead of waiting for drop.
    #[doc(alias = "PopTexture")]
    pub fn pop(self) {}
}

impl Drop for DrawListTextureToken<'_, '_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_PopTexture(self.draw_list) }
    }
}

impl<'ui> DrawListMut<'ui> {
    // channels_split is provided on DrawListMut

    /// Push a texture on the drawlist texture stack (ImGui 1.92+)
    ///
    /// While pushed, image and primitives will use this texture unless otherwise specified.
    ///
    /// Example:
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # fn demo(ui: &Ui) {
    /// let dl = ui.get_window_draw_list();
    /// let tex = texture::TextureId::new(1);
    /// unsafe { dl.push_texture(tex) };
    /// dl.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
    /// dl.pop_texture();
    /// # }
    /// ```
    #[doc(alias = "PushTexture")]
    ///
    /// # Safety
    ///
    /// The pushed texture reference remains on Dear ImGui's draw-list texture stack until
    /// [`Self::pop_texture`] is called. If this is a managed texture reference, the referenced
    /// texture data must remain valid until the stack entry is popped and any draw commands using it
    /// have been consumed. Prefer [`Self::push_texture_token`] or [`Self::with_texture`] for scoped
    /// safe usage.
    pub unsafe fn push_texture<'tex>(&self, texture: impl Into<crate::texture::TextureRef<'tex>>) {
        let tex_ref = texture.into().raw();
        unsafe { sys::ImDrawList_PushTexture(self.draw_list, tex_ref) }
    }

    /// Push a texture on the draw-list texture stack and return an RAII token.
    ///
    /// Prefer this or [`Self::with_texture`] for scoped usage that remains
    /// balanced if a panic unwinds through the scope. The manual
    /// [`Self::push_texture`] / [`Self::pop_texture`] pair is kept for
    /// compatibility with existing push/pop-style code.
    #[doc(alias = "PushTexture")]
    pub fn push_texture_token<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
    ) -> DrawListTextureToken<'_, 'tex> {
        unsafe { self.push_texture(texture) };
        DrawListTextureToken::new(self.draw_list)
    }

    /// Pop the last texture from the drawlist texture stack (ImGui 1.92+)
    #[doc(alias = "PopTexture")]
    pub fn pop_texture(&self) {
        unsafe {
            sys::ImDrawList_PopTexture(self.draw_list);
        }
    }

    /// Push a texture, run `f`, then pop the texture.
    ///
    /// The texture is popped during unwinding if `f` panics.
    #[doc(alias = "PushTexture", alias = "PopTexture")]
    pub fn with_texture<'tex, R>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        f: impl FnOnce() -> R,
    ) -> R {
        let _texture = self.push_texture_token(texture);
        f()
    }
}
