// Renderer external texture helpers (register/update/unregister)

use super::WgpuRenderer;

impl WgpuRenderer {
    /// Register an external WGPU texture + view and obtain a TextureId for ImGui usage.
    ///
    /// Use this when you already have a `wgpu::Texture` (e.g., game view RT, video frame,
    /// atlas upload) and want to display it via legacy `TextureId` path:
    /// `ui.image(TextureId::from(id), size)`.
    pub fn register_external_texture(
        &mut self,
        texture: &wgpu::Texture,
        view: &wgpu::TextureView,
    ) -> u64 {
        self.texture_manager
            .register_texture(crate::WgpuTexture::new(texture.clone(), view.clone()))
    }

    /// Register an external WGPU texture + view with a custom sampler.
    ///
    /// This lets you control sampling (e.g. nearest vs linear) per external texture.
    /// The sampler must be a filtering sampler compatible with the ImGui pipeline.
    pub fn register_external_texture_with_sampler(
        &mut self,
        texture: &wgpu::Texture,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> u64 {
        let id = self
            .texture_manager
            .register_texture(crate::WgpuTexture::new(texture.clone(), view.clone()));
        self.texture_manager
            .set_custom_sampler_for_texture(id, sampler.clone());
        id
    }

    /// Update the view for an already-registered external texture.
    ///
    /// Returns true if the texture existed and the view was replaced.
    pub fn update_external_texture_view(
        &mut self,
        texture_id: u64,
        view: &wgpu::TextureView,
    ) -> bool {
        if let Some(mut tex) = self.texture_manager.remove_texture(texture_id) {
            tex.texture_view = view.clone();
            self.texture_manager.insert_texture_with_id(texture_id, tex);
            if let Some(backend) = self.backend_data.as_mut() {
                backend.render_resources.remove_image_bind_group(texture_id);
            }
            true
        } else {
            false
        }
    }

    /// Update (or set) a custom sampler for an already-registered external texture.
    ///
    /// Returns false if the texture_id is not registered.
    pub fn update_external_texture_sampler(
        &mut self,
        texture_id: u64,
        sampler: &wgpu::Sampler,
    ) -> bool {
        self.texture_manager
            .update_custom_sampler_for_texture(texture_id, sampler.clone())
    }

    /// Unregister (remove) a texture by id. Safe for both external and managed textures.
    pub fn unregister_texture(&mut self, texture_id: u64) {
        self.texture_manager.remove_texture(texture_id);
        self.texture_manager
            .clear_custom_sampler_for_texture(texture_id);
        if let Some(backend) = self.backend_data.as_mut() {
            backend.render_resources.remove_image_bind_group(texture_id);
        }
    }
}
