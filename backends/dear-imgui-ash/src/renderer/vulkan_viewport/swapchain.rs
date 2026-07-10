use super::*;

pub(super) fn pick_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        return vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_SRGB,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        };
    }

    let preferred = [
        vk::Format::B8G8R8A8_SRGB,
        vk::Format::R8G8B8A8_SRGB,
        vk::Format::B8G8R8A8_UNORM,
        vk::Format::R8G8B8A8_UNORM,
    ];
    for preferred_format in preferred {
        if let Some(format) = formats
            .iter()
            .find(|format| format.format == preferred_format)
        {
            return *format;
        }
    }
    *formats.first().unwrap_or(&vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_UNORM,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    })
}

pub(super) fn pick_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else {
        vk::PresentModeKHR::FIFO
    }
}

pub(super) fn desired_extent_from_size_and_scale(
    size: [f32; 2],
    framebuffer_scale: [f32; 2],
) -> Option<vk::Extent2D> {
    let sx = if framebuffer_scale[0].is_finite() && framebuffer_scale[0] > 0.0 {
        framebuffer_scale[0]
    } else {
        1.0
    };
    let sy = if framebuffer_scale[1].is_finite() && framebuffer_scale[1] > 0.0 {
        framebuffer_scale[1]
    } else {
        1.0
    };
    let width = size[0] * sx;
    let height = size[1] * sy;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }

    let extent = vk::Extent2D {
        width: width.round() as u32,
        height: height.round() as u32,
    };
    (extent.width > 0 && extent.height > 0).then_some(extent)
}

pub(super) fn desired_extent_from_viewport(vpm: &Viewport) -> Option<vk::Extent2D> {
    desired_extent_from_size_and_scale(vpm.size(), vpm.framebuffer_scale())
}

pub(super) fn desired_extent_from_imvec2(
    size: sys::ImVec2,
    framebuffer_scale: [f32; 2],
) -> Option<vk::Extent2D> {
    desired_extent_from_size_and_scale([size.x, size.y], framebuffer_scale)
}

pub(super) fn select_swapchain_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    desired: Option<vk::Extent2D>,
) -> Option<vk::Extent2D> {
    if capabilities.current_extent.width != u32::MAX
        && capabilities.current_extent.height != u32::MAX
    {
        return (capabilities.current_extent.width > 0 && capabilities.current_extent.height > 0)
            .then_some(capabilities.current_extent);
    }

    let desired = desired?;
    let max_width = capabilities
        .max_image_extent
        .width
        .max(capabilities.min_image_extent.width);
    let max_height = capabilities
        .max_image_extent
        .height
        .max(capabilities.min_image_extent.height);
    let extent = vk::Extent2D {
        width: desired
            .width
            .max(capabilities.min_image_extent.width)
            .min(max_width),
        height: desired
            .height
            .max(capabilities.min_image_extent.height)
            .min(max_height),
    };
    (extent.width > 0 && extent.height > 0).then_some(extent)
}

fn destroy_image_views(device: &Device, image_views: Vec<vk::ImageView>) {
    unsafe {
        for image_view in image_views {
            device.destroy_image_view(image_view, None);
        }
    }
}

fn create_image_views(
    device: &Device,
    images: &[vk::Image],
    format: vk::Format,
) -> RendererResult<Vec<vk::ImageView>> {
    let mut image_views = Vec::with_capacity(images.len());
    for &image in images {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        match unsafe { device.create_image_view(&create_info, None) } {
            Ok(image_view) => image_views.push(image_view),
            Err(error) => {
                destroy_image_views(device, image_views);
                return Err(error.into());
            }
        }
    }
    Ok(image_views)
}

#[cfg(not(feature = "dynamic-rendering"))]
fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    image_views: &[vk::ImageView],
    extent: vk::Extent2D,
) -> RendererResult<Vec<vk::Framebuffer>> {
    let mut framebuffers = Vec::with_capacity(image_views.len());
    for &image_view in image_views {
        let create_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(std::slice::from_ref(&image_view))
            .width(extent.width)
            .height(extent.height)
            .layers(1);
        match unsafe { device.create_framebuffer(&create_info, None) } {
            Ok(framebuffer) => framebuffers.push(framebuffer),
            Err(error) => {
                unsafe {
                    for framebuffer in framebuffers {
                        device.destroy_framebuffer(framebuffer, None);
                    }
                }
                return Err(error.into());
            }
        }
    }
    Ok(framebuffers)
}

pub(super) fn recreate_swapchain(
    renderer: &mut AshRenderer,
    global: &GlobalHandles,
    data: &mut ViewportAshData,
    desired_extent: Option<vk::Extent2D>,
) -> RendererResult<()> {
    if data.state == ViewportRuntimeState::Failed {
        return Err(RendererError::InvalidRenderState(
            "cannot rebuild a failed viewport runtime".into(),
        ));
    }

    if data.swapchain.is_some() {
        if let Err(error) = unsafe { renderer.device.device_wait_idle() } {
            data.mark_failed();
            return Err(error.into());
        }
    }
    recreate_swapchain_after_device_idle(renderer, global, data, desired_extent)
}

pub(super) fn recreate_swapchain_after_device_idle(
    renderer: &mut AshRenderer,
    global: &GlobalHandles,
    data: &mut ViewportAshData,
    desired_extent: Option<vk::Extent2D>,
) -> RendererResult<()> {
    if data.state == ViewportRuntimeState::Failed {
        return Err(RendererError::InvalidRenderState(
            "cannot rebuild a failed viewport runtime".into(),
        ));
    }

    data.pending_present = None;
    data.rebuild_after_present = false;
    data.state = ViewportRuntimeState::RebuildRequired;

    let support = query_surface_support(global, data.surface)
        .map_err(|error| RendererError::InvalidRenderState(error.to_string()))?;
    let Some(extent) = select_swapchain_extent(&support.capabilities, desired_extent) else {
        data.state = ViewportRuntimeState::Paused;
        return Ok(());
    };
    let surface_format = pick_surface_format(&support.formats);
    let present_mode = pick_present_mode(&support.present_modes);

    #[cfg(not(feature = "dynamic-rendering"))]
    let clear_render_pass = renderer
        .viewport_pipeline(surface_format.format)?
        .clear_render_pass;
    #[cfg(feature = "dynamic-rendering")]
    renderer.viewport_pipeline(surface_format.format)?;

    let capabilities = support.capabilities;
    let min_image_count = {
        let desired = capabilities.min_image_count.saturating_add(1);
        if capabilities.max_image_count > 0 {
            desired.min(capabilities.max_image_count)
        } else {
            desired
        }
    };
    let composite_alpha = [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::INHERIT,
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
    ]
    .into_iter()
    .find(|candidate| capabilities.supported_composite_alpha.contains(*candidate))
    .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);

    let old_swapchain = data
        .swapchain
        .as_ref()
        .map_or(vk::SwapchainKHR::null(), |resources| resources.swapchain);
    let queue_family_indices = [
        global.graphics_queue_family_index,
        global.present_queue_family_index,
    ];
    let mut create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(data.surface)
        .min_image_count(min_image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .pre_transform(capabilities.current_transform)
        .composite_alpha(composite_alpha)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(old_swapchain);
    if global.graphics_queue_family_index != global.present_queue_family_index {
        create_info = create_info
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&queue_family_indices);
    } else {
        create_info = create_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
    }

    // Calling vkCreateSwapchainKHR with oldSwapchain may retire the old swapchain even when the
    // call fails. Stop exposing the old resources as active immediately after the call.
    let created_swapchain = unsafe { data.swapchain_loader.create_swapchain(&create_info, None) };
    data.retire_swapchain_after_device_idle(&renderer.device);
    let swapchain = created_swapchain?;

    let images = match unsafe { data.swapchain_loader.get_swapchain_images(swapchain) } {
        Ok(images) if !images.is_empty() => images,
        Ok(_) => {
            unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
            return Err(RendererError::InvalidRenderState(
                "Vulkan swapchain returned no images".into(),
            ));
        }
        Err(error) => {
            unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
            return Err(error.into());
        }
    };
    let image_views = match create_image_views(&renderer.device, &images, surface_format.format) {
        Ok(image_views) => image_views,
        Err(error) => {
            unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
            return Err(error);
        }
    };
    let present_semaphores = match create_present_semaphores(&renderer.device, images.len()) {
        Ok(semaphores) => semaphores,
        Err(error) => {
            destroy_image_views(&renderer.device, image_views);
            unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
            return Err(error);
        }
    };
    #[cfg(not(feature = "dynamic-rendering"))]
    let framebuffers =
        match create_framebuffers(&renderer.device, clear_render_pass, &image_views, extent) {
            Ok(framebuffers) => framebuffers,
            Err(error) => {
                destroy_present_semaphores(&renderer.device, present_semaphores);
                destroy_image_views(&renderer.device, image_views);
                unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
                return Err(error);
            }
        };

    let image_count = images.len();
    data.swapchain = Some(SwapchainResources {
        swapchain,
        format: surface_format.format,
        extent,
        #[cfg(feature = "dynamic-rendering")]
        images,
        image_views,
        #[cfg(feature = "dynamic-rendering")]
        image_layouts: vec![vk::ImageLayout::UNDEFINED; image_count],
        #[cfg(not(feature = "dynamic-rendering"))]
        framebuffers,
        present_semaphores,
        images_in_flight: vec![vk::Fence::null(); image_count],
    });
    data.state = ViewportRuntimeState::Active;
    Ok(())
}

#[cfg(feature = "dynamic-rendering")]
fn swapchain_image_barrier(
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) -> (
    vk::PipelineStageFlags,
    vk::PipelineStageFlags,
    vk::ImageMemoryBarrier<'static>,
) {
    let (src_access, dst_access, src_stage, dst_stage) =
        if new_layout == vk::ImageLayout::PRESENT_SRC_KHR {
            (
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::empty(),
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            )
        } else {
            (
                vk::AccessFlags::empty(),
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            )
        };
    let barrier = vk::ImageMemoryBarrier::default()
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .src_access_mask(src_access)
        .dst_access_mask(dst_access);
    (src_stage, dst_stage, barrier)
}

#[cfg(feature = "dynamic-rendering")]
pub(super) fn transition_swapchain_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let (src_stage, dst_stage, barrier) = swapchain_image_barrier(image, old_layout, new_layout);
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}

#[cfg(all(test, feature = "dynamic-rendering"))]
mod dynamic_tests {
    use super::*;

    #[test]
    fn swapchain_barriers_do_not_transfer_queue_family_ownership() {
        let (_, _, barrier) = swapchain_image_barrier(
            vk::Image::null(),
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );
        assert_eq!(barrier.src_queue_family_index, vk::QUEUE_FAMILY_IGNORED);
        assert_eq!(barrier.dst_queue_family_index, vk::QUEUE_FAMILY_IGNORED);
    }
}
