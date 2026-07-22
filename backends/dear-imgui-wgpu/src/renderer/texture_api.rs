use super::WgpuRenderer;
use crate::WgpuTextureManager;

impl WgpuRenderer {
    /// Get the texture manager
    pub fn texture_manager(&self) -> &WgpuTextureManager {
        &self.texture_manager
    }

    /// Check if the renderer is initialized
    pub fn is_initialized(&self) -> bool {
        self.context_state.is_some()
            && self.renderer_consumer.is_some()
            && self
                .backend_data
                .as_ref()
                .is_some_and(crate::WgpuBackendData::is_initialized)
    }
}
