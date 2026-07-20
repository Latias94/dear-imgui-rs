use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::{PlatformIo, Viewport};
use dear_imgui_rs::{BackendFlags, Context};

use super::registry::{
    current_context, register_viewport_data, runtime_for_context, take_viewport_data,
    take_viewport_data_from_viewport, viewport_user_data_mut, with_current_runtime,
};
use super::runtime::{AshViewportError, RuntimeControl};
use super::*;

pub(super) fn unary_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    expected: unsafe extern "C" fn(*mut sys::ImGuiViewport),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

pub(super) fn render_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    expected: unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn callback_name_for_occupied_slot(platform_io: &PlatformIo) -> Option<&'static str> {
    [
        (
            platform_io.renderer_create_window_raw().is_some(),
            "Renderer_CreateWindow",
        ),
        (
            platform_io.renderer_destroy_window_raw().is_some(),
            "Renderer_DestroyWindow",
        ),
        (
            unsafe { (*platform_io.as_raw()).Renderer_SetWindowSize.is_some() },
            "Renderer_SetWindowSize",
        ),
        (
            platform_io.renderer_render_window_raw().is_some(),
            "Renderer_RenderWindow",
        ),
        (
            platform_io.renderer_swap_buffers_raw().is_some(),
            "Renderer_SwapBuffers",
        ),
    ]
    .into_iter()
    .find_map(|(occupied, name)| occupied.then_some(name))
}

fn first_callback_drift(platform_io: &PlatformIo) -> Option<&'static str> {
    if !unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) {
        return Some("Renderer_CreateWindow");
    }
    if !unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) {
        return Some("Renderer_DestroyWindow");
    }
    if !platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
    {
        return Some("Renderer_SetWindowSize");
    }
    if !render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render_window_sys,
    ) {
        return Some("Renderer_RenderWindow");
    }
    (!render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_swap_buffers_sys,
    ))
    .then_some("Renderer_SwapBuffers")
}

pub(super) fn validate_secondary_viewports(
    states: &[(bool, *mut c_void)],
) -> Result<(), AshViewportError> {
    if states.iter().any(|(_, slot)| !slot.is_null()) {
        Err(AshViewportError::RendererUserDataOccupied)
    } else if states.iter().any(|(created, _)| *created) {
        Err(AshViewportError::PlatformWindowsAlreadyCreated)
    } else {
        Ok(())
    }
}

pub(super) fn preflight_callbacks(context: &Context) -> Result<(), AshViewportError> {
    let binding = context.binding();
    binding.with_bound_context(|| {
        if !context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
        {
            return Err(AshViewportError::PlatformBackendUnavailable);
        }
        if !sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
            return Err(AshViewportError::AggregateCallbackHooksUnavailable);
        }

        let platform_io = context.platform_io();
        let raw = unsafe { &*platform_io.as_raw() };
        for (available, callback) in [
            (raw.Platform_CreateWindow.is_some(), "Platform_CreateWindow"),
            (
                raw.Platform_DestroyWindow.is_some(),
                "Platform_DestroyWindow",
            ),
        ] {
            if !available {
                return Err(AshViewportError::PlatformCallbackUnavailable { callback });
            }
        }
        if let Some(callback) = callback_name_for_occupied_slot(platform_io) {
            return Err(AshViewportError::RendererCallbackOccupied { callback });
        }

        let secondary_viewports = platform_io
            .viewports_iter()
            .skip(1)
            .map(|viewport| {
                (
                    viewport.platform_window_created(),
                    viewport.renderer_user_data(),
                )
            })
            .collect::<Vec<_>>();
        validate_secondary_viewports(&secondary_viewports)
    })
}

pub(super) fn claim_callbacks(control: &RuntimeControl, context: &mut Context) {
    let binding = context.binding();
    binding.with_bound_context(|| {
        let platform_io = context.platform_io_mut();
        // SAFETY: these callbacks use the exact sys ABI and remain installed until the runtime
        // quiesces and destroys all registered Vulkan viewport data.
        unsafe {
            platform_io.set_renderer_create_window_raw(Some(renderer_create_window_sys));
            platform_io.set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
            platform_io.set_renderer_set_window_size_raw(Some(renderer_set_window_size_sys));
            platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
            platform_io.set_renderer_swap_buffers_raw(Some(renderer_swap_buffers_sys));
        }
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);
    });
    control.mark_callback_claimed();
}

pub(super) fn detect_callback_drift(control: &RuntimeControl) {
    if !control.should_detect_callback_drift() {
        return;
    }
    let result = control.binding().try_with_bound_context(|| {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return Some("Renderer_CreateWindow");
        }
        let platform_io = unsafe { PlatformIo::from_raw(platform_io) };
        first_callback_drift(platform_io)
    });
    if let Ok(Some(callback)) = result {
        control.record_callback_replaced(callback);
    }
}

pub(super) fn release_callbacks(control: &RuntimeControl) -> Result<(), AshViewportError> {
    if control.callback_released() {
        return Ok(());
    }
    if current_context() != control.context_raw() {
        return Err(AshViewportError::BoundContextMismatch {
            expected: control.binding().id(),
        });
    }
    let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
    if platform_io.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "release renderer callbacks",
        });
    }
    let platform_io = unsafe { PlatformIo::from_raw_mut(platform_io) };
    let drift = first_callback_drift(platform_io);

    // SAFETY: each slot is cleared only when it still contains this runtime's callback, after the
    // runtime has stopped accepting new callback work.
    unsafe {
        if unary_callback_matches(
            platform_io.renderer_create_window_raw(),
            renderer_create_window_sys,
        ) {
            platform_io.set_renderer_create_window_raw(None);
        }
        if unary_callback_matches(
            platform_io.renderer_destroy_window_raw(),
            renderer_destroy_window_sys,
        ) {
            platform_io.set_renderer_destroy_window_raw(None);
        }
        platform_io
            .clear_renderer_set_window_size_if_pointer_callback(renderer_set_window_size_sys);
        if render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_render_window_sys,
        ) {
            platform_io.set_renderer_render_window_raw(None);
        }
        if render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_swap_buffers_sys,
        ) {
            platform_io.set_renderer_swap_buffers_raw(None);
        }
    }

    if platform_io.renderer_callbacks_are_empty() {
        let io = unsafe { sys::igGetIO_Nil() };
        if !io.is_null() {
            let bit = BackendFlags::RENDERER_HAS_VIEWPORTS.bits();
            unsafe {
                (*io).BackendFlags =
                    ((*io).BackendFlags & !bit) | (control.prior_backend_flags().bits() & bit);
            }
        }
    }
    control.mark_callback_released();
    drift.map_or(Ok(()), |callback| {
        Err(AshViewportError::RendererCallbackReplaced { callback })
    })
}

pub(super) fn destroy_renderer_viewport_resources(
    control: &RuntimeControl,
) -> Result<(), AshViewportError> {
    if current_context() != control.context_raw() {
        return Err(AshViewportError::BoundContextMismatch {
            expected: control.binding().id(),
        });
    }
    let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
    if platform_io.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "release viewport resources",
        });
    }

    control.with_renderer_teardown(|renderer, globals| {
        control.wait_device_idle(renderer, "viewport runtime shutdown")?;
        let surface_loader = khr_surface::Instance::new(&globals.entry, &globals.instance);
        let platform_io = unsafe { PlatformIo::from_raw_mut(platform_io) };
        let mut ownership_fault = None;
        for viewport in platform_io.viewports_iter_mut() {
            let pointer = viewport.renderer_user_data();
            let Some(data) =
                (unsafe { take_viewport_data_from_viewport(control.context_raw(), viewport) })
            else {
                if !pointer.is_null() && ownership_fault.is_none() {
                    ownership_fault = Some(AshViewportError::RendererUserDataOwnershipLost {
                        callback: "viewport runtime shutdown",
                    });
                }
                continue;
            };
            data.destroy_after_device_idle(renderer, &surface_loader)?;
        }
        for data in take_viewport_data(control.binding().id()) {
            data.destroy_after_device_idle(renderer, &surface_loader)?;
        }
        ownership_fault.map_or(Ok(()), Err)
    })?;
    Ok(())
}

pub(super) fn publish_registered_box<T>(
    mut value: Box<T>,
    register: impl FnOnce(*mut T) -> Result<(), AshViewportError>,
    publish: impl FnOnce(*mut T),
) -> Result<(), (AshViewportError, Box<T>)> {
    let pointer = std::ptr::from_mut(value.as_mut());
    if let Err(error) = register(pointer) {
        return Err((error, value));
    }
    let owned_pointer = Box::into_raw(value);
    debug_assert_eq!(owned_pointer, pointer);
    publish(owned_pointer);
    Ok(())
}

pub(super) fn publish_registered_box_transactionally<T>(
    value: Box<T>,
    register: impl FnOnce(*mut T) -> Result<(), AshViewportError>,
    publish: impl FnOnce(*mut T),
    cleanup: impl FnOnce(Box<T>) -> Result<(), AshViewportError>,
) -> Result<(), AshViewportError> {
    match publish_registered_box(value, register, publish) {
        Ok(()) => Ok(()),
        Err((registration_error, value)) => {
            cleanup(value)?;
            Err(registration_error)
        }
    }
}

pub(super) fn request_platform_close_after_create_failure(viewport: &mut Viewport) {
    viewport.set_platform_request_close(true);
}

fn renderer_data_pointer(
    control: &RuntimeControl,
    viewport: &mut Viewport,
    callback: &'static str,
) -> Result<*mut ViewportAshData, AshViewportError> {
    let data =
        unsafe { viewport_user_data_mut(control.context_raw(), viewport) }.map(std::ptr::from_mut);
    match data {
        Some(data) => Ok(data),
        None if viewport.renderer_user_data().is_null() => {
            Err(AshViewportError::InvalidCallbackArgument { callback })
        }
        None => Err(AshViewportError::RendererUserDataOwnershipLost { callback }),
    }
}

unsafe fn renderer_create_window(
    control: &RuntimeControl,
    viewport: *mut Viewport,
) -> Result<(), AshViewportError> {
    if viewport.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "Renderer_CreateWindow",
        });
    }
    let viewport = unsafe { &mut *viewport };
    if !viewport.renderer_user_data().is_null() {
        request_platform_close_after_create_failure(viewport);
        return Err(AshViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_CreateWindow",
        });
    }

    let result = control.with_renderer_callback("Renderer_CreateWindow", |renderer, globals| {
        let surface = unsafe {
            globals
                .surface_adapter
                .create_surface(&globals.entry, &globals.instance, viewport)
        }?;
        let surface_loader = khr_surface::Instance::new(&globals.entry, &globals.instance);
        let swapchain_loader = khr_swapchain::Device::new(&globals.instance, &renderer.device);
        let command_pool =
            match create_command_pool(&renderer.device, globals.graphics_queue_family_index) {
                Ok(command_pool) => command_pool,
                Err(error) => {
                    unsafe { surface_loader.destroy_surface(surface, None) };
                    return Err(error.into());
                }
            };
        let frames =
            match create_frame_syncs(&renderer.device, command_pool, globals.in_flight_frames) {
                Ok(frames) => frames,
                Err(error) => {
                    unsafe {
                        renderer.device.destroy_command_pool(command_pool, None);
                        surface_loader.destroy_surface(surface, None);
                    }
                    return Err(error.into());
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
            mesh_frames: Frames::new(globals.in_flight_frames),
        };
        if let Err(error) = recreate_swapchain(
            renderer,
            globals,
            &mut data,
            desired_extent_from_viewport(viewport),
        ) {
            data.destroy_after_device_idle(renderer, &surface_loader)?;
            return Err(error.into());
        }

        publish_registered_box_transactionally(
            Box::new(data),
            |pointer| register_viewport_data(control.binding(), pointer),
            |pointer| unsafe { viewport.set_renderer_user_data(pointer.cast()) },
            |data| {
                data.destroy_after_device_idle(renderer, &surface_loader)?;
                Ok(())
            },
        )
    });
    if result.is_err() {
        request_platform_close_after_create_failure(viewport);
    }
    result
}

unsafe fn renderer_destroy_window(
    control: &RuntimeControl,
    viewport: *mut Viewport,
) -> Result<(), AshViewportError> {
    if viewport.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "Renderer_DestroyWindow",
        });
    }
    let viewport = unsafe { &mut *viewport };
    if viewport.renderer_user_data().is_null() {
        return Ok(());
    }
    let pointer = renderer_data_pointer(control, viewport, "Renderer_DestroyWindow")?;
    control.with_renderer_callback("Renderer_DestroyWindow", |renderer, globals| {
        control.wait_device_idle(renderer, "Renderer_DestroyWindow")?;
        let Some(data) =
            (unsafe { take_viewport_data_from_viewport(control.context_raw(), viewport) })
        else {
            return Err(AshViewportError::RendererUserDataOwnershipLost {
                callback: "Renderer_DestroyWindow",
            });
        };
        debug_assert_eq!(std::ptr::from_ref(data.as_ref()), pointer.cast_const());
        let surface_loader = khr_surface::Instance::new(&globals.entry, &globals.instance);
        data.destroy_after_device_idle(renderer, &surface_loader)?;
        Ok(())
    })
}

fn rebuild_viewport(
    renderer: &mut AshRenderer,
    globals: &GlobalHandles,
    data: &mut ViewportAshData,
    desired_extent: Option<vk::Extent2D>,
) -> Result<(), AshViewportError> {
    recreate_swapchain(renderer, globals, data, desired_extent).map_err(Into::into)
}

unsafe fn renderer_set_window_size(
    control: &RuntimeControl,
    viewport: *mut Viewport,
    size: sys::ImVec2,
) -> Result<(), AshViewportError> {
    if viewport.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "Renderer_SetWindowSize",
        });
    }
    let viewport = unsafe { &mut *viewport };
    let data = renderer_data_pointer(control, viewport, "Renderer_SetWindowSize")?;
    let desired_extent = desired_extent_from_imvec2(size, viewport.framebuffer_scale());
    control.with_renderer_callback("Renderer_SetWindowSize", |renderer, globals| {
        let data = unsafe { &mut *data };
        if data.state == ViewportRuntimeState::Failed {
            return Ok(());
        }
        rebuild_viewport(renderer, globals, data, desired_extent)
    })
}

fn recover_aborted_acquire(
    control: &RuntimeControl,
    renderer: &mut AshRenderer,
    globals: &GlobalHandles,
    data: &mut ViewportAshData,
    frame_index: usize,
    desired_extent: Option<vk::Extent2D>,
) -> Result<(), AshViewportError> {
    data.pending_present = None;
    data.state = ViewportRuntimeState::RebuildRequired;
    control.wait_device_idle(renderer, "recover aborted viewport acquire")?;

    let Some(frame) = data.frames.get_mut(frame_index) else {
        data.mark_failed();
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "recover viewport frame",
        });
    };
    let abandoned_fence = frame.fence;
    if let Some(resources) = data.swapchain.as_mut() {
        for image_fence in &mut resources.images_in_flight {
            if *image_fence == abandoned_fence {
                *image_fence = vk::Fence::null();
            }
        }
    }
    replace_frame_sync(&renderer.device, data.command_pool, frame)?;

    if desired_extent.is_none() {
        data.retire_swapchain_after_device_idle(&renderer.device);
    }
    recreate_swapchain_after_device_idle(renderer, globals, data, desired_extent)?;
    Ok(())
}

unsafe fn renderer_render_window(
    control: &RuntimeControl,
    viewport: *mut Viewport,
    _render_arg: *mut c_void,
) -> Result<(), AshViewportError> {
    if viewport.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "Renderer_RenderWindow",
        });
    }
    let viewport = unsafe { &mut *viewport };
    let desired_extent = desired_extent_from_viewport(viewport);
    let attachment_load_op = viewport_attachment_load_op(viewport.flags());
    let draw_data = viewport.draw_data();
    let data = match renderer_data_pointer(control, viewport, "Renderer_RenderWindow") {
        Ok(data) => data,
        Err(AshViewportError::InvalidCallbackArgument { .. }) => {
            request_platform_close_after_create_failure(viewport);
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if draw_data.is_null() {
        return Ok(());
    }
    let draw_data: &dear_imgui_rs::render::DrawData =
        unsafe { dear_imgui_rs::render::DrawData::from_raw(&*draw_data) };

    control.with_renderer_callback("Renderer_RenderWindow", |renderer, globals| {
        let data = unsafe { &mut *data };
        if data.pending_present.is_some() || data.frames.is_empty() {
            return Ok(());
        }
        if desired_extent.is_none() && data.state == ViewportRuntimeState::Active {
            rebuild_viewport(renderer, globals, data, None)?;
        }
        if data.state == ViewportRuntimeState::Paused && desired_extent.is_none() {
            return Ok(());
        }
        if !data.state.can_acquire() || data.swapchain.is_none() {
            if data.state == ViewportRuntimeState::Failed {
                return Ok(());
            }
            rebuild_viewport(renderer, globals, data, desired_extent)?;
            if !data.state.can_acquire() {
                return Ok(());
            }
        }

        let frame_index = data.frame_index % data.frames.len();
        let frame_fence = data.frames[frame_index].fence;
        let command_buffer = data.frames[frame_index].command_buffer;
        let image_available = data.frames[frame_index].image_available;
        unsafe {
            renderer
                .device
                .wait_for_fences(&[frame_fence], true, u64::MAX)
        }
        .map_err(RendererError::from)?;

        let Some(resources) = data.swapchain.as_ref() else {
            data.state = ViewportRuntimeState::RebuildRequired;
            return Ok(());
        };
        let swapchain = resources.swapchain;
        let format = resources.format;
        #[cfg(not(feature = "dynamic-rendering"))]
        let (pipeline, render_pass) = {
            let pipeline = renderer.viewport_pipeline(format)?;
            (pipeline.pipeline, pipeline.render_pass(attachment_load_op))
        };
        #[cfg(feature = "dynamic-rendering")]
        let pipeline = renderer.viewport_pipeline(format)?.pipeline;
        let gamma = renderer.gamma_for_format(format);

        let (image_index, suboptimal) = match unsafe {
            data.swapchain_loader.acquire_next_image(
                swapchain,
                u64::MAX,
                image_available,
                vk::Fence::null(),
            )
        } {
            Ok(acquired) => acquired,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                rebuild_viewport(renderer, globals, data, desired_extent)?;
                return Ok(());
            }
            Err(error) => {
                data.mark_failed();
                return Err(RendererError::from(error).into());
            }
        };
        data.rebuild_after_present |= suboptimal;
        let image_index_usize = image_index as usize;
        let Some(resources) = data.swapchain.as_ref() else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        let Some(image_fence) = resources.images_in_flight.get(image_index_usize).copied() else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        let Some(present_semaphore) =
            present_semaphore_for_image(&resources.present_semaphores, image_index)
        else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        #[cfg(feature = "dynamic-rendering")]
        let Some(image) = resources.images.get(image_index_usize).copied() else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        #[cfg(feature = "dynamic-rendering")]
        let Some(image_view) = resources.image_views.get(image_index_usize).copied() else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        let extent = resources.extent;
        #[cfg(not(feature = "dynamic-rendering"))]
        let Some(framebuffer) = resources.framebuffers.get(image_index_usize).copied() else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        #[cfg(feature = "dynamic-rendering")]
        let old_layout = resources
            .image_layouts
            .get(image_index_usize)
            .copied()
            .unwrap_or(vk::ImageLayout::UNDEFINED);

        if image_fence != vk::Fence::null() {
            unsafe {
                renderer
                    .device
                    .wait_for_fences(&[image_fence], true, u64::MAX)
            }
            .map_err(RendererError::from)?;
        }
        unsafe {
            renderer
                .device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        }
        .map_err(RendererError::from)?;
        let Some(mesh) = data.mesh_frames.next() else {
            return recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            );
        };
        unsafe {
            renderer.device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        }
        .map_err(RendererError::from)?;

        #[cfg(not(feature = "dynamic-rendering"))]
        unsafe {
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
                renderer.device.cmd_end_render_pass(command_buffer);
                recover_aborted_acquire(
                    control,
                    renderer,
                    globals,
                    data,
                    frame_index,
                    desired_extent,
                )?;
                return Err(error.into());
            }
            renderer.device.cmd_end_render_pass(command_buffer);
        }

        #[cfg(feature = "dynamic-rendering")]
        unsafe {
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
                renderer.device.cmd_end_rendering(command_buffer);
                recover_aborted_acquire(
                    control,
                    renderer,
                    globals,
                    data,
                    frame_index,
                    desired_extent,
                )?;
                return Err(error.into());
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

        unsafe { renderer.device.end_command_buffer(command_buffer) }
            .map_err(RendererError::from)?;
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&command_buffer))
            .signal_semaphores(std::slice::from_ref(&present_semaphore));
        if let Err(error) = unsafe { renderer.device.reset_fences(&[frame_fence]) } {
            recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            )?;
            return Err(RendererError::from(error).into());
        }
        if let Err(error) = unsafe {
            renderer.device.queue_submit(
                renderer.queue,
                std::slice::from_ref(&submit_info),
                frame_fence,
            )
        } {
            recover_aborted_acquire(
                control,
                renderer,
                globals,
                data,
                frame_index,
                desired_extent,
            )?;
            return Err(RendererError::from(error).into());
        }

        let Some(resources) = data.swapchain.as_mut() else {
            data.mark_failed();
            return Err(AshViewportError::InvalidCallbackArgument {
                callback: "publish viewport submission",
            });
        };
        resources.images_in_flight[image_index_usize] = frame_fence;
        #[cfg(feature = "dynamic-rendering")]
        {
            resources.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        data.frame_index = (data.frame_index + 1) % data.frames.len();
        data.pending_present = Some(image_index);
        Ok(())
    })
}

unsafe fn renderer_swap_buffers(
    control: &RuntimeControl,
    viewport: *mut Viewport,
    _render_arg: *mut c_void,
) -> Result<(), AshViewportError> {
    if viewport.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "Renderer_SwapBuffers",
        });
    }
    let viewport = unsafe { &mut *viewport };
    let desired_extent = desired_extent_from_viewport(viewport);
    let data = renderer_data_pointer(control, viewport, "Renderer_SwapBuffers")?;
    control.with_renderer_callback("Renderer_SwapBuffers", |renderer, globals| {
        let data = unsafe { &mut *data };
        let Some(image_index) = data.pending_present.take() else {
            return Ok(());
        };
        if !data.state.can_acquire() {
            return Ok(());
        }
        let Some(resources) = data.swapchain.as_ref() else {
            data.state = ViewportRuntimeState::RebuildRequired;
            return Ok(());
        };
        let Some(present_semaphore) =
            present_semaphore_for_image(&resources.present_semaphores, image_index)
        else {
            data.mark_failed();
            return Err(AshViewportError::InvalidCallbackArgument {
                callback: "Renderer_SwapBuffers image index",
            });
        };
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&present_semaphore))
            .swapchains(std::slice::from_ref(&resources.swapchain))
            .image_indices(std::slice::from_ref(&image_index));
        match unsafe {
            data.swapchain_loader
                .queue_present(globals.present_queue, &present_info)
        } {
            Ok(suboptimal) if suboptimal || data.rebuild_after_present => {
                rebuild_viewport(renderer, globals, data, desired_extent)
            }
            Ok(_) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                rebuild_viewport(renderer, globals, data, desired_extent)
            }
            Err(error) => {
                data.mark_failed();
                Err(RendererError::from(error).into())
            }
        }
    })
}

fn run_callback(
    callback_name: &'static str,
    callback: impl FnOnce(&RuntimeControl) -> Result<(), AshViewportError>,
) {
    let Some(control) = runtime_for_context(current_context()) else {
        return;
    };
    if !control.is_callback_accessible() {
        return;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_current_runtime(|active| {
            #[cfg(test)]
            active.maybe_panic_callback_for_test();
            callback(active)
        })
    }));
    match result {
        Ok(Some(Err(error))) => control.record_fault(error),
        Ok(Some(Ok(()))) | Ok(None) => {}
        Err(_) => control.record_fault(AshViewportError::CallbackPanicked {
            callback: callback_name,
        }),
    }
}

#[cfg(test)]
pub(super) unsafe extern "C" fn renderer_probe_runtime_sys() {
    run_callback(
        "Renderer_Probe",
        RuntimeControl::probe_renderer_storage_for_test,
    );
}

pub unsafe extern "C" fn renderer_create_window_sys(viewport: *mut sys::ImGuiViewport) {
    run_callback("Renderer_CreateWindow", |control| unsafe {
        renderer_create_window(control, viewport.cast())
    });
}

pub unsafe extern "C" fn renderer_destroy_window_sys(viewport: *mut sys::ImGuiViewport) {
    run_callback("Renderer_DestroyWindow", |control| unsafe {
        renderer_destroy_window(control, viewport.cast())
    });
}

pub unsafe extern "C" fn renderer_set_window_size_sys(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    run_callback("Renderer_SetWindowSize", |control| {
        if size.is_null() {
            return Err(AshViewportError::InvalidCallbackArgument {
                callback: "Renderer_SetWindowSize",
            });
        }
        unsafe { renderer_set_window_size(control, viewport.cast(), *size) }
    });
}

pub unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut sys::ImGuiViewport,
    argument: *mut c_void,
) {
    run_callback("Renderer_RenderWindow", |control| unsafe {
        renderer_render_window(control, viewport.cast(), argument)
    });
}

pub unsafe extern "C" fn renderer_swap_buffers_sys(
    viewport: *mut sys::ImGuiViewport,
    argument: *mut c_void,
) {
    run_callback("Renderer_SwapBuffers", |control| unsafe {
        renderer_swap_buffers(control, viewport.cast(), argument)
    });
}
