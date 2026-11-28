use super::*;
use dear_imgui_rs::ContextAttachmentLease;
use std::rc::Rc;

/// Vulkan renderer for Dear ImGui using `ash`.
///
/// It records draws in an application command buffer and submits managed texture uploads to its
/// configured graphics queue. The application submits the main viewport's draw commands.
pub struct AshRenderer {
    pub(super) device: Device,
    pub(super) allocator: RendererAllocator,
    pub(super) queue: vk::Queue,
    pub(super) command_pool: vk::CommandPool,
    pub(super) resources: VulkanRendererResources,
    pub(super) textures: TextureManager,
    pub(super) consumer: Option<SynchronousRendererConsumer>,
    pub(super) context_state: Rc<RendererContextState>,
    pub(super) publication_attachment: Option<ContextAttachmentLease>,
    pub(super) default_texture_id: u64,
    pub(super) default_texture_pending: bool,
    pub(super) options: Options,
    pub(super) frames: Frames,
    pub(super) destroyed: bool,
    pub(super) uploads: UploadRing,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) render_mode: RenderMode,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_pipelines: HashMap<vk::Format, ViewportPipeline>,
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_clear_color: [f32; 4],
}

/// Keeps the backing allocator alive until its last Vulkan resource can be released.
pub(super) struct RendererAllocator {
    inner: Option<Allocator>,
    live_allocations: usize,
    failed_release: bool,
}

impl RendererAllocator {
    pub(super) fn new(allocator: Allocator) -> Self {
        Self {
            inner: Some(allocator),
            live_allocations: 0,
            failed_release: false,
        }
    }

    fn inner_mut(&mut self) -> &mut Allocator {
        self.inner.as_mut().expect("live renderer allocator")
    }

    pub(super) fn release(&mut self) -> RendererResult<()> {
        self.check_cleanup()?;
        if self.live_allocations != 0 {
            return Err(RendererError::Allocator(
                "Vulkan allocations still depend on the renderer allocator".into(),
            ));
        }
        drop(self.inner.take());
        Ok(())
    }

    pub(super) fn check_cleanup(&self) -> RendererResult<()> {
        if self.failed_release {
            return Err(RendererError::Allocator(
                "a renderer Vulkan allocation could not be released".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn is_released(&self) -> bool {
        self.inner.is_none()
    }

    fn record_release(&mut self, result: RendererResult<()>) -> RendererResult<()> {
        if result.is_ok() {
            self.live_allocations -= 1;
        } else {
            self.failed_release = true;
        }
        result
    }
}

impl Allocate for RendererAllocator {
    type Memory = Memory;

    fn create_buffer(
        &mut self,
        device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Memory)> {
        let allocation = self.inner_mut().create_buffer(device, size, usage)?;
        self.live_allocations += 1;
        Ok(allocation)
    }

    fn create_image(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> RendererResult<(vk::Image, Memory)> {
        let allocation = self
            .inner_mut()
            .create_image(device, width, height, format)?;
        self.live_allocations += 1;
        Ok(allocation)
    }

    fn destroy_buffer(
        &mut self,
        device: &Device,
        buffer: vk::Buffer,
        memory: Memory,
    ) -> RendererResult<()> {
        let result = self.inner_mut().destroy_buffer(device, buffer, memory);
        self.record_release(result)
    }

    fn destroy_image(
        &mut self,
        device: &Device,
        image: vk::Image,
        memory: Memory,
    ) -> RendererResult<()> {
        let result = self.inner_mut().destroy_image(device, image, memory);
        self.record_release(result)
    }

    fn update_buffer<T: Copy>(
        &mut self,
        device: &Device,
        memory: &mut Memory,
        data: &[T],
    ) -> RendererResult<()> {
        self.inner_mut().update_buffer(device, memory, data)
    }
}

impl Drop for RendererAllocator {
    fn drop(&mut self) {
        if self.live_allocations != 0
            && let Some(allocator) = self.inner.take()
        {
            // The last shared allocator could free memory still bound to a Vulkan object.
            // Retain it when completion or allocation release could not be established.
            std::mem::forget(allocator);
        }
    }
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
