use super::*;

/// Vulkan renderer for Dear ImGui using `ash`.
///
/// It records rendering commands to the provided command buffer and does not submit.
pub struct AshRenderer {
    pub(super) device: Device,
    pub(super) allocator: Allocator,
    pub(super) queue: vk::Queue,
    pub(super) command_pool: vk::CommandPool,
    pub(super) pipeline: vk::Pipeline,
    pub(super) pipeline_layout: vk::PipelineLayout,
    pub(super) descriptor_set_layout: vk::DescriptorSetLayout,
    pub(super) descriptor_pool: vk::DescriptorPool,
    pub(super) textures: TextureManager,
    pub(super) default_texture_id: u64,
    pub(super) options: Options,
    pub(super) frames: Frames,
    pub(super) destroyed: bool,
    pub(super) in_flight_uploads: VecDeque<InFlightUpload>,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_pipelines: HashMap<vk::Format, ViewportPipeline>,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_clear_color: [f32; 4],
}
