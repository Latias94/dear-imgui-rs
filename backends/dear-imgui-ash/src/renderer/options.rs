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
    /// Maximum number of sampled-image descriptor sets allocated from the pool.
    ///
    /// This excludes the two renderer-owned standard sampler sets and must be at least 8 so font
    /// atlas replacements and fence-delayed texture retirement have bounded headroom.
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
        }
    }
}

/// Renderer-owned Vulkan handles and target configuration.
///
/// Construct this value from handles that belong to one logical-device lineage, then pass it to
/// one of [`super::AshRenderer`]'s unsafe constructors. The constructors retain `device`; the
/// caller retains ownership of the queue and command pool and must keep them live and externally
/// synchronized for the renderer lifetime.
pub struct AshRendererConfig {
    pub(super) device: Device,
    pub(super) queue: vk::Queue,
    pub(super) command_pool: vk::CommandPool,
    #[cfg(not(feature = "dynamic-rendering"))]
    pub(super) render_pass: vk::RenderPass,
    #[cfg(feature = "dynamic-rendering")]
    pub(super) dynamic_rendering: DynamicRendering,
    pub(super) options: Options,
}

impl AshRendererConfig {
    /// Configure a renderer for one compatible render pass.
    #[cfg(not(feature = "dynamic-rendering"))]
    pub fn with_render_pass(
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        render_pass: vk::RenderPass,
    ) -> Self {
        Self {
            device,
            queue,
            command_pool,
            render_pass,
            options: Options::default(),
        }
    }

    /// Configure a renderer for Vulkan dynamic rendering.
    #[cfg(feature = "dynamic-rendering")]
    pub fn with_dynamic_rendering(
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        dynamic_rendering: DynamicRendering,
    ) -> Self {
        Self {
            device,
            queue,
            command_pool,
            dynamic_rendering,
            options: Options::default(),
        }
    }

    /// Replace the renderer options.
    #[must_use]
    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }
}

/// `dynamic-rendering` feature related params.
#[cfg(feature = "dynamic-rendering")]
#[derive(Debug, Clone, Copy)]
pub struct DynamicRendering {
    pub color_attachment_format: vk::Format,
    pub depth_attachment_format: Option<vk::Format>,
}
