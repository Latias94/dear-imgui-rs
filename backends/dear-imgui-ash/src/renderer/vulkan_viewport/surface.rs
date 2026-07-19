use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportRuntimeState {
    Active,
    Paused,
    RebuildRequired,
    Failed,
}

impl ViewportRuntimeState {
    pub(super) fn can_acquire(self) -> bool {
        self == Self::Active
    }
}

pub(super) struct SwapchainResources {
    pub(super) swapchain: vk::SwapchainKHR,
    pub(super) format: vk::Format,
    pub(super) extent: vk::Extent2D,
    #[cfg(feature = "dynamic-rendering")]
    pub(super) images: Vec<vk::Image>,
    pub(super) image_views: Vec<vk::ImageView>,
    #[cfg(feature = "dynamic-rendering")]
    pub(super) image_layouts: Vec<vk::ImageLayout>,
    #[cfg(not(feature = "dynamic-rendering"))]
    pub(super) framebuffers: Vec<vk::Framebuffer>,
    pub(super) present_semaphores: Vec<vk::Semaphore>,
    pub(super) images_in_flight: Vec<vk::Fence>,
}

impl SwapchainResources {
    pub(super) fn destroy(mut self, device: &Device, swapchain_loader: &khr_swapchain::Device) {
        unsafe {
            #[cfg(not(feature = "dynamic-rendering"))]
            for framebuffer in self.framebuffers.drain(..) {
                device.destroy_framebuffer(framebuffer, None);
            }
            for view in self.image_views.drain(..) {
                device.destroy_image_view(view, None);
            }
        }
        destroy_present_semaphores(device, self.present_semaphores);
        unsafe { swapchain_loader.destroy_swapchain(self.swapchain, None) };
    }
}

pub(super) struct ViewportAshData {
    pub(super) surface: vk::SurfaceKHR,
    pub(super) swapchain_loader: khr_swapchain::Device,
    pub(super) swapchain: Option<SwapchainResources>,
    pub(super) command_pool: vk::CommandPool,
    pub(super) frames: Vec<FrameSync>,
    pub(super) frame_index: usize,
    pub(super) pending_present: Option<u32>,
    pub(super) rebuild_after_present: bool,
    pub(super) state: ViewportRuntimeState,
    pub(super) mesh_frames: Frames,
}

impl ViewportAshData {
    pub(super) fn mark_failed(&mut self) {
        self.pending_present = None;
        self.rebuild_after_present = false;
        self.state = ViewportRuntimeState::Failed;
    }

    pub(super) fn retire_swapchain_after_device_idle(&mut self, device: &Device) {
        if let Some(resources) = self.swapchain.take() {
            resources.destroy(device, &self.swapchain_loader);
        }
    }

    pub(super) fn destroy_after_device_idle(
        mut self,
        renderer: &mut AshRenderer,
        surface_loader: &khr_surface::Instance,
    ) -> RendererResult<()> {
        self.retire_swapchain_after_device_idle(&renderer.device);
        let _ = self
            .mesh_frames
            .destroy(&renderer.device, &mut renderer.allocator);
        destroy_frame_syncs(&renderer.device, self.command_pool, self.frames);

        unsafe {
            renderer
                .device
                .destroy_command_pool(self.command_pool, None);
            surface_loader.destroy_surface(self.surface, None);
        }

        Ok(())
    }
}
