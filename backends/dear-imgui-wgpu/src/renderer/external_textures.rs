use super::WgpuRenderer;
use crate::{ExternalTextureId, RendererError, RendererResult};

impl WgpuRenderer {
    /// Registers an application-owned WGPU texture view for Dear ImGui rendering.
    ///
    /// The renderer clones the view handle. The application retains ownership of the texture
    /// contents and must not explicitly destroy the underlying GPU resource while it is
    /// registered.
    pub fn register_external_texture(
        &mut self,
        view: &wgpu::TextureView,
    ) -> RendererResult<ExternalTextureId> {
        self.ensure_renderer_contract()?;
        self.texture_manager.register_external_view(view)
    }

    /// Replaces the WGPU view associated with an external texture handle.
    ///
    /// Stale handles, handles from another renderer, and already-unregistered handles are
    /// rejected without changing renderer state.
    pub fn update_external_texture(
        &mut self,
        texture: ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> RendererResult<()> {
        self.ensure_renderer_contract()?;
        let backend = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("WGPU renderer is not initialized".to_owned())
        })?;
        self.texture_manager.update_external_view(texture, view)?;
        backend
            .render_resources
            .remove_image_bind_group(texture.texture_id());
        Ok(())
    }

    /// Unregisters an application-owned external texture view.
    ///
    /// The underlying WGPU texture remains application-owned and is not destroyed by this call.
    pub fn unregister_external_texture(
        &mut self,
        texture: ExternalTextureId,
    ) -> RendererResult<()> {
        self.ensure_renderer_contract()?;
        let backend = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("WGPU renderer is not initialized".to_owned())
        })?;
        self.texture_manager.remove_external_view(texture)?;
        backend
            .render_resources
            .remove_image_bind_group(texture.texture_id());
        Ok(())
    }
}
