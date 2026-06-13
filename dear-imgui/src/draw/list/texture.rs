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

    /// Pop the texture immediately instead of waiting for drop.
    #[doc(alias = "PopTexture")]
    pub fn end(self) {}
}

impl Drop for DrawListTextureToken<'_, '_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_PopTexture(self.draw_list) }
    }
}

impl<'ui> DrawListMut<'ui> {
    // channels_split is provided on DrawListMut

    /// Push a texture on the drawlist texture stack (ImGui 1.92+).
    ///
    /// While pushed, image and primitives will use this texture unless otherwise specified.
    /// The returned token pops the texture when dropped.
    ///
    /// Example:
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # fn demo(ui: &Ui) {
    /// let dl = ui.get_window_draw_list();
    /// let tex = texture::TextureId::new(1);
    /// let _texture = dl.push_texture(tex);
    /// dl.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
    /// # }
    /// ```
    #[doc(alias = "PushTexture")]
    pub fn push_texture<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
    ) -> DrawListTextureToken<'_, 'tex> {
        let tex_ref = texture.into().raw();
        unsafe { sys::ImDrawList_PushTexture(self.draw_list, tex_ref) };
        DrawListTextureToken::new(self.draw_list)
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
        let _texture = self.push_texture(texture);
        f()
    }
}
