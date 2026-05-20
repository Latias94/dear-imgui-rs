use super::WgpuRenderer;
use crate::{RendererError, RendererResult, WgpuTextureManager};

impl WgpuRenderer {
    /// Get the texture manager
    pub fn texture_manager(&self) -> &WgpuTextureManager {
        &self.texture_manager
    }

    /// Get the texture manager mutably
    pub fn texture_manager_mut(&mut self) -> &mut WgpuTextureManager {
        &mut self.texture_manager
    }

    /// Check if the renderer is initialized
    pub fn is_initialized(&self) -> bool {
        self.backend_data.is_some()
    }

    /// Update a single texture manually
    ///
    /// This corresponds to ImGui_ImplWGPU_UpdateTexture in the C++ implementation.
    /// Use this when you need precise control over texture update timing.
    ///
    /// # Returns
    ///
    /// Returns a `TextureUpdateResult` that contains any status/ID updates that need
    /// to be applied to the texture data. This follows Rust's principle of explicit
    /// state management.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui_wgpu::*;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # // Assume `renderer` has already been created and initialized elsewhere.
    /// # let mut renderer: WgpuRenderer = todo!();
    /// # let mut texture_data = dear_imgui_rs::TextureData::new();
    /// let result = renderer.update_texture(&texture_data)?;
    /// result.apply_to(&mut texture_data);
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_texture(
        &mut self,
        texture_data: &dear_imgui_rs::TextureData,
    ) -> RendererResult<crate::TextureUpdateResult> {
        if let Some(backend_data) = &mut self.backend_data {
            let result = self.texture_manager.update_single_texture(
                texture_data,
                &backend_data.device,
                &backend_data.queue,
            )?;

            // Invalidate any cached bind groups for this texture id so that subsequent
            // draws will see the updated texture view.
            match result {
                crate::TextureUpdateResult::Created { texture_id } => {
                    backend_data
                        .render_resources
                        .remove_image_bind_group(texture_id);
                }
                crate::TextureUpdateResult::Updated | crate::TextureUpdateResult::Destroyed => {
                    let id = texture_data.tex_id();
                    if !id.is_null() {
                        backend_data.render_resources.remove_image_bind_group(id);
                    }
                }
                crate::TextureUpdateResult::Failed | crate::TextureUpdateResult::NoAction => {}
            }

            Ok(result)
        } else {
            Err(RendererError::InvalidRenderState(
                "Renderer not initialized".to_string(),
            ))
        }
    }
}
