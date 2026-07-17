use super::PlatformIo;

impl PlatformIo {
    /// Get a shared iterator over all textures managed by the platform.
    ///
    /// Use this for inspection. Renderer backends or feedback application code that need to write
    /// texture status or backend IDs must use [`Self::textures_mut`].
    pub fn textures(&self) -> crate::render::draw_data::TextureIterator<'_> {
        unsafe {
            let vector = &self.inner().Textures;
            let size = match usize::try_from(vector.Size) {
                Ok(size) => size,
                Err(_) => 0,
            };
            if size == 0 || vector.Data.is_null() {
                crate::render::draw_data::TextureIterator::new(std::ptr::null(), std::ptr::null())
            } else {
                crate::render::draw_data::TextureIterator::new(vector.Data, vector.Data.add(size))
            }
        }
    }

    /// Get a mutable cursor over all textures managed by the platform.
    ///
    /// This is used on the UI thread for applying renderer feedback and during shutdown paths that
    /// need to mutate backend texture fields.
    pub fn textures_mut(&mut self) -> crate::render::draw_data::TextureMutCursor<'_> {
        unsafe {
            let vector = &mut self.inner_mut().Textures;
            let size = match usize::try_from(vector.Size) {
                Ok(size) => size,
                Err(_) => 0,
            };
            if size == 0 || vector.Data.is_null() {
                crate::render::draw_data::TextureMutCursor::new(
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            } else {
                crate::render::draw_data::TextureMutCursor::new(vector.Data, vector.Data.add(size))
            }
        }
    }

    /// Get the number of textures managed by the platform
    pub fn textures_count(&self) -> usize {
        let vector = &self.inner().Textures;
        if vector.Data.is_null() {
            return 0;
        }
        usize::try_from(vector.Size).unwrap_or(0)
    }

    /// Invalidate renderer-owned bindings while preserving managed texture data.
    ///
    /// Renderer backends must call this whenever the GPU objects backing their texture IDs are
    /// discarded. Dear ImGui converts live textures with retained pixels back to `WantCreate`,
    /// while textures already queued for destruction remain `Destroyed`. Textures referenced by
    /// multiple contexts are left untouched: a single renderer teardown cannot invalidate a shared
    /// atlas binding while another context may still use it.
    ///
    /// Returns the number of registered textures invalidated.
    pub fn invalidate_renderer_texture_bindings(&mut self) -> usize {
        let mut invalidated = 0;
        let mut textures = self.textures_mut();
        while let Some(mut texture) = textures.next() {
            if texture.ref_count() != 1 {
                continue;
            }
            texture.set_status(crate::texture::TextureStatus::Destroyed);
            invalidated += 1;
        }
        invalidated
    }

    /// Get a specific texture by index
    ///
    /// Returns None if the index is out of bounds.
    pub fn texture(&self, index: usize) -> Option<&crate::texture::TextureData> {
        unsafe {
            let vector = &self.inner().Textures;
            let size = usize::try_from(vector.Size).ok()?;
            if size == 0 || vector.Data.is_null() {
                return None;
            }
            if index >= size {
                return None;
            }
            let texture_ptr = *vector.Data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw_ref(
                texture_ptr as *const _,
            ))
        }
    }

    /// Get a mutable reference to a specific texture by index
    ///
    /// Returns None if the index is out of bounds.
    pub fn texture_mut(&mut self, index: usize) -> Option<&mut crate::texture::TextureData> {
        unsafe {
            let vector = &self.inner().Textures;
            let size = usize::try_from(vector.Size).ok()?;
            if size == 0 || vector.Data.is_null() {
                return None;
            }
            if index >= size {
                return None;
            }
            let texture_ptr = *vector.Data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw(texture_ptr))
        }
    }
}
