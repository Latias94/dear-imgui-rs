use crate::sys;

use super::DrawListMut;

/// Tracks a texture pushed to a draw-list texture stack.
///
/// The texture is popped when the token is dropped or when [`Self::pop`] is
/// called explicitly. Tokens for the same draw list must finish in reverse creation order. A token
/// from a window draw list must also finish in the exact window `Begin` scope that created the
/// draw-list view. Prefer [`DrawListMut::with_texture`] for ordinary scoped use.
#[must_use]
pub struct DrawListTextureToken<'draw_list, 'tex> {
    scope: crate::scope::NativeScopeToken<'draw_list>,
    _texture: std::marker::PhantomData<&'tex ()>,
}

impl<'draw_list, 'tex> DrawListTextureToken<'draw_list, 'tex> {
    fn new(
        ui: &'draw_list crate::Ui,
        draw_list: *mut sys::ImDrawList,
        window_scoped: bool,
    ) -> Self {
        Self {
            scope: ui.begin_native_scope(
                crate::scope::NativeScopePop::DrawListPopTexture {
                    draw_list,
                    window_scoped,
                },
                "DrawListTextureToken",
            ),
            _texture: std::marker::PhantomData,
        }
    }

    /// Pop the texture immediately instead of waiting for drop.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later texture token for the same draw list is active or a
    /// window-scoped draw list is no longer in its originating window `Begin` scope.
    #[doc(alias = "PopTexture")]
    pub fn pop(self) {}

    /// Pop the texture immediately instead of waiting for drop.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::pop`].
    #[doc(alias = "PopTexture")]
    pub fn end(self) {}
}

impl Drop for DrawListTextureToken<'_, '_> {
    fn drop(&mut self) {
        self.scope.finish();
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
        let texture = texture.into();
        self.assert_scope("DrawListMut::push_texture()");
        self.ui().run_with_bound_context(|| {
            let tex_ref = self
                .ui()
                .resolve_texture_ref(texture)
                .unwrap_or_else(|error| {
                    panic!("DrawListMut::push_texture() rejected texture: {error}")
                });
            unsafe { sys::ImDrawList_PushTexture(self.draw_list, tex_ref) };
        });
        DrawListTextureToken::new(self.ui(), self.draw_list, self.is_window_scoped())
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
        let texture = self.push_texture(texture);
        let result = f();
        drop(texture);
        result
    }
}
