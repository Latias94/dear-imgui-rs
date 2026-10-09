use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportRuntimeState {
    Active,
    Paused,
    RebuildRequired,
    AcquireRecoveryRequired { frame_index: usize },
    Failed,
}

impl ViewportRuntimeState {
    pub(super) fn can_acquire(self) -> bool {
        self == Self::Active
    }

    pub(super) fn begin_acquire(&mut self, frame_index: usize) -> bool {
        if *self != Self::Active {
            return false;
        }
        *self = Self::AcquireRecoveryRequired { frame_index };
        true
    }

    pub(super) fn recovery_frame_index(self) -> Option<usize> {
        match self {
            Self::AcquireRecoveryRequired { frame_index } => Some(frame_index),
            Self::Active | Self::Paused | Self::RebuildRequired | Self::Failed => None,
        }
    }

    pub(super) fn finish_submission(&mut self, frame_index: usize) -> bool {
        if self.recovery_frame_index() != Some(frame_index) {
            return false;
        }
        *self = Self::Active;
        true
    }
}

pub(super) enum SwapchainRenderTarget {
    #[cfg(feature = "render-pass")]
    RenderPass { framebuffers: Vec<vk::Framebuffer> },
    #[cfg(feature = "dynamic-rendering")]
    DynamicRendering {
        images: Vec<vk::Image>,
        image_layouts: Vec<vk::ImageLayout>,
    },
}

pub(super) enum ViewportFrameTarget {
    #[cfg(feature = "render-pass")]
    RenderPass {
        render_pass: vk::RenderPass,
        framebuffer: vk::Framebuffer,
    },
    #[cfg(feature = "dynamic-rendering")]
    DynamicRendering {
        image: vk::Image,
        image_view: vk::ImageView,
        old_layout: vk::ImageLayout,
    },
}

pub(super) struct SwapchainResources {
    pub(super) swapchain: vk::SwapchainKHR,
    pub(super) format: vk::Format,
    pub(super) extent: vk::Extent2D,
    pub(super) image_views: Vec<vk::ImageView>,
    pub(super) target: SwapchainRenderTarget,
    pub(super) present_semaphores: Vec<vk::Semaphore>,
    pub(super) images_in_flight: Vec<vk::Fence>,
}

impl SwapchainResources {
    pub(super) fn frame_target(
        &self,
        pipeline_target: ViewportRenderTarget,
        image_index: usize,
        load_op: vk::AttachmentLoadOp,
    ) -> Option<ViewportFrameTarget> {
        match (&self.target, pipeline_target) {
            #[cfg(feature = "render-pass")]
            (
                SwapchainRenderTarget::RenderPass { framebuffers },
                ViewportRenderTarget::RenderPass { clear, discard },
            ) => {
                // Load ops do not affect render-pass compatibility; both passes share framebuffers.
                let render_pass = if load_op == vk::AttachmentLoadOp::DONT_CARE {
                    discard
                } else {
                    debug_assert_eq!(load_op, vk::AttachmentLoadOp::CLEAR);
                    clear
                };
                Some(ViewportFrameTarget::RenderPass {
                    render_pass,
                    framebuffer: *framebuffers.get(image_index)?,
                })
            }
            #[cfg(feature = "dynamic-rendering")]
            (
                SwapchainRenderTarget::DynamicRendering {
                    images,
                    image_layouts,
                },
                ViewportRenderTarget::DynamicRendering(_),
            ) => {
                let _ = load_op;
                Some(ViewportFrameTarget::DynamicRendering {
                    image: *images.get(image_index)?,
                    image_view: *self.image_views.get(image_index)?,
                    old_layout: *image_layouts.get(image_index)?,
                })
            }
            #[cfg(all(feature = "render-pass", feature = "dynamic-rendering"))]
            _ => None,
        }
    }

    pub(super) fn destroy(mut self, device: &Device, swapchain_loader: &khr_swapchain::Device) {
        unsafe {
            match self.target {
                #[cfg(feature = "render-pass")]
                SwapchainRenderTarget::RenderPass { framebuffers } => {
                    for framebuffer in framebuffers {
                        device.destroy_framebuffer(framebuffer, None);
                    }
                }
                #[cfg(feature = "dynamic-rendering")]
                SwapchainRenderTarget::DynamicRendering { .. } => {}
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
    pub(super) requested_extent: Option<vk::Extent2D>,
    pub(super) command_pool: vk::CommandPool,
    pub(super) frames: Vec<FrameSync>,
    pub(super) frame_index: usize,
    pub(super) pending_present: Option<u32>,
    pub(super) rebuild_after_present: bool,
    pub(super) state: ViewportRuntimeState,
    pub(super) mesh_frames: Frames,
}

#[cfg(test)]
mod render_target_tests {
    use super::*;
    use ash::vk::Handle;

    fn swapchain(target: SwapchainRenderTarget) -> SwapchainResources {
        SwapchainResources {
            swapchain: vk::SwapchainKHR::null(),
            format: vk::Format::B8G8R8A8_SRGB,
            extent: vk::Extent2D {
                width: 640,
                height: 480,
            },
            image_views: vec![vk::ImageView::from_raw(3)],
            target,
            present_semaphores: Vec::new(),
            images_in_flight: Vec::new(),
        }
    }

    #[cfg(feature = "render-pass")]
    #[test]
    fn render_pass_frames_select_clear_or_discard_without_dynamic_resources() {
        let resources = swapchain(SwapchainRenderTarget::RenderPass {
            framebuffers: vec![vk::Framebuffer::from_raw(4)],
        });
        let target = ViewportRenderTarget::RenderPass {
            clear: vk::RenderPass::from_raw(1),
            discard: vk::RenderPass::from_raw(2),
        };
        assert!(
            matches!(target.pipeline_target(), RenderTarget::RenderPass(pass) if pass.as_raw() == 1)
        );
        for (load_op, expected_pass) in [
            (vk::AttachmentLoadOp::CLEAR, 1),
            (vk::AttachmentLoadOp::DONT_CARE, 2),
        ] {
            assert!(matches!(resources.frame_target(target, 0, load_op),
                Some(ViewportFrameTarget::RenderPass { render_pass, framebuffer })
                    if render_pass.as_raw() == expected_pass && framebuffer.as_raw() == 4));
        }
        assert!(
            resources
                .frame_target(target, 1, vk::AttachmentLoadOp::CLEAR)
                .is_none()
        );
    }

    #[cfg(feature = "dynamic-rendering")]
    #[test]
    fn dynamic_frames_use_the_acquired_image_and_tracked_layout_without_framebuffers() {
        let resources = swapchain(SwapchainRenderTarget::DynamicRendering {
            images: vec![vk::Image::from_raw(5)],
            image_layouts: vec![vk::ImageLayout::PRESENT_SRC_KHR],
        });
        let target = ViewportRenderTarget::DynamicRendering(DynamicRendering {
            color_attachment_format: resources.format,
            depth_attachment_format: None,
        });
        assert!(
            matches!(target.pipeline_target(), RenderTarget::DynamicRendering(params)
            if params.color_attachment_format == resources.format && params.depth_attachment_format.is_none())
        );
        assert!(
            matches!(resources.frame_target(target, 0, vk::AttachmentLoadOp::CLEAR),
            Some(ViewportFrameTarget::DynamicRendering { image, image_view, old_layout })
                if image.as_raw() == 5 && image_view.as_raw() == 3 && old_layout == vk::ImageLayout::PRESENT_SRC_KHR)
        );
        assert!(
            resources
                .frame_target(target, 1, vk::AttachmentLoadOp::CLEAR)
                .is_none()
        );
    }

    #[cfg(all(feature = "render-pass", feature = "dynamic-rendering"))]
    #[test]
    fn mismatched_pipeline_and_swapchain_modes_are_rejected_before_recording() {
        let render_pass = ViewportRenderTarget::RenderPass {
            clear: vk::RenderPass::null(),
            discard: vk::RenderPass::null(),
        };
        let dynamic = ViewportRenderTarget::DynamicRendering(DynamicRendering {
            color_attachment_format: vk::Format::B8G8R8A8_SRGB,
            depth_attachment_format: None,
        });
        let render_pass_resources = swapchain(SwapchainRenderTarget::RenderPass {
            framebuffers: vec![vk::Framebuffer::null()],
        });
        let dynamic_resources = swapchain(SwapchainRenderTarget::DynamicRendering {
            images: vec![vk::Image::null()],
            image_layouts: vec![vk::ImageLayout::UNDEFINED],
        });
        assert!(
            render_pass_resources
                .frame_target(dynamic, 0, vk::AttachmentLoadOp::CLEAR)
                .is_none()
        );
        assert!(
            dynamic_resources
                .frame_target(render_pass, 0, vk::AttachmentLoadOp::CLEAR)
                .is_none()
        );
    }
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
