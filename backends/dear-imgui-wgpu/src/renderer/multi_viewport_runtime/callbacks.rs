//! Dear ImGui renderer callback ownership and C ABI entry points.

use super::CallbackOwnershipError;
use super::platform_adapter;
use super::registry::{
    CurrentContextGuard, borrow_renderer, current_context, destroy_viewport_data,
    globals_for_current_context, has_renderer_state, insert_renderer_state, register_viewport_data,
    registered_renderer_for_context, remove_renderer_state_for_context, renderer_globals,
    validate_new_registration, viewport_data_pointer,
};
use super::surface::{
    SurfaceAction, SurfaceEvent, create_viewport_data, handle_non_renderable_surface_event,
    request_close_after_surface_creation_failure, should_clear_viewport, surface_action,
};
use crate::renderer::WgpuRenderer;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::{PlatformIo, Viewport};
use dear_imgui_rs::{BackendFlags, Context};
use std::ffi::c_void;
use std::sync::atomic::Ordering;

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

pub(super) fn callbacks_owned(platform_io: &PlatformIo) -> bool {
    unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) && unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) && platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
        && render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_render_window_sys,
        )
        && render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_swap_buffers_sys,
        )
}

fn any_callback_owned(platform_io: &PlatformIo) -> bool {
    unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) || unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) || platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
        || render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_render_window_sys,
        )
        || render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_swap_buffers_sys,
        )
}

pub(super) fn claim_callbacks(
    platform_io: &mut PlatformIo,
    aggregate_hooks_available: bool,
) -> Result<(), CallbackOwnershipError> {
    if !platform_io.renderer_callbacks_are_empty() {
        if callbacks_owned(platform_io) {
            return Ok(());
        }
        return Err(CallbackOwnershipError::RendererCallbacksOccupied);
    }
    if !aggregate_hooks_available {
        return Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable);
    }
    platform_io.set_renderer_create_window_raw(Some(renderer_create_window_sys));
    platform_io.set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    platform_io.set_renderer_set_window_size_raw(Some(renderer_set_window_size_sys));
    platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
    platform_io.set_renderer_swap_buffers_raw(Some(renderer_swap_buffers_sys));
    Ok(())
}

pub(super) fn validate_secondary_viewports(
    states: &[(bool, *mut c_void)],
) -> Result<(), CallbackOwnershipError> {
    if states.iter().any(|(_, slot)| !slot.is_null()) {
        Err(CallbackOwnershipError::RendererUserDataOccupied)
    } else if states.iter().any(|(created, _)| *created) {
        Err(CallbackOwnershipError::PlatformWindowsAlreadyCreated)
    } else {
        Ok(())
    }
}

pub(super) unsafe fn enable(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    let raw_context = context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };
    let globals = renderer_globals(renderer)?;
    if !context
        .io()
        .backend_flags()
        .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    {
        return Err(CallbackOwnershipError::PlatformBackendUnavailable);
    }
    validate_new_registration(raw_context, renderer)?;
    let secondary_viewports = context
        .platform_io()
        .viewports_iter()
        .skip(1)
        .map(|viewport| {
            (
                viewport.platform_window_created(),
                viewport.renderer_user_data(),
            )
        })
        .collect::<Vec<_>>();
    validate_secondary_viewports(&secondary_viewports)?;
    claim_callbacks(
        context.platform_io_mut(),
        dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
    )?;
    insert_renderer_state(raw_context, renderer, Some(globals));
    renderer
        .multi_viewport_active
        .store(true, Ordering::Release);
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);
    Ok(())
}

pub(super) fn disable_after_platform_shutdown(context: &mut Context) {
    let raw_context = context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };
    let had_state = has_renderer_state(raw_context);
    let registered_renderer = registered_renderer_for_context(raw_context);
    let platform_io = context.platform_io_mut();
    let had_owned_callbacks = any_callback_owned(platform_io);
    for viewport in platform_io.viewports_iter_mut() {
        unsafe { destroy_viewport_data(raw_context, viewport) };
    }
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
    let renderer_callbacks_are_empty = platform_io.renderer_callbacks_are_empty();
    if let Some(renderer) = registered_renderer {
        // SAFETY: the registry entry is removed below while the owning context and renderer are
        // still required to be alive by the runtime shutdown contract.
        unsafe { &*renderer }
            .multi_viewport_active
            .store(false, Ordering::Release);
    }
    remove_renderer_state_for_context(raw_context);
    if (had_state || had_owned_callbacks) && renderer_callbacks_are_empty {
        let io = context.io_mut();
        let mut flags = io.backend_flags();
        flags.remove(BackendFlags::RENDERER_HAS_VIEWPORTS);
        io.set_backend_flags(flags);
    }
}

pub(super) fn shutdown_multi_viewport_support(
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    let raw_context = context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };
    if !has_renderer_state(raw_context) {
        return Ok(());
    }
    if !callbacks_owned(context.platform_io()) {
        return Err(CallbackOwnershipError::RendererCallbacksReplaced);
    }
    context.destroy_platform_windows();
    disable_after_platform_shutdown(context);
    Ok(())
}

pub(super) unsafe fn renderer_create_window(viewport: *mut Viewport) {
    let context = current_context();
    // SAFETY: callback entry points reject null pointers and Dear ImGui owns this live viewport.
    let viewport = unsafe { &mut *viewport };
    if !viewport.renderer_user_data().is_null() {
        return;
    }
    let Some(globals) = globals_for_current_context() else {
        return;
    };
    let Some(data) = (unsafe { create_viewport_data(context, viewport, &globals) }) else {
        request_close_after_surface_creation_failure(viewport);
        return;
    };
    let pointer = Box::into_raw(Box::new(data));
    register_viewport_data(context, pointer);
    viewport.set_renderer_user_data(pointer.cast());
}

pub(super) unsafe fn renderer_destroy_window(viewport: *mut Viewport) {
    // SAFETY: callback entry points reject null pointers and the registry validates ownership
    // before reclaiming any renderer data.
    unsafe { destroy_viewport_data(current_context(), &mut *viewport) };
}

unsafe fn renderer_set_window_size(viewport: *mut Viewport, size: dear_imgui_rs::sys::ImVec2) {
    // SAFETY: callback entry points reject null pointers and Dear ImGui owns this live viewport.
    let viewport = unsafe { &mut *viewport };
    let Some(pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        return;
    };
    let pixels = platform_adapter::logical_size_to_framebuffer(
        [size.x, size.y],
        viewport.framebuffer_scale(),
    );
    // SAFETY: `viewport_data_pointer` validates the pointer against this context's registry.
    let data = unsafe { &mut *pointer };
    if data.config.width != pixels[0] || data.config.height != pixels[1] {
        data.config.width = pixels[0];
        data.config.height = pixels[1];
        data.surface.configure(&data.device, &data.config);
    }
}

pub(super) unsafe fn renderer_render_window(viewport: *mut Viewport) {
    // SAFETY: callback entry points reject null pointers and Dear ImGui owns this live viewport.
    let viewport = unsafe { &mut *viewport };
    let Some(data_pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        if viewport.renderer_user_data().is_null() {
            // `UpdatePlatformWindows()` clears request flags after the create callbacks. Reassert
            // the fail-closed request here so it survives until Dear ImGui processes the window
            // on the next frame instead of leaving an unrenderable platform window alive.
            request_close_after_surface_creation_failure(viewport);
        }
        return;
    };
    let Some(mut renderer) = (unsafe { borrow_renderer() }) else {
        return;
    };
    let Some(globals) = globals_for_current_context() else {
        return;
    };
    let Some(backend) = renderer.backend_data.as_ref() else {
        return;
    };
    let device = backend.device.clone();
    let queue = backend.queue.clone();
    let raw_draw_data = viewport.draw_data();
    if raw_draw_data.is_null() {
        return;
    }
    // SAFETY: registry membership proves this is the live data owned by the current viewport.
    let data = unsafe { &mut *data_pointer };
    // SAFETY: Dear ImGui supplies live draw data for the duration of this render callback.
    let draw_data = unsafe { dear_imgui_rs::render::DrawData::from_raw_mut(&mut *raw_draw_data) };

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
                    &globals,
                )
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Lost => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Lost, viewport, data, &globals)
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Timeout => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Timeout, viewport, data, &globals)
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Occluded => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::Occluded,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::Validation,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
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
                    &globals,
                )
            };
            return;
        }
        Err(wgpu::SurfaceError::Lost) => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Lost, viewport, data, &globals)
            };
            return;
        }
        Err(wgpu::SurfaceError::Timeout) => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Timeout, viewport, data, &globals)
            };
            return;
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::OutOfMemory,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
        Err(wgpu::SurfaceError::Other) => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Other, viewport, data, &globals)
            };
            return;
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
        if let Err(error) = renderer.render_draw_data_with_fb_size_ex(
            draw_data,
            &mut render_pass,
            data.config.width,
            data.config.height,
            false,
            // SAFETY: Dear ImGui owns the process-global nil PlatformIO sentinel.
            unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() },
        ) {
            eprintln!("[wgpu-mv] viewport render failed: {error:?}");
            return;
        }
    }
    queue.submit(std::iter::once(encoder.finish()));
    data.pending_frame = Some(frame);
    data.pending_reconfigure = reconfigure_after_present;
}

unsafe fn renderer_swap_buffers(viewport: *mut Viewport) {
    // SAFETY: callback entry points reject null pointers and Dear ImGui owns this live viewport.
    let viewport = unsafe { &mut *viewport };
    let refreshed_size = unsafe { platform_adapter::framebuffer_size(viewport) };
    let Some(pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        return;
    };
    // SAFETY: `viewport_data_pointer` validates the pointer against this context's registry.
    let data = unsafe { &mut *pointer };
    let Some(frame) = data.pending_frame.take() else {
        return;
    };
    #[cfg(feature = "wgpu-30")]
    data.queue.present(frame);
    #[cfg(not(feature = "wgpu-30"))]
    frame.present();
    if data.pending_reconfigure {
        if let Some(size) = refreshed_size {
            data.config.width = size[0].max(1);
            data.config.height = size[1].max(1);
        }
        data.surface.configure(&data.device, &data.config);
        data.pending_reconfigure = false;
    }
}

fn run_callback(name: &str, callback: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err() {
        eprintln!("[wgpu-mv] panic in {name}");
        std::process::abort();
    }
}

unsafe extern "C" fn renderer_create_window_sys(viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_CreateWindow", || unsafe {
        renderer_create_window(viewport.cast())
    });
}

pub(super) unsafe extern "C" fn renderer_destroy_window_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_DestroyWindow", || unsafe {
        renderer_destroy_window(viewport.cast())
    });
}

// The pointer-based aggregate hook is intentional: passing ImVec2 by value is not ABI-compatible
// across every supported C++ MSVC target.
unsafe extern "C" fn renderer_set_window_size_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: *const dear_imgui_rs::sys::ImVec2,
) {
    if viewport.is_null() || size.is_null() {
        return;
    }
    run_callback("Renderer_SetWindowSize", || unsafe {
        renderer_set_window_size(viewport.cast(), *size)
    });
}

unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_RenderWindow", || unsafe {
        renderer_render_window(viewport.cast())
    });
}

unsafe extern "C" fn renderer_swap_buffers_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_SwapBuffers", || unsafe {
        renderer_swap_buffers(viewport.cast())
    });
}
