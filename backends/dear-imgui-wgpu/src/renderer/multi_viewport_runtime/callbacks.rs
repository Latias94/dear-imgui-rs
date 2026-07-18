//! Dear ImGui renderer callback ownership and C ABI entry points.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::{PlatformIo, Viewport};
use dear_imgui_rs::{BackendFlags, Context};

use super::platform_adapter;
use super::registry::{
    current_context, destroy_viewport_data, register_viewport_data, runtime_for_context,
    take_viewport_data, viewport_data_pointer, with_current_runtime,
};
use super::runtime::{RuntimeControl, WgpuViewportError};
use super::surface::{
    SurfaceAction, SurfaceEvent, create_viewport_data, handle_non_renderable_surface_event,
    request_close_after_surface_creation_failure, should_clear_viewport, surface_action,
};

pub(super) fn unary_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)>,
    expected: unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

pub(super) fn render_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, *mut c_void)>,
    expected: unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, *mut c_void),
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
            // SAFETY: PlatformIo owns this callback table for the duration of the borrow.
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
) -> Result<(), WgpuViewportError> {
    if states.iter().any(|(_, slot)| !slot.is_null()) {
        Err(WgpuViewportError::RendererUserDataOccupied)
    } else if states.iter().any(|(created, _)| *created) {
        Err(WgpuViewportError::PlatformWindowsAlreadyCreated)
    } else {
        Ok(())
    }
}

pub(super) fn preflight_callbacks(context: &Context) -> Result<(), WgpuViewportError> {
    let binding = context.binding();
    binding.with_bound_context(|| {
        if !context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
        {
            return Err(WgpuViewportError::PlatformBackendUnavailable);
        }
        if !dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
            return Err(WgpuViewportError::AggregateCallbackHooksUnavailable);
        }

        let platform_io = context.platform_io();
        // WGPU creates its surface after the platform window exists and relies on the platform
        // owner to destroy that window after renderer resources have been released.
        // SAFETY: PlatformIo owns this callback table for the duration of the borrow.
        let raw = unsafe { &*platform_io.as_raw() };
        for (available, callback) in [
            (raw.Platform_CreateWindow.is_some(), "Platform_CreateWindow"),
            (
                raw.Platform_DestroyWindow.is_some(),
                "Platform_DestroyWindow",
            ),
        ] {
            if !available {
                return Err(WgpuViewportError::PlatformCallbackUnavailable { callback });
            }
        }
        if platform_io
            .viewports_iter()
            .next()
            .is_none_or(|viewport| viewport.platform_handle().is_null())
        {
            return Err(WgpuViewportError::MainViewportHandleUnavailable);
        }

        if let Some(callback) = callback_name_for_occupied_slot(platform_io) {
            return Err(WgpuViewportError::RendererCallbackOccupied { callback });
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
        platform_io.set_renderer_create_window_raw(Some(renderer_create_window_sys));
        platform_io.set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
        platform_io.set_renderer_set_window_size_raw(Some(renderer_set_window_size_sys));
        platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
        platform_io.set_renderer_swap_buffers_raw(Some(renderer_swap_buffers_sys));
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
        let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
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

pub(super) fn release_callbacks(control: &RuntimeControl) -> Result<(), WgpuViewportError> {
    if control.callback_released() {
        return Ok(());
    }
    if current_context() != control.context_raw() {
        return Err(WgpuViewportError::BoundContextMismatch {
            expected: control.binding().id(),
        });
    }
    let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
    if platform_io.is_null() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "read PlatformIO during callback release",
        });
    }
    let platform_io = unsafe { PlatformIo::from_raw_mut(platform_io) };
    let drift = first_callback_drift(platform_io);

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
    platform_io.clear_renderer_set_window_size_if_pointer_callback(renderer_set_window_size_sys);
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

    if platform_io.renderer_callbacks_are_empty() {
        let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
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
        Err(WgpuViewportError::RendererCallbackReplaced { callback })
    })
}

pub(super) fn destroy_renderer_viewport_resources(
    control: &RuntimeControl,
) -> Result<(), WgpuViewportError> {
    if current_context() != control.context_raw() {
        return Err(WgpuViewportError::BoundContextMismatch {
            expected: control.binding().id(),
        });
    }
    #[cfg(test)]
    if control.take_viewport_cleanup_failure_for_test() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "injected viewport cleanup failure",
        });
    }
    let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
    if platform_io.is_null() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "read PlatformIO during viewport teardown",
        });
    }
    let platform_io = unsafe { PlatformIo::from_raw_mut(platform_io) };
    for viewport in platform_io.viewports_iter_mut() {
        unsafe {
            destroy_viewport_data(control.context_raw(), viewport);
        }
    }
    for pointer in take_viewport_data(control.binding().id()) {
        // Registry membership is the ownership sidecar for allocations that were detached from
        // their native viewport slot by a foreign callback.
        drop(unsafe { Box::from_raw(pointer) });
    }
    Ok(())
}

pub(super) fn publish_registered_box<T>(
    mut value: Box<T>,
    register: impl FnOnce(*mut T) -> Result<(), WgpuViewportError>,
    publish: impl FnOnce(*mut T),
) -> Result<(), WgpuViewportError> {
    let pointer = std::ptr::from_mut(value.as_mut());
    register(pointer)?;
    let owned_pointer = Box::into_raw(value);
    debug_assert_eq!(owned_pointer, pointer);
    publish(owned_pointer);
    Ok(())
}

fn record_renderer_user_data_drift(
    control: &RuntimeControl,
    callback: &'static str,
    viewport: &Viewport,
) {
    if !viewport.renderer_user_data().is_null() {
        control.record_fault(WgpuViewportError::RendererUserDataOwnershipLost { callback });
    }
}

pub(super) unsafe fn renderer_create_window(control: &RuntimeControl, viewport: *mut Viewport) {
    if viewport.is_null() {
        control.record_fault(WgpuViewportError::InvalidViewport {
            callback: "Renderer_CreateWindow",
        });
        return;
    }
    let viewport = unsafe { &mut *viewport };
    if !viewport.renderer_user_data().is_null() {
        record_renderer_user_data_drift(control, "Renderer_CreateWindow", viewport);
        request_close_after_surface_creation_failure(viewport);
        return;
    }
    let Some(globals) = control.globals() else {
        control.record_fault(WgpuViewportError::RuntimeDetached);
        request_close_after_surface_creation_failure(viewport);
        return;
    };
    match unsafe { create_viewport_data(control.context_raw(), viewport, &globals) } {
        Ok(data) => {
            if let Err(error) = publish_registered_box(
                Box::new(data),
                |pointer| register_viewport_data(control.binding(), pointer),
                |pointer| viewport.set_renderer_user_data(pointer.cast()),
            ) {
                control.record_fault(error);
                request_close_after_surface_creation_failure(viewport);
            }
        }
        Err(error) => {
            control.record_fault(error);
            request_close_after_surface_creation_failure(viewport);
        }
    }
}

pub(super) unsafe fn renderer_destroy_window(control: &RuntimeControl, viewport: *mut Viewport) {
    if viewport.is_null() {
        control.record_fault(WgpuViewportError::InvalidViewport {
            callback: "Renderer_DestroyWindow",
        });
        return;
    }
    let viewport = unsafe { &mut *viewport };
    if !(unsafe { destroy_viewport_data(control.context_raw(), viewport) }) {
        record_renderer_user_data_drift(control, "Renderer_DestroyWindow", viewport);
    }
}

pub(super) unsafe fn renderer_set_window_size(
    control: &RuntimeControl,
    viewport: *mut Viewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if viewport.is_null() {
        control.record_fault(WgpuViewportError::InvalidViewport {
            callback: "Renderer_SetWindowSize",
        });
        return;
    }
    let viewport = unsafe { &mut *viewport };
    let Some(pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        record_renderer_user_data_drift(control, "Renderer_SetWindowSize", viewport);
        return;
    };
    let pixels = super::logical_size_to_framebuffer([size.x, size.y], viewport.framebuffer_scale());
    let data = unsafe { &mut *pointer };
    if data.config.width != pixels[0] || data.config.height != pixels[1] {
        data.config.width = pixels[0];
        data.config.height = pixels[1];
        data.surface.configure(&data.device, &data.config);
    }
}

pub(super) unsafe fn renderer_render_window(control: &RuntimeControl, viewport: *mut Viewport) {
    if viewport.is_null() {
        control.record_fault(WgpuViewportError::InvalidViewport {
            callback: "Renderer_RenderWindow",
        });
        return;
    }
    let viewport = unsafe { &mut *viewport };
    let Some(data_pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        if viewport.renderer_user_data().is_null() {
            request_close_after_surface_creation_failure(viewport);
        } else {
            record_renderer_user_data_drift(control, "Renderer_RenderWindow", viewport);
        }
        return;
    };
    control.with_renderer_callback("Renderer_RenderWindow", |renderer, globals| {
        let backend = renderer
            .backend_data
            .as_ref()
            .ok_or(WgpuViewportError::RendererNotInitialized)?;
        let device = backend.device.clone();
        let queue = backend.queue.clone();
        let raw_draw_data = viewport.draw_data();
        if raw_draw_data.is_null() {
            return Ok(());
        }
        let data = unsafe { &mut *data_pointer };
        let draw_data = unsafe { dear_imgui_rs::render::DrawData::from_raw(&*raw_draw_data) };

        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
        let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (
                frame,
                surface_action(SurfaceEvent::Success) == SurfaceAction::RenderThenReconfigure,
            ),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (
                frame,
                surface_action(SurfaceEvent::Suboptimal) == SurfaceAction::RenderThenReconfigure,
            ),
            wgpu::CurrentSurfaceTexture::Outdated => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Outdated,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                unsafe {
                    handle_non_renderable_surface_event(SurfaceEvent::Lost, viewport, data, globals)
                }?;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Timeout,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Occluded,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Validation,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
        };

        #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
        let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
            Ok(frame) => {
                let event = if frame.suboptimal {
                    SurfaceEvent::Suboptimal
                } else {
                    SurfaceEvent::Success
                };
                (
                    frame,
                    surface_action(event) == SurfaceAction::RenderThenReconfigure,
                )
            }
            Err(wgpu::SurfaceError::Outdated) => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Outdated,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
            Err(wgpu::SurfaceError::Lost) => {
                unsafe {
                    handle_non_renderable_surface_event(SurfaceEvent::Lost, viewport, data, globals)
                }?;
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Timeout,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::OutOfMemory,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
            Err(wgpu::SurfaceError::Other) => {
                unsafe {
                    handle_non_renderable_surface_event(
                        SurfaceEvent::Other,
                        viewport,
                        data,
                        globals,
                    )
                }?;
                return Ok(());
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dear-imgui-wgpu::viewport-encoder"),
        });
        {
            let load = if should_clear_viewport(viewport.flags()) {
                wgpu::LoadOp::Clear(renderer.viewport_clear_color())
            } else {
                wgpu::LoadOp::Load
            };
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dear-imgui-wgpu::viewport-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                multiview_mask: None,
                timestamp_writes: None,
            });
            renderer.render_read_only_draw_data_with_fb_size(
                draw_data,
                &mut render_pass,
                data.config.width,
                data.config.height,
                false,
                unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() },
            )?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        data.pending_frame = Some(frame);
        data.pending_reconfigure = reconfigure_after_present;
        Ok(())
    });
}

pub(super) unsafe fn renderer_swap_buffers(control: &RuntimeControl, viewport: *mut Viewport) {
    if viewport.is_null() {
        control.record_fault(WgpuViewportError::InvalidViewport {
            callback: "Renderer_SwapBuffers",
        });
        return;
    }
    let viewport = unsafe { &mut *viewport };
    let Some(pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        record_renderer_user_data_drift(control, "Renderer_SwapBuffers", viewport);
        return;
    };
    let data = unsafe { &mut *pointer };
    let Some(frame) = data.pending_frame.take() else {
        return;
    };
    #[cfg(feature = "wgpu-30")]
    data.queue.present(frame);
    #[cfg(not(feature = "wgpu-30"))]
    frame.present();
    let refreshed_size = framebuffer_size_for_reconfigure(data.pending_reconfigure, || unsafe {
        platform_adapter::framebuffer_size(viewport)
    });
    if data.pending_reconfigure {
        let size = match refreshed_size {
            Ok(Some(size)) => size,
            Ok(None) => return,
            Err(error) => {
                control.record_fault(error);
                return;
            }
        };
        data.config.width = size[0].max(1);
        data.config.height = size[1].max(1);
        data.surface.configure(&data.device, &data.config);
        data.pending_reconfigure = false;
    }
}

pub(super) fn framebuffer_size_for_reconfigure<E>(
    pending_reconfigure: bool,
    query: impl FnOnce() -> Result<[u32; 2], E>,
) -> Result<Option<[u32; 2]>, E> {
    if pending_reconfigure {
        query().map(Some)
    } else {
        Ok(None)
    }
}

fn run_callback(callback_name: &'static str, callback: impl FnOnce(&RuntimeControl)) {
    let Some(control) = runtime_for_context(current_context()) else {
        return;
    };
    if !control.is_callback_accessible() {
        return;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = with_current_runtime(|active| {
            #[cfg(test)]
            active.maybe_panic_callback_for_test();
            callback(active);
        });
    }));
    if result.is_err() {
        control.record_fault(WgpuViewportError::CallbackPanicked {
            callback: callback_name,
        });
    }
}

pub(super) unsafe extern "C" fn renderer_create_window_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Renderer_CreateWindow", |control| unsafe {
        renderer_create_window(control, viewport.cast())
    });
}

pub(super) unsafe extern "C" fn renderer_destroy_window_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Renderer_DestroyWindow", |control| unsafe {
        renderer_destroy_window(control, viewport.cast())
    });
}

// The pointer-based aggregate hook is intentional: passing ImVec2 by value is not ABI-compatible
// across every supported C++ MSVC target.
pub(super) unsafe extern "C" fn renderer_set_window_size_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: *const dear_imgui_rs::sys::ImVec2,
) {
    run_callback("Renderer_SetWindowSize", |control| {
        if size.is_null() {
            control.record_fault(WgpuViewportError::SurfaceOperationFailed {
                operation: "read Renderer_SetWindowSize argument",
            });
            return;
        }
        unsafe { renderer_set_window_size(control, viewport.cast(), *size) };
    });
}

pub(super) unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    run_callback("Renderer_RenderWindow", |control| unsafe {
        renderer_render_window(control, viewport.cast())
    });
}

pub(super) unsafe extern "C" fn renderer_swap_buffers_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    run_callback("Renderer_SwapBuffers", |control| unsafe {
        renderer_swap_buffers(control, viewport.cast())
    });
}
