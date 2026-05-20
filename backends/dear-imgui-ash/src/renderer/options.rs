use super::*;

/// Optional parameters of the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// The number of in-flight frames of the application.
    pub in_flight_frames: usize,
    /// If true enables depth test when rendering.
    pub enable_depth_test: bool,
    /// If true enables depth writes when rendering.
    pub enable_depth_write: bool,
    /// Subpass for the graphics pipeline.
    pub subpass: u32,
    /// Sample count for the graphics pipeline multisampling state.
    pub sample_count: vk::SampleCountFlags,
    /// Maximum number of texture descriptor sets allocated from the pool.
    pub max_textures: u32,
    /// If true, treat the render target as sRGB.
    ///
    /// This backend follows the WGPU renderer approach: ImGui provides colors/texels in sRGB
    /// space (stored as UNORM), and the fragment shader applies `pow(rgb, gamma)` to convert
    /// to linear before writing to an sRGB render target.
    pub framebuffer_srgb: bool,
    /// Override the gamma used for sRGB->linear conversion in the shader.
    ///
    /// - `None`: auto (2.2 when `framebuffer_srgb`, else 1.0)
    /// - `Some(gamma)`: force a value (e.g. 2.2 or 1.0)
    pub color_gamma_override: Option<f32>,
    /// Format used for internally managed RGBA textures (font atlas, `TextureData` uploads).
    ///
    /// Recommended: keep this as `vk::Format::R8G8B8A8_UNORM` to match the shader gamma path.
    pub texture_format: vk::Format,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            in_flight_frames: 1,
            enable_depth_test: false,
            enable_depth_write: false,
            subpass: 0,
            sample_count: vk::SampleCountFlags::TYPE_1,
            max_textures: 1024,
            framebuffer_srgb: false,
            color_gamma_override: None,
            texture_format: vk::Format::R8G8B8A8_UNORM,
        }
    }
}

/// `dynamic-rendering` feature related params.
#[cfg(feature = "dynamic-rendering")]
#[derive(Debug, Clone, Copy)]
pub struct DynamicRendering {
    pub color_attachment_format: vk::Format,
    pub depth_attachment_format: Option<vk::Format>,
}
