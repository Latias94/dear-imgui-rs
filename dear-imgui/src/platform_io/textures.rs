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

    /// Apply managed texture feedback produced by a renderer thread.
    ///
    /// In ImGui 1.92+, `DrawData::textures()` is built from `PlatformIO.Textures[]`. Renderer backends
    /// are expected to update each `TextureData`'s `Status` (and `TexID` on creation) after handling
    /// `WantCreate`/`WantUpdates`/`WantDestroy` requests.
    ///
    /// In a threaded engine, the renderer thread cannot safely mutate ImGui state directly. The
    /// intended flow is:
    /// - UI thread: snapshot texture requests and send to renderer
    /// - Render thread: create/update/destroy GPU resources and produce `TextureFeedback`
    /// - UI thread: call this function before the next frame to apply the feedback
    ///
    /// Returns the number of textures updated.
    pub fn apply_texture_feedback(
        &mut self,
        feedback: &[crate::render::snapshot::TextureFeedback],
    ) -> usize {
        if feedback.is_empty() {
            return 0;
        }

        let mut by_id: std::collections::HashMap<
            crate::texture::ManagedTextureId,
            crate::render::snapshot::TextureFeedback,
        > = std::collections::HashMap::with_capacity(feedback.len());
        for &fb in feedback {
            by_id.insert(fb.id, fb);
        }

        let mut applied = 0usize;
        let mut textures = self.textures_mut();
        while let Some(mut tex) = textures.next() {
            let uid = tex.unique_id();
            let Some(fb) = by_id.get(&uid) else { continue };

            // Destroyed clears backend bindings (TexID/BackendUserData) as expected by ImGui.
            if fb.status == crate::texture::TextureStatus::Destroyed {
                tex.set_status(fb.status);
            } else {
                if let Some(tex_id) = fb.tex_id {
                    tex.set_tex_id(tex_id);
                }
                tex.set_status(fb.status);
            }

            applied += 1;
        }

        applied
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
