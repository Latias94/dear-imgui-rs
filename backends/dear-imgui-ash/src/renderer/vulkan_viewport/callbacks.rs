use super::registry::binding_for_current_context;
use super::*;

pub(super) fn request_platform_close_after_create_failure(viewport: &mut Viewport) {
    viewport.set_platform_request_close(true);
}

/// Renderer: create per-viewport Vulkan resources (surface + swapchain).
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn renderer_create_window(viewport: *mut Viewport) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui owns this live viewport for the current context. The registry enforces a
    // single renderer borrow, and every Vulkan resource destroyed on failure was created here.
    unsafe {
        let Some(context_binding) = binding_for_current_context() else {
            return;
        };
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let viewport = &mut *viewport;
        if !viewport.renderer_user_data().is_null() {
            eprintln!("[ash-mv] refusing to overwrite foreign RendererUserData");
            return;
        }

        let surface =
            match global
                .surface_adapter
                .create_surface(&global.entry, &global.instance, viewport)
            {
                Ok(surface) => surface,
                Err(error) => {
                    eprintln!("[ash-mv] create surface error: {error}");
                    request_platform_close_after_create_failure(viewport);
                    return;
                }
            };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);
        let swapchain_loader = khr_swapchain::Device::new(&global.instance, &renderer.device);
        let command_pool =
            match create_command_pool(&renderer.device, global.graphics_queue_family_index) {
                Ok(command_pool) => command_pool,
                Err(error) => {
                    eprintln!("[ash-mv] create command pool error: {error}");
                    surface_loader.destroy_surface(surface, None);
                    request_platform_close_after_create_failure(viewport);
                    return;
                }
            };
        let frames =
            match create_frame_syncs(&renderer.device, command_pool, global.in_flight_frames) {
                Ok(frames) => frames,
                Err(error) => {
                    eprintln!("[ash-mv] create frame synchronization error: {error}");
                    renderer.device.destroy_command_pool(command_pool, None);
                    surface_loader.destroy_surface(surface, None);
                    request_platform_close_after_create_failure(viewport);
                    return;
                }
            };

        let mut data = ViewportAshData {
            surface,
            swapchain_loader,
            swapchain: None,
            command_pool,
            frames,
            frame_index: 0,
            pending_present: None,
            rebuild_after_present: false,
            state: ViewportRuntimeState::RebuildRequired,
            mesh_frames: Frames::new(global.in_flight_frames),
        };
        if let Err(error) = recreate_swapchain(
            &mut renderer,
            &global,
            &mut data,
            desired_extent_from_viewport(viewport),
        ) {
            eprintln!("[ash-mv] initial swapchain build failed: {error}");
            let _ = data.destroy(&mut renderer, &surface_loader);
            request_platform_close_after_create_failure(viewport);
            return;
        }

        let data = Box::into_raw(Box::new(data));
        register_viewport_data(&context_binding, data);
        viewport.set_renderer_user_data(data.cast());
    }
}

/// Renderer: destroy per-viewport Vulkan resources.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn renderer_destroy_window(viewport: *mut Viewport) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui owns this live viewport, and the context-local registry proves both the
    // renderer borrow and viewport allocation belong to this runtime before reclamation.
    unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);
        let viewport = &mut *viewport;
        let Some(data) = take_viewport_data(viewport) else {
            return;
        };
        let _ = data.destroy(&mut renderer, &surface_loader);
    }
}

fn rebuild_or_log(
    renderer: &mut AshRenderer,
    global: &GlobalHandles,
    data: &mut ViewportAshData,
    desired_extent: Option<vk::Extent2D>,
) {
    if let Err(error) = recreate_swapchain(renderer, global, data, desired_extent) {
        eprintln!("[ash-mv] swapchain rebuild deferred: {error}");
    }
}

/// Renderer: resize/recreate per-viewport swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn renderer_set_window_size(viewport: *mut Viewport, size: sys::ImVec2) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui owns this live viewport, and registry checks serialize renderer and
    // viewport-data mutation before any raw pointer is dereferenced.
    unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let viewport = &mut *viewport;
        let desired_extent = desired_extent_from_imvec2(size, viewport.framebuffer_scale());
        let Some(data) = viewport_user_data_mut(viewport) else {
            return;
        };
        if data.state != ViewportRuntimeState::Failed {
            rebuild_or_log(&mut renderer, &global, data, desired_extent);
        }
    }
}

unsafe fn recover_aborted_acquire(
    renderer: &mut AshRenderer,
    global: &GlobalHandles,
    data: &mut ViewportAshData,
    frame_index: usize,
    desired_extent: Option<vk::Extent2D>,
) {
    data.pending_present = None;
    data.state = ViewportRuntimeState::RebuildRequired;
    // SAFETY: the renderer registry keeps this logical device live and exclusively borrowed while
    // recovery waits for all submitted viewport work to become idle.
    if let Err(error) = unsafe { renderer.device.device_wait_idle() } {
        eprintln!("[ash-mv] device wait failed during frame recovery: {error:?}");
        data.mark_failed();
        return;
    }

    if let Some(frame) = data.frames.get_mut(frame_index) {
        let abandoned_fence = frame.fence;
        if let Some(resources) = data.swapchain.as_mut() {
            for image_fence in &mut resources.images_in_flight {
                if *image_fence == abandoned_fence {
                    *image_fence = vk::Fence::null();
                }
            }
        }
        if let Err(error) = replace_frame_sync(&renderer.device, data.command_pool, frame) {
            eprintln!("[ash-mv] failed to isolate abandoned frame synchronization: {error}");
            data.mark_failed();
            return;
        }
    } else {
        data.mark_failed();
        return;
    }

    if desired_extent.is_none() {
        data.retire_swapchain_after_device_idle(&renderer.device);
    }
    if let Err(error) = recreate_swapchain_after_device_idle(renderer, global, data, desired_extent)
    {
        eprintln!("[ash-mv] swapchain rebuild deferred: {error}");
    }
}

fn fail_viewport(
    renderer: &AshRenderer,
    data: &mut ViewportAshData,
    operation: &str,
    error: vk::Result,
) {
    eprintln!("[ash-mv] {operation} failed; disabling viewport runtime: {error:?}");
    // SAFETY: the callback borrow keeps the renderer and its logical device live until this
    // failure path finishes quiescing outstanding work.
    let _ = unsafe { renderer.device.device_wait_idle() };
    data.mark_failed();
}

/// Renderer: render viewport draw data into its swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub(super) unsafe fn renderer_render_window(viewport: *mut Viewport, _render_arg: *mut c_void) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui owns this live viewport, and registry checks serialize all renderer,
    // viewport-data, and Vulkan command access for this callback.
    unsafe {
        let viewport = &mut *viewport;
        let desired_extent = desired_extent_from_viewport(viewport);
        let attachment_load_op = viewport_attachment_load_op(viewport.flags());
        let draw_data = viewport.draw_data();
        let Some(data) = viewport_user_data_mut(viewport) else {
            if viewport.renderer_user_data().is_null() {
                // `UpdatePlatformWindows()` clears request flags after create callbacks. Reassert
                // the request during rendering so the failed viewport closes on the next frame.
                request_platform_close_after_create_failure(viewport);
            }
            return;
        };
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        if draw_data.is_null() {
            return;
        }
        let draw_data: &dear_imgui_rs::render::DrawData =
            dear_imgui_rs::render::DrawData::from_raw(&*draw_data);
        if data.pending_present.is_some() || data.frames.is_empty() {
            return;
        }
        if desired_extent.is_none() && data.state == ViewportRuntimeState::Active {
            rebuild_or_log(&mut renderer, &global, data, None);
        }
        if data.state == ViewportRuntimeState::Paused && desired_extent.is_none() {
            return;
        }
        if !data.state.can_acquire() || data.swapchain.is_none() {
            if data.state == ViewportRuntimeState::Failed {
                return;
            }
            rebuild_or_log(&mut renderer, &global, data, desired_extent);
            if !data.state.can_acquire() {
                return;
            }
        }

        let frame_index = data.frame_index % data.frames.len();
        let frame_fence = data.frames[frame_index].fence;
        let command_buffer = data.frames[frame_index].command_buffer;
        let image_available = data.frames[frame_index].image_available;
        if let Err(error) = renderer
            .device
            .wait_for_fences(&[frame_fence], true, u64::MAX)
        {
            fail_viewport(&renderer, data, "wait_for_fences", error);
            return;
        }

        let Some(resources) = data.swapchain.as_ref() else {
            data.state = ViewportRuntimeState::RebuildRequired;
            return;
        };
        let swapchain = resources.swapchain;
        let format = resources.format;
        #[cfg(not(feature = "dynamic-rendering"))]
        let (pipeline, render_pass) = match renderer.viewport_pipeline(format) {
            Ok(pipeline) => (pipeline.pipeline, pipeline.render_pass(attachment_load_op)),
            Err(error) => {
                eprintln!("[ash-mv] viewport pipeline error: {error}");
                data.mark_failed();
                return;
            }
        };
        #[cfg(feature = "dynamic-rendering")]
        let pipeline = match renderer.viewport_pipeline(format) {
            Ok(pipeline) => pipeline.pipeline,
            Err(error) => {
                eprintln!("[ash-mv] viewport pipeline error: {error}");
                data.mark_failed();
                return;
            }
        };
        let gamma = renderer.gamma_for_format(format);

        let (image_index, suboptimal) = match data.swapchain_loader.acquire_next_image(
            swapchain,
            u64::MAX,
            image_available,
            vk::Fence::null(),
        ) {
            Ok(acquired) => acquired,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                rebuild_or_log(&mut renderer, &global, data, desired_extent);
                return;
            }
            Err(error) => {
                fail_viewport(&renderer, data, "acquire_next_image", error);
                return;
            }
        };
        data.rebuild_after_present |= suboptimal;
        let image_index_usize = image_index as usize;
        let Some(resources) = data.swapchain.as_ref() else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        let Some(image_fence) = resources.images_in_flight.get(image_index_usize).copied() else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        let Some(present_semaphore) =
            present_semaphore_for_image(&resources.present_semaphores, image_index)
        else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        #[cfg(feature = "dynamic-rendering")]
        let Some(image) = resources.images.get(image_index_usize).copied() else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        #[cfg(feature = "dynamic-rendering")]
        let Some(image_view) = resources.image_views.get(image_index_usize).copied() else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        let extent = resources.extent;
        #[cfg(not(feature = "dynamic-rendering"))]
        let Some(framebuffer) = resources.framebuffers.get(image_index_usize).copied() else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        #[cfg(feature = "dynamic-rendering")]
        let old_layout = resources
            .image_layouts
            .get(image_index_usize)
            .copied()
            .unwrap_or(vk::ImageLayout::UNDEFINED);

        if image_fence != vk::Fence::null()
            && renderer
                .device
                .wait_for_fences(&[image_fence], true, u64::MAX)
                .is_err()
        {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        }
        if renderer
            .device
            .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
            .is_err()
        {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        }
        let Some(mesh) = data.mesh_frames.next() else {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        };
        if renderer
            .device
            .begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .is_err()
        {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        }

        #[cfg(not(feature = "dynamic-rendering"))]
        {
            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: renderer.viewport_clear_color(),
                },
            }];
            let clear_values: &[vk::ClearValue] =
                if attachment_load_op == vk::AttachmentLoadOp::CLEAR {
                    &clear_values
                } else {
                    &[]
                };
            renderer.device.cmd_begin_render_pass(
                command_buffer,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(render_pass)
                    .framebuffer(framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent,
                    })
                    .clear_values(clear_values),
                vk::SubpassContents::INLINE,
            );
            if let Err(error) =
                renderer.cmd_draw_with_mesh(command_buffer, draw_data, pipeline, gamma, mesh)
            {
                eprintln!("[ash-mv] draw command recording failed: {error}");
                renderer.device.cmd_end_render_pass(command_buffer);
                recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
                return;
            }
            renderer.device.cmd_end_render_pass(command_buffer);
        }

        #[cfg(feature = "dynamic-rendering")]
        {
            transition_swapchain_image(
                &renderer.device,
                command_buffer,
                image,
                old_layout,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            let clear_value = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: renderer.viewport_clear_color(),
                },
            };
            let color_attachment = vk::RenderingAttachmentInfo::default()
                .image_view(image_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(attachment_load_op)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear_value);
            renderer.device.cmd_begin_rendering(
                command_buffer,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_attachment)),
            );
            if let Err(error) =
                renderer.cmd_draw_with_mesh(command_buffer, draw_data, pipeline, gamma, mesh)
            {
                eprintln!("[ash-mv] draw command recording failed: {error}");
                renderer.device.cmd_end_rendering(command_buffer);
                recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
                return;
            }
            renderer.device.cmd_end_rendering(command_buffer);
            transition_swapchain_image(
                &renderer.device,
                command_buffer,
                image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
        }

        if renderer.device.end_command_buffer(command_buffer).is_err() {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        }
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&command_buffer))
            .signal_semaphores(std::slice::from_ref(&present_semaphore));
        if renderer.device.reset_fences(&[frame_fence]).is_err()
            || renderer
                .device
                .queue_submit(
                    renderer.queue,
                    std::slice::from_ref(&submit_info),
                    frame_fence,
                )
                .is_err()
        {
            recover_aborted_acquire(&mut renderer, &global, data, frame_index, desired_extent);
            return;
        }

        let Some(resources) = data.swapchain.as_mut() else {
            data.mark_failed();
            return;
        };
        resources.images_in_flight[image_index_usize] = frame_fence;
        #[cfg(feature = "dynamic-rendering")]
        {
            resources.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        data.frame_index = (data.frame_index + 1) % data.frames.len();
        data.pending_present = Some(image_index);
    }
}

/// Renderer: present frame for viewport swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn renderer_swap_buffers(viewport: *mut Viewport, _render_arg: *mut c_void) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui owns this live viewport, and registry checks serialize the renderer and
    // pending-present state before Vulkan presentation or resource rebuild.
    unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let viewport = &mut *viewport;
        let desired_extent = desired_extent_from_viewport(viewport);
        let Some(data) = viewport_user_data_mut(viewport) else {
            return;
        };
        let Some(image_index) = data.pending_present.take() else {
            return;
        };
        if !data.state.can_acquire() {
            return;
        }
        let Some(resources) = data.swapchain.as_ref() else {
            data.state = ViewportRuntimeState::RebuildRequired;
            return;
        };
        let Some(present_semaphore) =
            present_semaphore_for_image(&resources.present_semaphores, image_index)
        else {
            data.mark_failed();
            return;
        };
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&present_semaphore))
            .swapchains(std::slice::from_ref(&resources.swapchain))
            .image_indices(std::slice::from_ref(&image_index));
        let present = data
            .swapchain_loader
            .queue_present(global.present_queue, &present_info);
        match present {
            Ok(suboptimal) if suboptimal || data.rebuild_after_present => {
                rebuild_or_log(&mut renderer, &global, data, desired_extent);
            }
            Ok(_) => {}
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                rebuild_or_log(&mut renderer, &global, data, desired_extent);
            }
            Err(error) => fail_viewport(&renderer, data, "queue_present", error),
        }
    }
}

fn run_callback(name: &str, callback: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(binding) = binding_for_current_context() else {
            return;
        };
        let _ = binding.try_with_bound_context(callback);
    }));
    if result.is_err() {
        eprintln!("[ash-mv] panic in {name}");
        std::process::abort();
    }
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_create_window_sys(viewport: *mut sys::ImGuiViewport) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui supplied a live viewport for the current context and the null check above
    // makes the cast valid for the duration of this callback.
    run_callback("Renderer_CreateWindow", || unsafe {
        renderer_create_window(viewport.cast());
    });
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_destroy_window_sys(viewport: *mut sys::ImGuiViewport) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui supplied a live viewport for the current context and the null check above
    // makes the cast valid for the duration of this callback.
    run_callback("Renderer_DestroyWindow", || unsafe {
        renderer_destroy_window(viewport.cast());
    });
}

/// # Safety
///
/// Called by the repository-owned C++ aggregate ABI hook with a valid viewport and size pointer.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_set_window_size_sys(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    if viewport.is_null() || size.is_null() {
        return;
    }
    // SAFETY: the aggregate ABI hook guarantees both pointers remain live for this call; both were
    // checked before dereferencing or casting.
    run_callback("Renderer_SetWindowSize", || unsafe {
        renderer_set_window_size(viewport.cast(), *size);
    });
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut sys::ImGuiViewport,
    argument: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui supplied a live viewport for the current context and the null check above
    // makes the cast valid for the duration of this callback.
    run_callback("Renderer_RenderWindow", || unsafe {
        renderer_render_window(viewport.cast(), argument);
    });
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_swap_buffers_sys(
    viewport: *mut sys::ImGuiViewport,
    argument: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }
    // SAFETY: Dear ImGui supplied a live viewport for the current context and the null check above
    // makes the cast valid for the duration of this callback.
    run_callback("Renderer_SwapBuffers", || unsafe {
        renderer_swap_buffers(viewport.cast(), argument);
    });
}
