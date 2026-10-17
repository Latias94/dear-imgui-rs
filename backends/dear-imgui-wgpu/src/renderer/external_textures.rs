use super::WgpuRenderer;
use crate::{ExternalTextureBinding, ExternalTextureId, RendererError, RendererResult};

impl WgpuRenderer {
    /// Returns the current view-mapping revision of an external texture registration.
    ///
    /// The returned value is owned metadata. It does not keep the registration or texture alive,
    /// and it does not identify the application's texture-content frame.
    pub fn external_texture_binding(
        &self,
        texture: ExternalTextureId,
    ) -> RendererResult<ExternalTextureBinding> {
        self.ensure_renderer_contract()?;
        self.gpu
            .texture_manager
            .external_binding(texture.texture_id())
            .ok_or(RendererError::ExternalTextureNotFound(texture.texture_id()))
    }

    /// Registers an application-owned WGPU texture view for Dear ImGui rendering.
    ///
    /// The renderer clones the view handle. The application retains ownership of the texture
    /// contents and must not explicitly destroy the underlying GPU resource while it is
    /// registered or while recorded/submitted GPU work still uses it. Keep the registration until
    /// all Dear ImGui frames referencing its identifier have been recorded.
    pub fn register_external_texture(
        &mut self,
        view: &wgpu::TextureView,
    ) -> RendererResult<ExternalTextureId> {
        self.ensure_renderer_contract()?;
        self.gpu.texture_manager.register_external_view(view)
    }

    /// Replaces the WGPU view associated with an external texture handle.
    ///
    /// Every successful replacement advances the binding revision. Stale handles, handles from
    /// another renderer, already-unregistered handles, and exhausted revisions are rejected
    /// without changing the view mapping or cached image bind group.
    pub fn update_external_texture(
        &mut self,
        texture: ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> RendererResult<()> {
        self.ensure_renderer_contract()?;
        let backend = self.gpu.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("WGPU renderer is not initialized".to_owned())
        })?;
        self.gpu
            .texture_manager
            .update_external_view(texture, view)?;
        backend
            .render_resources
            .remove_image_bind_group(texture.texture_id());
        Ok(())
    }

    /// Unregisters an application-owned external texture view.
    ///
    /// The underlying WGPU texture remains application-owned and is not destroyed by this call.
    /// Record every Dear ImGui frame referencing this handle before unregistering it. This method
    /// removes the mapping immediately and does not wait for GPU completion; explicit destruction
    /// of the application texture must wait for its remaining GPU users.
    pub fn unregister_external_texture(
        &mut self,
        texture: ExternalTextureId,
    ) -> RendererResult<()> {
        self.ensure_renderer_contract()?;
        let backend = self.gpu.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("WGPU renderer is not initialized".to_owned())
        })?;
        self.gpu.texture_manager.remove_external_view(texture)?;
        backend
            .render_resources
            .remove_image_bind_group(texture.texture_id());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WgpuInitInfo, WgpuTextureManager,
        texture::{external_test_texture, request_test_device},
    };
    use dear_imgui_rs::Context;

    #[test]
    fn invalid_external_texture_updates_preserve_bindings_and_cached_groups() -> RendererResult<()>
    {
        let Some((device, queue)) = request_test_device() else {
            return Ok(());
        };
        let mut context = Context::create();
        let mut renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm),
            &mut context,
        )?;
        let (_first_texture, first_view) = external_test_texture(&device);
        let (_second_texture, second_view) = external_test_texture(&device);
        let texture = renderer.register_external_texture(&first_view)?;
        let previous = renderer.external_texture_binding(texture)?;
        let backend = renderer.gpu.backend_data.as_mut().unwrap();
        let cached = backend
            .render_resources
            .get_or_create_image_bind_group(&device, texture.texture_id(), &first_view)?
            .clone();
        let mut foreign_manager = WgpuTextureManager::new();
        let foreign = foreign_manager.register_external_view(&second_view)?;

        assert!(matches!(
            renderer.update_external_texture(foreign, &second_view),
            Err(RendererError::ExternalTextureNotFound(id)) if id == foreign.texture_id()
        ));
        assert!(matches!(
            renderer.external_texture_binding(foreign),
            Err(RendererError::ExternalTextureNotFound(id)) if id == foreign.texture_id()
        ));
        assert_eq!(renderer.external_texture_binding(texture)?, previous);
        assert_eq!(
            renderer
                .gpu
                .texture_manager
                .texture_view(texture.texture_id()),
            Some(&first_view)
        );
        assert_eq!(
            renderer
                .gpu
                .backend_data
                .as_ref()
                .unwrap()
                .render_resources
                .image_bind_groups
                .get(&texture.texture_id()),
            Some(&cached)
        );

        renderer.update_external_texture(texture, &second_view)?;
        assert_eq!(
            renderer.external_texture_binding(texture)?.revision.get(),
            2
        );
        assert!(
            !renderer
                .gpu
                .backend_data
                .as_ref()
                .unwrap()
                .render_resources
                .image_bind_groups
                .contains_key(&texture.texture_id())
        );
        renderer.unregister_external_texture(texture)?;
        assert!(matches!(
            renderer.update_external_texture(texture, &first_view),
            Err(RendererError::ExternalTextureNotFound(id)) if id == texture.texture_id()
        ));
        renderer.shutdown(&mut context)
    }
}
