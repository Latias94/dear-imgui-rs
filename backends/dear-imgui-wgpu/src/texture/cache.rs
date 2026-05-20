use super::*;

impl WgpuTextureManager {
    /// Create a new texture manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 1, // Start from 1, 0 is reserved for null texture
            custom_samplers: HashMap::new(),
            custom_sampler_by_texture: HashMap::new(),
            common_bind_groups: HashMap::new(),
            next_sampler_id: 1, // Start from 1, 0 means "default sampler"
        }
    }

    /// Register a new texture and return its ID
    pub fn register_texture(&mut self, texture: WgpuTexture) -> TextureId {
        let id = TextureId::new(self.next_id);
        self.next_id += 1;
        self.textures.insert(id, texture);
        id
    }

    /// Get a texture by ID
    pub fn get_texture(&self, id: TextureId) -> Option<&WgpuTexture> {
        self.textures.get(&id)
    }

    /// Remove a texture by ID
    pub fn remove_texture(&mut self, id: TextureId) -> Option<WgpuTexture> {
        self.textures.remove(&id)
    }

    /// Check if a texture exists
    pub fn contains_texture(&self, id: TextureId) -> bool {
        self.textures.contains_key(&id)
    }

    /// Insert a texture with a specific ID
    pub fn insert_texture_with_id(&mut self, id: TextureId, texture: WgpuTexture) {
        self.textures.insert(id, texture);
        // Update next_id if necessary
        if id.id() >= self.next_id {
            self.next_id = id.id().saturating_add(1);
        }
    }

    /// Associate a custom sampler with a texture id (used by external textures).
    ///
    /// Returns the internal sampler_id assigned to this sampler.
    pub(crate) fn set_custom_sampler_for_texture(
        &mut self,
        texture_id: TextureId,
        sampler: Sampler,
    ) -> u64 {
        let sampler_id = self.next_sampler_id;
        self.next_sampler_id += 1;
        self.custom_samplers.insert(sampler_id, sampler);
        self.custom_sampler_by_texture
            .insert(texture_id, sampler_id);
        // Invalidate any cached common bind group for this sampler id (defensive).
        self.common_bind_groups.remove(&sampler_id);
        sampler_id
    }

    /// Update or set a custom sampler for an existing texture.
    ///
    /// If the texture already has a custom sampler association, we replace the sampler
    /// in place (keeping the sampler_id stable) and invalidate the cached common bind group.
    /// If there is no association yet, we create one.
    ///
    /// Returns false if the texture_id is not registered.
    pub(crate) fn update_custom_sampler_for_texture(
        &mut self,
        texture_id: TextureId,
        sampler: Sampler,
    ) -> bool {
        if !self.textures.contains_key(&texture_id) {
            return false;
        }
        if let Some(sampler_id) = self.custom_sampler_by_texture.get(&texture_id).copied() {
            self.custom_samplers.insert(sampler_id, sampler);
            self.common_bind_groups.remove(&sampler_id);
        } else {
            self.set_custom_sampler_for_texture(texture_id, sampler);
        }
        true
    }

    /// Get the custom sampler id for a texture (if any).
    pub(crate) fn custom_sampler_id_for_texture(&self, texture_id: TextureId) -> Option<u64> {
        self.custom_sampler_by_texture.get(&texture_id).copied()
    }

    /// Remove any custom sampler association for a texture.
    pub(crate) fn clear_custom_sampler_for_texture(&mut self, texture_id: TextureId) {
        if let Some(sampler_id) = self.custom_sampler_by_texture.remove(&texture_id) {
            // Drop cached bind group so next use rebuilds it.
            self.common_bind_groups.remove(&sampler_id);
        }
    }

    /// Get or create a common bind group (uniform buffer + sampler) for the given sampler id.
    ///
    /// The bind group uses the same uniform buffer but swaps the sampler, allowing
    /// per-texture sampling without changing the pipeline layout.
    pub(crate) fn get_or_create_common_bind_group_for_sampler(
        &mut self,
        device: &Device,
        common_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        sampler_id: u64,
    ) -> Option<BindGroup> {
        if let Some(bg) = self.common_bind_groups.get(&sampler_id) {
            return Some(bg.clone());
        }
        let sampler = self.custom_samplers.get(&sampler_id)?;
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Dear ImGui Common Bind Group (custom sampler)"),
            layout: common_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        });
        self.common_bind_groups.insert(sampler_id, bg.clone());
        Some(bg)
    }

    /// Get the number of registered textures
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Clear all textures
    pub fn clear(&mut self) {
        self.textures.clear();
        self.next_id = 1;
        self.custom_sampler_by_texture.clear();
        self.common_bind_groups.clear();
        // Keep samplers around? Clear to avoid holding stale handles after device loss.
        self.custom_samplers.clear();
        self.next_sampler_id = 1;
    }
}
