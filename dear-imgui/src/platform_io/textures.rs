use super::PlatformIo;

impl PlatformIo {
    /// Invalidate renderer-owned bindings while preserving managed texture data.
    ///
    /// Renderer backends must call this whenever the GPU objects backing their texture IDs are
    /// discarded. Dear ImGui converts live textures with retained pixels back to `WantCreate`,
    /// while textures already queued for destruction remain `Destroyed`. Textures referenced by
    /// multiple contexts are left untouched: a single renderer teardown cannot invalidate a shared
    /// atlas binding while another context may still use it.
    ///
    /// Returns the number of registered textures invalidated.
    pub(crate) fn invalidate_renderer_texture_bindings(&mut self) -> usize {
        let vector = &mut self.inner_mut().Textures;
        let Ok(size) = usize::try_from(vector.Size) else {
            return 0;
        };
        if size == 0 || vector.Data.is_null() {
            return 0;
        }
        let mut invalidated = 0;
        for index in 0..size {
            let texture = unsafe { *vector.Data.add(index) };
            if texture.is_null() {
                continue;
            }
            let texture = unsafe { crate::texture::TextureData::from_raw(texture) };
            if texture.ref_count() != 1 {
                continue;
            }
            texture.set_status(crate::texture::TextureStatus::Destroyed);
            invalidated += 1;
        }
        invalidated
    }
}
