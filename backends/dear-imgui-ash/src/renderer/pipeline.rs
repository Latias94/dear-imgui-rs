use super::*;

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
pub(super) struct ViewportPipeline {
    pub(super) pipeline: vk::Pipeline,
    #[cfg(not(feature = "dynamic-rendering"))]
    pub(super) clear_render_pass: vk::RenderPass,
    #[cfg(not(feature = "dynamic-rendering"))]
    pub(super) discard_render_pass: vk::RenderPass,
}

#[cfg(all(
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"),
    not(feature = "dynamic-rendering")
))]
impl ViewportPipeline {
    pub(super) fn render_pass(&self, load_op: vk::AttachmentLoadOp) -> vk::RenderPass {
        // Load ops do not affect render-pass compatibility, so both passes share one pipeline.
        if load_op == vk::AttachmentLoadOp::DONT_CARE {
            self.discard_render_pass
        } else {
            debug_assert_eq!(load_op, vk::AttachmentLoadOp::CLEAR);
            self.clear_render_pass
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
    not(feature = "dynamic-rendering")
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
