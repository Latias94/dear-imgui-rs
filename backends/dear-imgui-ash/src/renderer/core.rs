use super::*;

/// Vulkan renderer for Dear ImGui using `ash`.
///
/// It records rendering commands to the provided command buffer and does not submit.
pub struct AshRenderer {
    pub(super) device: Device,
    pub(super) allocator: Allocator,
    pub(super) queue: vk::Queue,
    pub(super) command_pool: vk::CommandPool,
    pub(super) resources: VulkanRendererResources,
    pub(super) textures: TextureManager,
    pub(super) consumer: Option<SynchronousRendererConsumer>,
    pub(super) context_state: RendererContextState,
    pub(super) default_texture_id: u64,
    pub(super) options: Options,
    pub(super) frames: Frames,
    pub(super) destroyed: bool,
    pub(super) in_flight_uploads: VecDeque<InFlightUpload>,
    pub(super) managed_uploads: ManagedUploadTracker,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_pipelines: HashMap<vk::Format, ViewportPipeline>,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_clear_color: [f32; 4],
}

impl AshRenderer {
    /// Returns the synchronous consumer capability owned by this renderer.
    ///
    /// Pass it to [`dear_imgui_rs::Context::render`] to create the pending frame consumed by
    /// [`Self::prepare_frame`]. [`Self::cmd_draw`] then consumes the reconciled frame returned by
    /// that preparation step.
    pub fn renderer_consumer(&self) -> RendererResult<&SynchronousRendererConsumer> {
        self.consumer
            .as_ref()
            .ok_or(RendererError::RendererNotAttached)
    }
}
