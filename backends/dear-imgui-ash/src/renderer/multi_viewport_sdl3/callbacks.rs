use super::*;

/// Renderer: create per-viewport Vulkan resources (surface + swapchain).
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_create_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };

        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);
        let vpm = &mut *vp;

        let mut out_surface: sys::ImU64 = 0;
        let err = (global.platform_create_vk_surface)(
            vpm.as_raw_mut(),
            global.instance.handle().as_raw(),
            std::ptr::null(),
            &mut out_surface,
        );
        if err != 0 || out_surface == 0 {
            eprintln!(
                "[ash-mv-sdl3] Platform_CreateVkSurface failed (err={err}, surface={out_surface})"
            );
            return;
        }
        let surface = vk::SurfaceKHR::from_raw(out_surface as u64);

        let swapchain_loader = khr_swapchain::Device::new(&global.instance, &renderer.device);

        let present_supported = match {
            surface_loader.get_physical_device_surface_support(
                global.physical_device,
                global.present_queue_family_index,
                surface,
            )
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_surface_support error: {e:?}");
                surface_loader.destroy_surface(surface, None);
                return;
            }
        };
        if !present_supported {
            eprintln!("[ash-mv-sdl3] surface has no present support for the selected queue family");
            surface_loader.destroy_surface(surface, None);
            return;
        }

        let caps = match {
            surface_loader.get_physical_device_surface_capabilities(global.physical_device, surface)
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_surface_capabilities error: {e:?}");
                surface_loader.destroy_surface(surface, None);
                return;
            }
        };
        let formats = match {
            surface_loader.get_physical_device_surface_formats(global.physical_device, surface)
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_surface_formats error: {e:?}");
                surface_loader.destroy_surface(surface, None);
                return;
            }
        };
        let present_modes = match {
            surface_loader
                .get_physical_device_surface_present_modes(global.physical_device, surface)
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_present_modes error: {e:?}");
                surface_loader.destroy_surface(surface, None);
                return;
            }
        };

        let surface_format = pick_surface_format(&formats);
        let present_mode = pick_present_mode(&present_modes);

        let mut extent = extent_from_viewport(vpm);
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
            .surface(surface)
            .min_image_count(min_image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .clipped(true);

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

        let swapchain = match swapchain_loader.create_swapchain(&sci, None) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ash-mv] create_swapchain error: {e:?}");
                surface_loader.destroy_surface(surface, None);
                return;
            }
        };
        let images = match swapchain_loader.get_swapchain_images(swapchain) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv] get_swapchain_images error: {e:?}");
                swapchain_loader.destroy_swapchain(swapchain, None);
                surface_loader.destroy_surface(surface, None);
                return;
            }
        };

        if let Err(e) = renderer.viewport_pipeline(surface_format.format) {
            eprintln!("[ash-mv] create viewport pipeline error: {e:?}");
            swapchain_loader.destroy_swapchain(swapchain, None);
            surface_loader.destroy_surface(surface, None);
            return;
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
            let view = match renderer.device.create_image_view(&create_info, None) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[ash-mv] create_image_view error: {e:?}");
                    for v in image_views.drain(..) {
                        renderer.device.destroy_image_view(v, None);
                    }
                    swapchain_loader.destroy_swapchain(swapchain, None);
                    surface_loader.destroy_surface(surface, None);
                    return;
                }
            };
            image_views.push(view);
        }

        #[cfg(not(feature = "dynamic-rendering"))]
        let framebuffers: Vec<vk::Framebuffer> = {
            let rp = match renderer.viewport_pipeline(surface_format.format) {
                Ok(vp) => vp.render_pass,
                Err(e) => {
                    eprintln!("[ash-mv] create viewport pipeline error: {e:?}");
                    for v in image_views.drain(..) {
                        renderer.device.destroy_image_view(v, None);
                    }
                    swapchain_loader.destroy_swapchain(swapchain, None);
                    surface_loader.destroy_surface(surface, None);
                    return;
                }
            };
            let mut framebuffers = Vec::with_capacity(image_views.len());
            for &view in &image_views {
                let fb = renderer.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(rp)
                        .attachments(std::slice::from_ref(&view))
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1),
                    None,
                );
                match fb {
                    Ok(fb) => framebuffers.push(fb),
                    Err(e) => {
                        eprintln!("[ash-mv] create_framebuffer error: {e:?}");
                        for fb in framebuffers.drain(..) {
                            renderer.device.destroy_framebuffer(fb, None);
                        }
                        for v in image_views.drain(..) {
                            renderer.device.destroy_image_view(v, None);
                        }
                        swapchain_loader.destroy_swapchain(swapchain, None);
                        surface_loader.destroy_surface(surface, None);
                        return;
                    }
                }
            }
            framebuffers
        };

        let command_pool =
            match create_command_pool(&renderer.device, global.graphics_queue_family_index) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[ash-mv] create_command_pool error: {e:?}");
                    #[cfg(not(feature = "dynamic-rendering"))]
                    for fb in framebuffers.iter().copied() {
                        renderer.device.destroy_framebuffer(fb, None);
                    }
                    for v in image_views.drain(..) {
                        renderer.device.destroy_image_view(v, None);
                    }
                    swapchain_loader.destroy_swapchain(swapchain, None);
                    surface_loader.destroy_surface(surface, None);
                    return;
                }
            };
        let frames =
            match create_frame_syncs(&renderer.device, command_pool, global.in_flight_frames) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[ash-mv] create frame sync error: {e:?}");
                    #[cfg(not(feature = "dynamic-rendering"))]
                    for fb in framebuffers.iter().copied() {
                        renderer.device.destroy_framebuffer(fb, None);
                    }
                    for v in image_views.drain(..) {
                        renderer.device.destroy_image_view(v, None);
                    }
                    renderer.device.destroy_command_pool(command_pool, None);
                    swapchain_loader.destroy_swapchain(swapchain, None);
                    surface_loader.destroy_surface(surface, None);
                    return;
                }
            };

        let image_count = images.len();
        let data = ViewportAshData {
            surface,
            swapchain_loader,
            swapchain,
            format: surface_format.format,
            extent,
            images_in_flight: vec![vk::Fence::null(); image_count],
            images,
            image_views,
            #[cfg(feature = "dynamic-rendering")]
            image_layouts: vec![vk::ImageLayout::UNDEFINED; image_count],
            #[cfg(not(feature = "dynamic-rendering"))]
            framebuffers,
            command_pool,
            frames,
            frame_index: 0,
            pending_present: None,
            mesh_frames: Frames::new(global.in_flight_frames),
        };

        let boxed = Box::new(data);
        vpm.set_renderer_user_data(Box::into_raw(boxed) as *mut c_void);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Renderer_CreateWindow");
        std::process::abort();
    }
}

/// Renderer: destroy per-viewport Vulkan resources.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_destroy_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;
        let data_ptr = vpm.renderer_user_data();
        if data_ptr.is_null() {
            return;
        }
        vpm.set_renderer_user_data(std::ptr::null_mut());
        let boxed: Box<ViewportAshData> = Box::from_raw(data_ptr as *mut ViewportAshData);
        let _ = boxed.destroy(&mut renderer, &surface_loader);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Renderer_DestroyWindow");
        std::process::abort();
    }
}

/// Renderer: resize/recreate per-viewport swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_set_window_size(
    vp: *mut Viewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;

        // Convert to physical pixels using framebuffer scale (fallback to 1.0).
        let scale = vpm.framebuffer_scale();
        let extent = extent_from_imvec2(size, scale);

        let Some(data) = viewport_user_data_mut(vpm) else {
            return;
        };

        if data.extent != extent {
            let _ = recreate_swapchain(&mut renderer, &global, &surface_loader, data, extent);
        }
    }));
    if res.is_err() {
        eprintln!("[ash-mv-sdl3] panic in Renderer_SetWindowSize");
        std::process::abort();
    }
}

/// Renderer: render viewport draw data into its swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_render_window(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;
        let desired_extent = extent_from_viewport(vpm);

        let raw_dd = vpm.draw_data();
        if raw_dd.is_null() {
            return;
        }
        let draw_data: &mut dear_imgui_rs::render::DrawData =
            dear_imgui_rs::render::DrawData::from_raw_mut(&mut *raw_dd);

        let Some(data) = viewport_user_data_mut(vpm) else {
            return;
        };

        let frame_i = data.frame_index % data.frames.len();
        data.frame_index = (data.frame_index + 1) % data.frames.len();
        let frame = &data.frames[frame_i];

        if renderer
            .device
            .wait_for_fences(&[frame.fence], true, u64::MAX)
            .is_err()
        {
            return;
        }

        let acquire = data.swapchain_loader.acquire_next_image(
            data.swapchain,
            u64::MAX,
            frame.image_available,
            vk::Fence::null(),
        );

        let (image_index, _suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                let _ = recreate_swapchain(
                    &mut renderer,
                    &global,
                    &surface_loader,
                    data,
                    desired_extent,
                );
                return;
            }
            Err(e) => {
                eprintln!("[ash-mv] acquire_next_image error: {e:?}");
                return;
            }
        };

        if data.images_in_flight[image_index as usize] != vk::Fence::null() {
            if renderer
                .device
                .wait_for_fences(
                    &[data.images_in_flight[image_index as usize]],
                    true,
                    u64::MAX,
                )
                .is_err()
            {
                return;
            }
        }
        data.images_in_flight[image_index as usize] = frame.fence;

        if renderer.device.reset_fences(&[frame.fence]).is_err() {
            return;
        }
        if renderer
            .device
            .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())
            .is_err()
        {
            return;
        }

        let mesh = match data.mesh_frames.next() {
            Some(m) => m,
            None => return,
        };

        let pipeline = match renderer.viewport_pipeline(data.format) {
            Ok(p) => p.pipeline,
            Err(e) => {
                eprintln!("[ash-mv] viewport_pipeline error: {e:?}");
                return;
            }
        };
        let gamma = renderer.gamma_for_format(data.format);

        if renderer
            .device
            .begin_command_buffer(
                frame.command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .is_err()
        {
            return;
        }

        #[cfg(not(feature = "dynamic-rendering"))]
        {
            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: renderer.viewport_clear_color(),
                },
            }];

            let rp = match renderer.viewport_pipeline(data.format) {
                Ok(p) => p.render_pass,
                Err(e) => {
                    eprintln!("[ash-mv] viewport_pipeline error: {e:?}");
                    return;
                }
            };
            renderer.device.cmd_begin_render_pass(
                frame.command_buffer,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(rp)
                    .framebuffer(data.framebuffers[image_index as usize])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: data.extent,
                    })
                    .clear_values(&clear_values),
                vk::SubpassContents::INLINE,
            );

            if let Err(e) =
                renderer.cmd_draw_with_mesh(frame.command_buffer, draw_data, pipeline, gamma, mesh)
            {
                eprintln!("[ash-mv] cmd_draw error: {e:?}");
                renderer.device.cmd_end_render_pass(frame.command_buffer);
                return;
            }
            renderer.device.cmd_end_render_pass(frame.command_buffer);
        }

        #[cfg(feature = "dynamic-rendering")]
        {
            let img_i = image_index as usize;
            let old_layout = data
                .image_layouts
                .get(img_i)
                .copied()
                .unwrap_or(vk::ImageLayout::UNDEFINED);
            transition_swapchain_image(
                &renderer.device,
                frame.command_buffer,
                data.images[img_i],
                old_layout,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            if let Some(slot) = data.image_layouts.get_mut(img_i) {
                *slot = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }

            let clear = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: renderer.viewport_clear_color(),
                },
            };
            let color_attachment = vk::RenderingAttachmentInfo::default()
                .image_view(data.image_views[image_index as usize])
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear);

            renderer.device.cmd_begin_rendering(
                frame.command_buffer,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: data.extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_attachment)),
            );

            if let Err(e) =
                renderer.cmd_draw_with_mesh(frame.command_buffer, draw_data, pipeline, gamma, mesh)
            {
                eprintln!("[ash-mv] cmd_draw error: {e:?}");
                renderer.device.cmd_end_rendering(frame.command_buffer);
                return;
            }
            renderer.device.cmd_end_rendering(frame.command_buffer);

            transition_swapchain_image(
                &renderer.device,
                frame.command_buffer,
                data.images[img_i],
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
            if let Some(slot) = data.image_layouts.get_mut(img_i) {
                *slot = vk::ImageLayout::PRESENT_SRC_KHR;
            }
        }

        if renderer
            .device
            .end_command_buffer(frame.command_buffer)
            .is_err()
        {
            return;
        }

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&frame.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&frame.command_buffer))
            .signal_semaphores(std::slice::from_ref(&frame.render_finished));

        if renderer
            .device
            .queue_submit(
                renderer.queue,
                std::slice::from_ref(&submit_info),
                frame.fence,
            )
            .is_err()
        {
            return;
        }

        data.pending_present = Some((frame_i, image_index));
    }));
    if res.is_err() {
        eprintln!("[ash-mv-sdl3] panic in Renderer_RenderWindow");
        std::process::abort();
    }
}

/// Renderer: present frame for viewport swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;
        let desired_extent = extent_from_viewport(vpm);

        let Some(data) = viewport_user_data_mut(vpm) else {
            return;
        };
        let Some((frame_i, image_index)) = data.pending_present.take() else {
            return;
        };

        let frame = &data.frames[frame_i];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&frame.render_finished))
            .swapchains(std::slice::from_ref(&data.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        let present = data
            .swapchain_loader
            .queue_present(global.present_queue, &present_info);
        match present {
            Ok(_) => {}
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                let _ = recreate_swapchain(
                    &mut renderer,
                    &global,
                    &surface_loader,
                    data,
                    desired_extent,
                );
            }
            Err(e) => {
                eprintln!("[ash-mv] queue_present error: {e:?}");
            }
        }
    }));
    if res.is_err() {
        eprintln!("[ash-mv-sdl3] panic in Renderer_SwapBuffers");
        std::process::abort();
    }
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn platform_render_window_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_render_window(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Platform_RenderWindow");
        std::process::abort();
    }
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn platform_swap_buffers_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_swap_buffers(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Platform_SwapBuffers");
        std::process::abort();
    }
}
