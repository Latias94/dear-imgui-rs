use super::*;

/// WGPU texture resource
///
/// This corresponds to ImGui_ImplWGPU_Texture in the C++ implementation
#[derive(Debug)]
pub struct WgpuTexture {
    /// WGPU texture object
    pub texture: Texture,
    /// Texture view for binding
    pub texture_view: TextureView,
}

impl WgpuTexture {
    /// Create a new WGPU texture
    pub fn new(texture: Texture, texture_view: TextureView) -> Self {
        Self {
            texture,
            texture_view,
        }
    }

    /// Get the texture view for binding
    pub fn view(&self) -> &TextureView {
        &self.texture_view
    }

    /// Get the texture object
    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}
