use crate::{GammaMode, ShaderManager, WgpuBackendData, WgpuTextureManager};
use wgpu::TextureView;

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use wgpu::Color;

/// Main WGPU renderer for Dear ImGui

///
/// This corresponds to the main renderer functionality in imgui_impl_wgpu.cpp
pub struct WgpuRenderer {
    /// Backend data
    pub(super) backend_data: Option<WgpuBackendData>,
    /// Shader manager
    pub(super) shader_manager: ShaderManager,
    /// Texture manager
    pub(super) texture_manager: WgpuTextureManager,
    /// Default texture for fallback
    pub(super) default_texture: Option<TextureView>,
    /// Gamma mode: automatic (by format), force linear (1.0), or force 2.2
    pub(super) gamma_mode: GammaMode,
    /// Clear color used for secondary viewports (multi-viewport mode)
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_clear_color: Color,
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
impl Drop for WgpuRenderer {
    fn drop(&mut self) {
        self.clear_multi_viewport_renderer_state();
    }
}
