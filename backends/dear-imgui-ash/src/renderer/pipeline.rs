use super::*;

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
#[derive(Clone, Copy)]
pub(super) struct ViewportPipeline {
    pub(super) pipeline: vk::Pipeline,
    pub(super) target: ViewportRenderTarget,
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
#[derive(Clone, Copy)]
pub(super) enum ViewportRenderTarget {
    #[cfg(feature = "render-pass")]
    RenderPass {
        clear: vk::RenderPass,
        discard: vk::RenderPass,
    },
    #[cfg(feature = "dynamic-rendering")]
    DynamicRendering(DynamicRendering),
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
impl ViewportRenderTarget {
    pub(super) fn pipeline_target(self) -> RenderTarget {
        match self {
            #[cfg(feature = "render-pass")]
            Self::RenderPass { clear, .. } => RenderTarget::RenderPass(clear),
            #[cfg(feature = "dynamic-rendering")]
            Self::DynamicRendering(params) => RenderTarget::DynamicRendering(params),
        }
    }

    pub(super) fn destroy(self, device: &Device) {
        match self {
            #[cfg(feature = "render-pass")]
            Self::RenderPass { clear, discard } => unsafe {
                device.destroy_render_pass(discard, None);
                device.destroy_render_pass(clear, None);
            },
            #[cfg(feature = "dynamic-rendering")]
            Self::DynamicRendering(_) => {
                let _ = device;
            }
        }
    }
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
pub(super) fn is_srgb_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB | vk::Format::A8B8G8R8_SRGB_PACK32
    )
}

#[cfg(all(
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"),
    feature = "render-pass"
))]
pub(super) fn create_viewport_render_pass(
    device: &Device,
    format: vk::Format,
    load_op: vk::AttachmentLoadOp,
) -> RendererResult<vk::RenderPass> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(load_op)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        // Swapchain contents are discarded for both CLEAR and DONT_CARE.
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

    let color_attachment_refs = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

    let subpass = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)];

    let dependencies = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )];

    let rp_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpass)
        .dependencies(&dependencies);
    unsafe { Ok(device.create_render_pass(&rp_info, None)?) }
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
pub(super) fn viewport_attachment_load_op(flags: ViewportFlags) -> vk::AttachmentLoadOp {
    if flags.contains(ViewportFlags::NO_RENDERER_CLEAR) {
        vk::AttachmentLoadOp::DONT_CARE
    } else {
        vk::AttachmentLoadOp::CLEAR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "render-pass", feature = "dynamic-rendering"))]
    #[test]
    fn viewport_mode_follows_the_main_target_when_both_features_are_enabled() {
        assert_eq!(
            RenderTarget::RenderPass(vk::RenderPass::null()).mode(),
            RenderMode::RenderPass
        );
        assert_eq!(
            RenderTarget::DynamicRendering(DynamicRendering {
                color_attachment_format: vk::Format::B8G8R8A8_UNORM,
                depth_attachment_format: None,
            })
            .mode(),
            RenderMode::DynamicRendering
        );
    }

    #[test]
    fn no_renderer_clear_selects_discard_policy() {
        assert_eq!(
            viewport_attachment_load_op(ViewportFlags::empty()),
            vk::AttachmentLoadOp::CLEAR
        );
        assert_eq!(
            viewport_attachment_load_op(ViewportFlags::NO_RENDERER_CLEAR),
            vk::AttachmentLoadOp::DONT_CARE
        );
    }
}
