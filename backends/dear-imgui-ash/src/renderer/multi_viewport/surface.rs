use super::*;

pub(super) struct ViewportAshData {
    pub(super) surface: vk::SurfaceKHR,
    pub(super) swapchain_loader: khr_swapchain::Device,
    pub(super) swapchain: vk::SwapchainKHR,
    pub(super) format: vk::Format,
    pub(super) extent: vk::Extent2D,
    pub(super) images: Vec<vk::Image>,
    pub(super) image_views: Vec<vk::ImageView>,
    #[cfg(feature = "dynamic-rendering")]
    pub(super) image_layouts: Vec<vk::ImageLayout>,
    #[cfg(not(feature = "dynamic-rendering"))]
    pub(super) framebuffers: Vec<vk::Framebuffer>,
    pub(super) command_pool: vk::CommandPool,
    pub(super) frames: Vec<FrameSync>,
    pub(super) images_in_flight: Vec<vk::Fence>,
    pub(super) frame_index: usize,
    pub(super) pending_present: Option<(usize, u32)>,
    pub(super) mesh_frames: Frames,
}

impl ViewportAshData {
    pub(super) fn destroy(
        mut self,
        renderer: &mut AshRenderer,
        surface_loader: &khr_surface::Instance,
    ) -> RendererResult<()> {
        unsafe {
            let _ = renderer.device.device_wait_idle();
        }

        let _ = self
            .mesh_frames
            .destroy(&renderer.device, &mut renderer.allocator);

        unsafe {
            for f in self.frames.drain(..) {
                renderer.device.destroy_semaphore(f.image_available, None);
                renderer.device.destroy_semaphore(f.render_finished, None);
                renderer.device.destroy_fence(f.fence, None);
                renderer
                    .device
                    .free_command_buffers(self.command_pool, &[f.command_buffer]);
            }
            renderer
                .device
                .destroy_command_pool(self.command_pool, None);

            #[cfg(not(feature = "dynamic-rendering"))]
            for fb in self.framebuffers.drain(..) {
                renderer.device.destroy_framebuffer(fb, None);
            }

            for view in self.image_views.drain(..) {
                renderer.device.destroy_image_view(view, None);
            }

            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            surface_loader.destroy_surface(self.surface, None);
        }

        Ok(())
    }
}
