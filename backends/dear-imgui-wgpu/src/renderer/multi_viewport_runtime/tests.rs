use super::callbacks::{
    callbacks_owned, claim_callbacks, disable_after_platform_shutdown,
    framebuffer_size_for_reconfigure, render_callback_matches, renderer_create_window,
    renderer_destroy_window, renderer_destroy_window_sys, renderer_render_window,
    shutdown_multi_viewport_support, unary_callback_matches, validate_secondary_viewports,
};
use super::registry::{
    CurrentContextGuard, borrow_renderer, has_renderer_state, insert_renderer_state,
    register_viewport_data, unregister_viewport_data, validate_new_registration,
    viewport_data_pointer,
};
use super::surface::{
    SurfaceAction, SurfaceEvent, ViewportWgpuData, request_close_after_surface_creation_failure,
    should_clear_viewport, surface_action,
};
use super::{CallbackOwnershipError, enable, logical_size_to_framebuffer};
use crate::renderer::WgpuRenderer;
use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::{BackendFlags, Context, ViewportFlags};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use std::sync::{Mutex as TestMutex, MutexGuard, OnceLock};

fn lock_context() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<TestMutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| TestMutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

#[test]
fn logical_size_conversion_clamps_invalid_dimensions_and_scale() {
    assert_eq!(
        logical_size_to_framebuffer([320.0, 200.0], [1.5, 2.0]),
        [480, 400]
    );
    assert_eq!(
        logical_size_to_framebuffer([0.0, f32::NAN], [0.0, f32::INFINITY]),
        [1, 1]
    );
}

#[test]
fn framebuffer_size_is_queried_only_for_pending_reconfigure() {
    let calls = Cell::new(0);
    let query = || {
        calls.set(calls.get() + 1);
        Some([640, 480])
    };

    assert_eq!(framebuffer_size_for_reconfigure(false, query), None);
    assert_eq!(calls.get(), 0);
    assert_eq!(
        framebuffer_size_for_reconfigure(true, query),
        Some([640, 480])
    );
    assert_eq!(calls.get(), 1);
}

unsafe fn install_test_renderer(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    let raw_context = context.as_raw();
    let _guard = unsafe { CurrentContextGuard::bind(raw_context) };
    validate_new_registration(raw_context, renderer)?;
    claim_callbacks(
        context.platform_io_mut(),
        dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
    )?;
    if renderer.context_binding.is_none() {
        renderer
            .bind_context(context, BackendFlags::empty())
            .expect("test renderer should bind to its registration context");
    }
    insert_renderer_state(raw_context, renderer, None);
    renderer
        .multi_viewport_active
        .store(true, Ordering::Release);
    Ok(())
}

unsafe extern "C" fn unary_sentinel(_viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {}

unsafe extern "C" fn size_sentinel(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _size: *const dear_imgui_rs::sys::ImVec2,
) {
}

unsafe extern "C" fn render_sentinel(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

unsafe extern "C" fn platform_sentinel(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

#[test]
fn install_targets_passed_context_and_preserves_platform_slots() {
    let _lock = lock_context();
    let mut context_a = Context::create();
    let raw_a = context_a.as_raw();
    let platform_io_a = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };
    unsafe {
        (*platform_io_a).Platform_RenderWindow = Some(platform_sentinel);
        (*platform_io_a).Platform_SwapBuffers = Some(platform_sentinel);
        dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
    }
    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    let platform_io_b = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_b) };
    let mut renderer = Box::new(WgpuRenderer::empty());

    unsafe { install_test_renderer(&mut renderer, &mut context_a) }.unwrap();

    unsafe {
        assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
        assert!(callbacks_owned(context_a.platform_io()));
        assert!((*platform_io_b).Renderer_CreateWindow.is_none());
        assert!(render_callback_matches(
            (*platform_io_a).Platform_RenderWindow,
            platform_sentinel
        ));
        assert!(render_callback_matches(
            (*platform_io_a).Platform_SwapBuffers,
            platform_sentinel
        ));
    }

    disable_after_platform_shutdown(&mut context_a);
    unsafe {
        (*platform_io_a).Platform_RenderWindow = None;
        (*platform_io_a).Platform_SwapBuffers = None;
        dear_imgui_rs::sys::igSetCurrentContext(raw_a);
    }
    drop(context_a);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    drop(context_b);
}

#[test]
fn foreign_renderer_slots_are_rejected_without_mutation() {
    let _lock = lock_context();
    let mut context = Context::create();
    let platform_io = context.platform_io_mut();
    platform_io.set_renderer_create_window_raw(Some(unary_sentinel));
    platform_io.set_renderer_destroy_window_raw(Some(unary_sentinel));
    platform_io.set_renderer_set_window_size_raw(Some(size_sentinel));
    platform_io.set_renderer_render_window_raw(Some(render_sentinel));
    platform_io.set_renderer_swap_buffers_raw(Some(render_sentinel));
    let mut renderer = Box::new(WgpuRenderer::empty());

    assert_eq!(
        unsafe { install_test_renderer(&mut renderer, &mut context) },
        Err(CallbackOwnershipError::RendererCallbacksOccupied)
    );

    let platform_io = context.platform_io_mut();
    assert!(unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        unary_sentinel
    ));
    assert!(unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        unary_sentinel
    ));
    assert!(platform_io.renderer_set_window_size_matches_pointer_callback(size_sentinel));
    assert!(render_callback_matches(
        platform_io.renderer_render_window_raw(),
        render_sentinel
    ));
    assert!(render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        render_sentinel
    ));
    platform_io.clear_renderer_handlers();
}

#[test]
fn shutdown_is_a_noop_for_a_foreign_renderer_context() {
    let _lock = lock_context();
    let mut context = Context::create();
    let platform_io = context.platform_io_mut();
    platform_io.set_renderer_create_window_raw(Some(unary_sentinel));
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    shutdown_multi_viewport_support(&mut context).unwrap();

    assert!(unary_callback_matches(
        context.platform_io().renderer_create_window_raw(),
        unary_sentinel
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    context
        .platform_io_mut()
        .set_renderer_create_window_raw(None);
}

#[test]
fn aggregate_hook_failure_does_not_install_partial_callbacks() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    assert_eq!(
        claim_callbacks(context.platform_io_mut(), false),
        Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state(raw));
}

#[test]
fn foreign_renderer_user_data_preflight_is_transactional() {
    let _lock = lock_context();
    let context = Context::create();
    let foreign = 0x1234_usize as *mut c_void;

    assert_eq!(
        validate_secondary_viewports(&[(false, std::ptr::null_mut()), (false, foreign)]),
        Err(CallbackOwnershipError::RendererUserDataOccupied)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state(context.as_raw()));
}

#[test]
fn existing_platform_window_preflight_is_transactional() {
    let _lock = lock_context();
    let context = Context::create();

    assert_eq!(
        validate_secondary_viewports(&[(true, std::ptr::null_mut())]),
        Err(CallbackOwnershipError::PlatformWindowsAlreadyCreated)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state(context.as_raw()));
}

#[test]
fn public_enable_preflight_is_transactional() {
    let _lock = lock_context();
    let mut context = Context::create();
    let mut renderer = Box::new(WgpuRenderer::empty());
    let flags_before = context.io().backend_flags();

    assert_eq!(
        unsafe { enable(&mut renderer, &mut context) },
        Err(CallbackOwnershipError::RendererNotInitialized)
    );

    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert_eq!(context.io().backend_flags(), flags_before);
    assert!(!has_renderer_state(context.as_raw()));
    assert!(!renderer.multi_viewport_active.load(Ordering::Acquire));
}

#[test]
fn disable_preserves_callbacks_replaced_after_install() {
    let _lock = lock_context();
    let mut context = Context::create();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
    let platform_io = context.platform_io_mut();
    platform_io.set_renderer_create_window_raw(Some(unary_sentinel));
    platform_io.set_renderer_destroy_window_raw(Some(unary_sentinel));
    platform_io.set_renderer_set_window_size_raw(Some(size_sentinel));
    platform_io.set_renderer_render_window_raw(Some(render_sentinel));
    platform_io.set_renderer_swap_buffers_raw(Some(render_sentinel));
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    disable_after_platform_shutdown(&mut context);

    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    let platform_io = context.platform_io_mut();
    assert!(unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        unary_sentinel
    ));
    assert!(unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        unary_sentinel
    ));
    assert!(platform_io.renderer_set_window_size_matches_pointer_callback(size_sentinel));
    assert!(render_callback_matches(
        platform_io.renderer_render_window_raw(),
        render_sentinel
    ));
    assert!(render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        render_sentinel
    ));
    platform_io.clear_renderer_handlers();
}

#[test]
fn shutdown_rejects_callback_drift_before_mutating_runtime_state() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
    let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
    register_viewport_data(raw, pointer);
    let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport {
        RendererUserData: pointer.cast(),
        ..Default::default()
    };
    context
        .platform_io_mut()
        .set_renderer_destroy_window_raw(Some(unary_sentinel));

    assert_eq!(
        shutdown_multi_viewport_support(&mut context),
        Err(CallbackOwnershipError::RendererCallbacksReplaced)
    );

    assert!(has_renderer_state(raw));
    assert!(renderer.multi_viewport_active.load(Ordering::Acquire));
    let viewport = unsafe { Viewport::from_raw_mut(&mut raw_viewport) };
    assert_eq!(unsafe { viewport_data_pointer(viewport) }, Some(pointer));
    assert!(unary_callback_matches(
        context.platform_io().renderer_destroy_window_raw(),
        unary_sentinel
    ));

    unregister_viewport_data(pointer);
    context
        .platform_io_mut()
        .set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    shutdown_multi_viewport_support(&mut context).unwrap();
}

#[test]
fn renderer_registry_is_context_local() {
    let _lock = lock_context();
    let mut context_a = Context::create();
    let raw_a = context_a.as_raw();
    let mut renderer_a = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer_a, &mut context_a) }.unwrap();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
    let mut context_b = Context::create();
    let raw_b = context_b.as_raw();
    let mut renderer_b = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer_b, &mut context_b) }.unwrap();

    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        let borrowed = borrow_renderer().unwrap();
        assert_eq!(borrowed.renderer, (&mut *renderer_a) as *mut _);
        drop(borrowed);
        dear_imgui_rs::sys::igSetCurrentContext(raw_b);
        let borrowed = borrow_renderer().unwrap();
        assert_eq!(borrowed.renderer, (&mut *renderer_b) as *mut _);
        drop(borrowed);
        dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
        assert!(borrow_renderer().is_none());
    }

    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    disable_after_platform_shutdown(&mut context_b);
    drop(context_b);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    disable_after_platform_shutdown(&mut context_a);
}

#[test]
fn nested_renderer_borrow_is_rejected_and_recovers_after_drop() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw) };

    let first = unsafe { borrow_renderer() }.unwrap();
    assert!(unsafe { borrow_renderer() }.is_none());
    drop(first);
    assert!(unsafe { borrow_renderer() }.is_some());

    disable_after_platform_shutdown(&mut context);
}

#[test]
fn renderer_rebind_is_rejected_while_callback_is_active() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    let mut first_renderer = Box::new(WgpuRenderer::empty());
    let mut next_renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut first_renderer, &mut context) }.unwrap();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw) };
    let borrow = unsafe { borrow_renderer() }.unwrap();

    assert_eq!(
        unsafe { install_test_renderer(&mut next_renderer, &mut context) },
        Err(CallbackOwnershipError::RendererCallbackActive)
    );

    drop(borrow);
    disable_after_platform_shutdown(&mut context);
}

#[test]
fn renderer_rebind_is_rejected_with_live_viewport_data() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    let mut first_renderer = Box::new(WgpuRenderer::empty());
    let mut next_renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut first_renderer, &mut context) }.unwrap();
    let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
    register_viewport_data(raw, pointer);

    assert_eq!(
        unsafe { install_test_renderer(&mut next_renderer, &mut context) },
        Err(CallbackOwnershipError::LiveViewportRendererRebind)
    );

    unregister_viewport_data(pointer);
    disable_after_platform_shutdown(&mut context);
}

#[test]
fn repeated_registration_requires_full_shutdown() {
    let _lock = lock_context();
    let mut context = Context::create();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();

    assert_eq!(
        unsafe { install_test_renderer(&mut renderer, &mut context) },
        Err(CallbackOwnershipError::AlreadyEnabled)
    );

    disable_after_platform_shutdown(&mut context);
}

#[test]
fn renderer_cannot_be_registered_with_two_contexts() {
    let _lock = lock_context();
    let mut context_a = Context::create();
    let raw_a = context_a.as_raw();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context_a) }.unwrap();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
    let mut context_b = Context::create();

    assert_eq!(
        unsafe { install_test_renderer(&mut renderer, &mut context_b) },
        Err(CallbackOwnershipError::RendererAlreadyRegistered)
    );
    assert!(context_b.platform_io().renderer_callbacks_are_empty());

    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    disable_after_platform_shutdown(&mut context_a);
    drop(context_a);
    drop(context_b);
}

#[test]
fn viewport_user_data_is_bound_to_its_owner_context() {
    let _lock = lock_context();
    let context_a = Context::create();
    let raw_a = context_a.as_raw();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
    register_viewport_data(raw_a, pointer);
    let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport {
        RendererUserData: pointer.cast(),
        ..Default::default()
    };
    let viewport = unsafe { Viewport::from_raw_mut(&mut raw_viewport) };

    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    assert!(unsafe { viewport_data_pointer(viewport) }.is_none());
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    assert_eq!(unsafe { viewport_data_pointer(viewport) }, Some(pointer));

    unregister_viewport_data(pointer);
    drop(context_a);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    drop(context_b);
}

#[test]
fn destroy_ignores_foreign_renderer_user_data() {
    let _lock = lock_context();
    let context = Context::create();
    let raw = context.as_raw();
    let foreign = 0x1234_usize as *mut c_void;
    let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport {
        RendererUserData: foreign,
        ..Default::default()
    };
    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(raw);
        renderer_destroy_window(
            (&mut raw_viewport as *mut dear_imgui_rs::sys::ImGuiViewport).cast::<Viewport>(),
        );
    }
    assert_eq!(raw_viewport.RendererUserData, foreign);
}

#[test]
fn failed_surface_creation_requests_platform_close_without_gpu() {
    let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport::default();
    let viewport = unsafe { Viewport::from_raw_mut(&mut raw_viewport) };

    request_close_after_surface_creation_failure(viewport);

    assert!(raw_viewport.PlatformRequestClose);
    assert!(raw_viewport.RendererUserData.is_null());
}

#[test]
fn missing_surface_render_reasserts_close_without_gpu() {
    let _lock = lock_context();
    let context = Context::create();
    let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport::default();

    unsafe {
        renderer_render_window(
            (&mut raw_viewport as *mut dear_imgui_rs::sys::ImGuiViewport).cast::<Viewport>(),
        );
    }

    assert!(raw_viewport.PlatformRequestClose);
    drop(context);
}

#[test]
fn renderer_create_preserves_foreign_user_data_without_requesting_close() {
    let _lock = lock_context();
    let foreign = 0x1234_usize as *mut c_void;
    let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport {
        RendererUserData: foreign,
        ..Default::default()
    };

    unsafe {
        renderer_create_window(
            (&mut raw_viewport as *mut dear_imgui_rs::sys::ImGuiViewport).cast::<Viewport>(),
        );
    }

    assert_eq!(raw_viewport.RendererUserData, foreign);
    assert!(!raw_viewport.PlatformRequestClose);
}

#[test]
fn disable_clears_runtime_state_and_renderer_capability() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    disable_after_platform_shutdown(&mut context);

    assert!(!has_renderer_state(raw));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!renderer.multi_viewport_active.load(Ordering::Acquire));
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
}

#[test]
fn renderer_lifecycle_mutation_is_blocked_until_runtime_shutdown() {
    let _lock = lock_context();
    let mut context = Context::create();
    let raw = context.as_raw();
    let mut renderer = Box::new(WgpuRenderer::empty());
    unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);
    assert!(matches!(
        renderer.shutdown(&mut context),
        Err(crate::RendererError::MultiViewportActive)
    ));
    assert!(has_renderer_state(raw));
    disable_after_platform_shutdown(&mut context);
    assert!(!renderer.multi_viewport_active.load(Ordering::Acquire));
    assert!(renderer.ensure_multi_viewport_inactive().is_ok());
    assert!(renderer.shutdown(&mut context).is_ok());
    assert!(!has_renderer_state(raw));
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
}

#[test]
fn surface_events_have_explicit_recovery_actions() {
    assert_eq!(surface_action(SurfaceEvent::Success), SurfaceAction::Render);
    assert_eq!(
        surface_action(SurfaceEvent::Suboptimal),
        SurfaceAction::RenderThenReconfigure
    );
    assert_eq!(
        surface_action(SurfaceEvent::Outdated),
        SurfaceAction::Reconfigure
    );
    assert_eq!(surface_action(SurfaceEvent::Lost), SurfaceAction::Recreate);
    assert_eq!(surface_action(SurfaceEvent::Timeout), SurfaceAction::Skip);
    assert_eq!(surface_action(SurfaceEvent::Occluded), SurfaceAction::Skip);
    assert_eq!(
        surface_action(SurfaceEvent::Validation),
        SurfaceAction::Reject
    );
    assert_eq!(
        surface_action(SurfaceEvent::OutOfMemory),
        SurfaceAction::Reject
    );
    assert_eq!(surface_action(SurfaceEvent::Other), SurfaceAction::Reject);
    assert!(should_clear_viewport(ViewportFlags::empty()));
    assert!(!should_clear_viewport(ViewportFlags::NO_RENDERER_CLEAR));
}
