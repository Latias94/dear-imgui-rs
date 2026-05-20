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
    for p in preferred {
        if let Some(f) = formats.iter().find(|f| f.format == p) {
            return *f;
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

pub(super) fn extent_from_window(window: &Window) -> vk::Extent2D {
    let size = window.inner_size();
    vk::Extent2D {
        width: size.width.max(1),
        height: size.height.max(1),
    }
}

pub(super) fn recreate_swapchain(
    renderer: &mut AshRenderer,
    global: &GlobalHandles,
    surface_loader: &khr_surface::Instance,
    data: &mut ViewportAshData,
    window: &Window,
) -> RendererResult<()> {
    unsafe {
        let _ = renderer.device.device_wait_idle();
    }

    let caps = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(global.physical_device, data.surface)?
    };
    let formats = unsafe {
        surface_loader.get_physical_device_surface_formats(global.physical_device, data.surface)?
    };
    let present_modes = unsafe {
        surface_loader
            .get_physical_device_surface_present_modes(global.physical_device, data.surface)?
    };

    let surface_format = pick_surface_format(&formats);
    let present_mode = pick_present_mode(&present_modes);

    let mut extent = extent_from_window(window);
    if caps.current_extent.width != u32::MAX && caps.current_extent.height != u32::MAX {
        extent = caps.current_extent;
    }

    let min_image_count = {
        let desired = caps.min_image_count.saturating_add(1);
        if caps.max_image_count > 0 {
            desired.min(caps.max_image_count)
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
    .find(|c| caps.supported_composite_alpha.contains(*c))
    .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);

    let mut sci = vk::SwapchainCreateInfoKHR::default()
        .surface(data.surface)
        .min_image_count(min_image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .pre_transform(caps.current_transform)
        .composite_alpha(composite_alpha)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(data.swapchain);

    let queue_family_indices = [
        global.graphics_queue_family_index,
        global.present_queue_family_index,
    ];
    if global.graphics_queue_family_index != global.present_queue_family_index {
        sci = sci
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&queue_family_indices);
    } else {
        sci = sci.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
    }

    let swapchain = unsafe { data.swapchain_loader.create_swapchain(&sci, None)? };
    let images = match unsafe { data.swapchain_loader.get_swapchain_images(swapchain) } {
        Ok(images) => images,
        Err(err) => {
            unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
            return Err(err.into());
        }
    };

    // Ensure pipeline exists for this format (per-format support).
    if let Err(err) = renderer.viewport_pipeline(surface_format.format) {
        unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
        return Err(err);
    }

    let mut image_views = Vec::with_capacity(images.len());
    for &image in &images {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(surface_format.format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = match unsafe { renderer.device.create_image_view(&create_info, None) } {
            Ok(view) => view,
            Err(err) => {
                for view in image_views.drain(..) {
                    unsafe { renderer.device.destroy_image_view(view, None) };
                }
                unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
                return Err(err.into());
            }
        };
        image_views.push(view);
    }

    #[cfg(not(feature = "dynamic-rendering"))]
    let framebuffers = {
        let rp = match renderer.viewport_pipeline(surface_format.format) {
            Ok(vp) => vp.render_pass,
            Err(err) => {
                for view in image_views.drain(..) {
                    unsafe { renderer.device.destroy_image_view(view, None) };
                }
                unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
                return Err(err);
            }
        };
        let mut framebuffers = Vec::with_capacity(image_views.len());
        for &view in &image_views {
            let fb = match unsafe {
                renderer.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(rp)
                        .attachments(std::slice::from_ref(&view))
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1),
                    None,
                )
            } {
                Ok(fb) => fb,
                Err(err) => {
                    for fb in framebuffers.drain(..) {
                        unsafe { renderer.device.destroy_framebuffer(fb, None) };
                    }
                    for view in image_views.drain(..) {
                        unsafe { renderer.device.destroy_image_view(view, None) };
                    }
                    unsafe { data.swapchain_loader.destroy_swapchain(swapchain, None) };
                    return Err(err.into());
                }
            };
            framebuffers.push(fb);
        }
        framebuffers
    };

    #[cfg(not(feature = "dynamic-rendering"))]
    let old_framebuffers = std::mem::replace(&mut data.framebuffers, framebuffers);
    let old_image_views = std::mem::replace(&mut data.image_views, image_views);
    let old_swapchain = std::mem::replace(&mut data.swapchain, swapchain);
    data.images = images;
    #[cfg(feature = "dynamic-rendering")]
    {
        data.image_layouts = vec![vk::ImageLayout::UNDEFINED; data.images.len()];
    }
    data.format = surface_format.format;
    data.extent = extent;
    data.images_in_flight = vec![vk::Fence::null(); data.images.len()];
    data.pending_present = None;

    unsafe {
        #[cfg(not(feature = "dynamic-rendering"))]
        for fb in old_framebuffers {
            renderer.device.destroy_framebuffer(fb, None);
        }
        for view in old_image_views {
            renderer.device.destroy_image_view(view, None);
        }
        data.swapchain_loader.destroy_swapchain(old_swapchain, None);
    }
    Ok(())
}

#[cfg(feature = "dynamic-rendering")]
pub(super) fn transition_swapchain_image(
    device: &Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    old: vk::ImageLayout,
    new: vk::ImageLayout,
) {
    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old)
        .new_layout(new)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
    unsafe {
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}
