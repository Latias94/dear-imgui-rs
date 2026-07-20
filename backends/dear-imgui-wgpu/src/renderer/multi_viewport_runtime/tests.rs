use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextBinding, ContextTeardown, sys,
};

use super::callbacks::{
    framebuffer_size_for_reconfigure, publish_registered_box, render_callback_matches,
    renderer_create_window_sys, renderer_destroy_window_sys, renderer_render_window_sys,
    renderer_set_window_size_sys, renderer_swap_buffers_sys, unary_callback_matches,
};
use super::registry::{
    fail_next_viewport_registration, register_viewport_data, unregister_viewport_data,
    viewport_data_count,
};
use super::runtime::{RuntimeControl, RuntimeState};
use super::surface::{
    SurfaceAction, SurfaceEvent, ViewportWgpuData, should_clear_viewport, surface_action,
};
use super::{OwningViewportRuntime, WgpuViewportError, logical_size_to_framebuffer};
use crate::renderer::WgpuRenderer;
use crate::renderer::callbacks::{
    draw_callback_reset_render_state, draw_callback_set_sampler_linear,
    draw_callback_set_sampler_nearest,
};

struct TestPlatformMarker;
struct TestPlatformAttachment;

impl ContextAttachment for TestPlatformAttachment {
    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        context.with_bound_context(clear_test_main_handle_raw);
    }
}

struct TestPlatformLease {
    binding: ContextBinding,
    _lease: ContextAttachmentLease,
}

impl Drop for TestPlatformLease {
    fn drop(&mut self) {
        let _ = self
            .binding
            .try_with_bound_context(clear_test_main_handle_raw);
    }
}

struct OrderingPlatformMarker;

struct OrderingPlatformAttachment {
    control: Rc<RefCell<Option<Rc<RuntimeControl>>>>,
    renderer_released_first: Rc<Cell<bool>>,
    platform_phase_count: Rc<Cell<u32>>,
}

impl ContextAttachment for OrderingPlatformAttachment {
    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        self.platform_phase_count
            .set(self.platform_phase_count.get() + 1);
        self.renderer_released_first.set(
            self.control
                .borrow()
                .as_ref()
                .is_some_and(|control| control.state() == RuntimeState::ResourceDropped),
        );
        context.with_bound_context(clear_test_main_handle_raw);
    }
}

struct OccupiedRendererMarker;
struct OccupiedRendererAttachment;

impl ContextAttachment for OccupiedRendererAttachment {}

struct DropProbe(Rc<Cell<u32>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn lock_context() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

unsafe extern "C" fn platform_unary(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn renderer_unary(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn renderer_set_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}

unsafe extern "C" fn renderer_render(_viewport: *mut sys::ImGuiViewport, _argument: *mut c_void) {}

unsafe extern "C" fn foreign_draw_callback(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
}

fn claim_test_platform_callbacks(context: &mut Context) {
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_platform_create_window_raw(Some(platform_unary));
        platform_io.set_platform_destroy_window_raw(Some(platform_unary));
        context
            .main_viewport()
            .set_platform_handle(std::ptr::dangling_mut::<c_void>());
    }
}

fn clear_test_main_handle_raw() {
    let viewport = unsafe { sys::igGetMainViewport() };
    if !viewport.is_null() {
        unsafe {
            (*viewport).PlatformHandle = std::ptr::null_mut();
            (*viewport).PlatformHandleRaw = std::ptr::null_mut();
        }
    }
}

fn attach_test_platform(context: &mut Context) -> TestPlatformLease {
    let lease = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    }
}

fn attach_ordering_platform(
    context: &mut Context,
    control: Rc<RefCell<Option<Rc<RuntimeControl>>>>,
    renderer_released_first: Rc<Cell<bool>>,
    platform_phase_count: Rc<Cell<u32>>,
) -> TestPlatformLease {
    let lease = context
        .register_attachment::<OrderingPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(OrderingPlatformAttachment {
                control,
                renderer_released_first,
                platform_phase_count,
            }),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    }
}

fn shutdown_returned_renderer(renderer: &mut WgpuRenderer, context: &mut Context) {
    renderer
        .shutdown(context)
        .expect("returned test renderer should remain usable");
}

fn occupy_renderer_slot(context: &mut Context, slot: usize) {
    let platform_io = context.platform_io_mut();
    unsafe {
        match slot {
            0 => platform_io.set_renderer_create_window_raw(Some(renderer_unary)),
            1 => platform_io.set_renderer_destroy_window_raw(Some(renderer_unary)),
            2 => platform_io.set_renderer_set_window_size_raw(Some(renderer_set_size)),
            3 => platform_io.set_renderer_render_window_raw(Some(renderer_render)),
            4 => platform_io.set_renderer_swap_buffers_raw(Some(renderer_render)),
            _ => unreachable!(),
        }
    }
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
        Ok::<_, ()>([640, 480])
    };
    assert_eq!(framebuffer_size_for_reconfigure(false, query), Ok(None));
    assert_eq!(calls.get(), 0);
    assert_eq!(
        framebuffer_size_for_reconfigure(true, query),
        Ok(Some([640, 480]))
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn attach_requires_a_registered_platform_role_and_returns_renderer() {
    let _guard = lock_context();
    let mut context = Context::create();
    claim_test_platform_callbacks(&mut context);

    let failure =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap_err();
    assert!(matches!(
        failure.error(),
        WgpuViewportError::Attachment(dear_imgui_rs::ContextAttachmentError::MissingPlatform)
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let mut renderer = failure.into_renderer();
    shutdown_returned_renderer(&mut renderer, &mut context);
    clear_test_main_handle_raw();
}

#[test]
fn missing_platform_callback_rejects_attach_transactionally() {
    let _guard = lock_context();
    let mut context = Context::create();
    let lease = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    let _platform = TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    };
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    unsafe {
        context
            .platform_io_mut()
            .set_platform_create_window_raw(Some(platform_unary));
        context
            .main_viewport()
            .set_platform_handle(std::ptr::dangling_mut::<c_void>());
    }

    let failure =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap_err();
    assert!(matches!(
        failure.error(),
        WgpuViewportError::PlatformCallbackUnavailable {
            callback: "Platform_DestroyWindow"
        }
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let mut renderer = failure.into_renderer();
    shutdown_returned_renderer(&mut renderer, &mut context);
}

#[test]
fn missing_main_window_handle_rejects_attach_transactionally() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_platform_create_window_raw(Some(platform_unary));
        platform_io.set_platform_destroy_window_raw(Some(platform_unary));
    }

    let failure =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap_err();
    assert!(matches!(
        failure.error(),
        WgpuViewportError::MainViewportHandleUnavailable
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let mut renderer = failure.into_renderer();
    shutdown_returned_renderer(&mut renderer, &mut context);
}

#[test]
fn every_occupied_renderer_slot_rejects_attach_without_partial_claim() {
    let _guard = lock_context();
    for slot in 0..5 {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        occupy_renderer_slot(&mut context, slot);

        let failure = OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty())
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            WgpuViewportError::RendererCallbackOccupied { .. }
        ));
        let mut renderer = failure.into_renderer();
        shutdown_returned_renderer(&mut renderer, &mut context);
        unsafe { context.platform_io_mut().clear_renderer_handlers() };
    }
}

#[test]
fn renderer_role_rollback_returns_renderer_without_claiming_callbacks() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let _occupied = context
        .register_attachment::<OccupiedRendererMarker>(
            ContextAttachmentRole::Renderer,
            Rc::new(OccupiedRendererAttachment),
        )
        .unwrap();

    let failure =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap_err();
    assert!(matches!(
        failure.error(),
        WgpuViewportError::Attachment(dear_imgui_rs::ContextAttachmentError::RoleOccupied(
            ContextAttachmentRole::Renderer
        ))
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let mut renderer = failure.into_renderer();
    shutdown_returned_renderer(&mut renderer, &mut context);
}

#[test]
fn moving_wrapper_keeps_runtime_owned_renderer_storage_stable() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let address = runtime.renderer_address_for_test();

    fn move_runtime(runtime: OwningViewportRuntime) -> OwningViewportRuntime {
        runtime
    }
    let mut moved = move_runtime(runtime);
    assert_eq!(moved.renderer_address_for_test(), address);
    assert_eq!(moved.state_for_test(), RuntimeState::Attached);

    moved.shutdown(&mut context).unwrap();
    moved.shutdown(&mut context).unwrap();
    assert_eq!(moved.state_for_test(), RuntimeState::ResourceDropped);
    assert_eq!(
        moved.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
}

#[test]
fn shutdown_with_outstanding_snapshot_keeps_renderer_for_retry() {
    let _guard = lock_context();
    let mut context = Context::create();
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    assert!(context.font_atlas().build());
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    let snapshot = {
        let renderer = control.borrow_renderer_for_test();
        let consumer = renderer
            .as_ref()
            .unwrap()
            .renderer_consumer
            .as_ref()
            .unwrap();
        context.begin_frame().render_snapshot(consumer).unwrap()
    };

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::Renderer(
            crate::RendererError::RendererConsumer(
                dear_imgui_rs::render::RendererConsumerError::OutstandingEpochs { count: 1 }
            )
        ))
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::Detached);
    assert!(control.has_renderer_for_test());

    drop(snapshot);
    context.poll_snapshot_completions().unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
}

#[test]
fn viewport_cleanup_failure_keeps_renderer_and_callbacks_for_retry() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    runtime.fail_next_viewport_cleanup_for_test();

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "injected viewport cleanup failure"
        })
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(control.has_renderer_for_test());
    assert!(!context.platform_io().renderer_callbacks_are_empty());

    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert!(context.platform_io().renderer_callbacks_are_empty());
}

#[test]
fn viewport_registration_failure_keeps_box_owned_and_publishes_nothing() {
    let _guard = lock_context();
    let context = Context::create();
    let binding = context.binding();
    let drops = Rc::new(Cell::new(0));
    let published = Cell::new(false);
    fail_next_viewport_registration();

    let result = publish_registered_box(
        Box::new(DropProbe(Rc::clone(&drops))),
        |pointer| register_viewport_data(&binding, pointer.cast()),
        |_| published.set(true),
    );

    assert!(matches!(
        result,
        Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "injected viewport registration failure"
        })
    ));
    assert_eq!(drops.get(), 1);
    assert!(!published.get());
    assert_eq!(viewport_data_count(context.id()), 0);
}

#[test]
fn foreign_renderer_user_data_is_preserved_and_reported_by_ffi_callbacks() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let foreign = std::ptr::dangling_mut::<c_void>();
    let mut viewport = sys::ImGuiViewport {
        RendererUserData: foreign,
        ..Default::default()
    };

    unsafe { renderer_create_window_sys(&mut viewport) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_CreateWindow"
        })
    ));
    assert_eq!(viewport.RendererUserData, foreign);

    unsafe { renderer_destroy_window_sys(&mut viewport) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert_eq!(viewport.RendererUserData, foreign);

    let size = sys::ImVec2 { x: 32.0, y: 24.0 };
    unsafe { renderer_set_window_size_sys(&mut viewport, &size) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_SetWindowSize"
        })
    ));
    assert_eq!(viewport.RendererUserData, foreign);

    unsafe { renderer_render_window_sys(&mut viewport, std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_RenderWindow"
        })
    ));
    assert_eq!(viewport.RendererUserData, foreign);

    unsafe { renderer_swap_buffers_sys(&mut viewport, std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_SwapBuffers"
        })
    ));
    assert_eq!(viewport.RendererUserData, foreign);

    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn foreign_callback_replacement_is_preserved_and_reported() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_render_window_raw(Some(renderer_render));
    }
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererCallbackReplaced {
            callback: "Renderer_RenderWindow"
        })
    ));
    assert!(render_callback_matches(
        context.platform_io().renderer_render_window_raw(),
        renderer_render
    ));

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::RendererCallbackReplaced {
            callback: "Renderer_RenderWindow"
        })
    ));
    assert!(render_callback_matches(
        context.platform_io().renderer_render_window_raw(),
        renderer_render
    ));
    unsafe { context.platform_io_mut().clear_renderer_handlers() };
}

#[test]
fn callback_reentry_is_deferred_to_next_rust_entry() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
    register_viewport_data(&context.binding(), pointer).unwrap();
    let mut viewport = sys::ImGuiViewport {
        RendererUserData: pointer.cast(),
        ..Default::default()
    };
    let renderer_borrow = control.borrow_renderer_for_test();

    unsafe { renderer_render_window_sys(&mut viewport, std::ptr::null_mut()) };
    drop(renderer_borrow);
    unregister_viewport_data(pointer);
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::CallbackReentered {
            callback: "Renderer_RenderWindow"
        })
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn callback_panic_is_contained_and_deferred() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    runtime.panic_next_callback_for_test();
    let viewport = unsafe { sys::igGetMainViewport() };

    unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::CallbackPanicked {
            callback: "Renderer_RenderWindow"
        })
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn context_first_shutdown_releases_renderer_before_platform_phase_once() {
    let _guard = lock_context();
    let mut context = Context::create();
    let control_slot = Rc::new(RefCell::new(None));
    let renderer_released_first = Rc::new(Cell::new(false));
    let platform_phase_count = Rc::new(Cell::new(0));
    let _platform = attach_ordering_platform(
        &mut context,
        Rc::clone(&control_slot),
        Rc::clone(&renderer_released_first),
        Rc::clone(&platform_phase_count),
    );
    let runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    control_slot.borrow_mut().replace(Rc::clone(&control));

    drop(context);

    assert!(renderer_released_first.get());
    assert_eq!(platform_phase_count.get(), 1);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(
        control.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
    drop(runtime);
}

#[test]
fn dropping_wrapper_releases_resources_and_allows_new_runtime() {
    let _guard = lock_context();
    let mut context = Context::create();
    let control_slot = Rc::new(RefCell::new(None));
    let renderer_released_first = Rc::new(Cell::new(false));
    let platform_phase_count = Rc::new(Cell::new(0));
    let _platform = attach_ordering_platform(
        &mut context,
        Rc::clone(&control_slot),
        Rc::clone(&renderer_released_first),
        Rc::clone(&platform_phase_count),
    );
    let runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    control_slot.borrow_mut().replace(Rc::clone(&control));

    drop(runtime);

    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert_eq!(platform_phase_count.get(), 0);

    let mut replacement =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    replacement.shutdown(&mut context).unwrap();
    drop(context);
    assert_eq!(platform_phase_count.get(), 1);
    assert!(renderer_released_first.get());
}

fn attach_configured_test_runtime(context: &mut Context) -> (OwningViewportRuntime, BackendFlags) {
    context
        .set_renderer_name(Some(format!(
            "dear-imgui-wgpu {}",
            env!("CARGO_PKG_VERSION")
        )))
        .unwrap();
    let owned_flags = BackendFlags::RENDERER_HAS_TEXTURES | BackendFlags::RENDERER_HAS_VTX_OFFSET;
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | owned_flags);
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
    }

    let mut renderer = WgpuRenderer::empty();
    renderer.bind_context(context, owned_flags).unwrap();
    renderer.renderer_consumer = Some(context.create_renderer_consumer().unwrap());
    (
        OwningViewportRuntime::attach_for_test(context, renderer).unwrap(),
        owned_flags,
    )
}

fn replace_backend_state_with_foreign_values(context: &mut Context) {
    context
        .set_renderer_name(Some("foreign-renderer".to_owned()))
        .unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_draw_callback_reset_render_state_raw(Some(foreign_draw_callback));
    }
}

fn assert_and_clear_foreign_backend_state(context: &mut Context, owned_flags: BackendFlags) {
    assert_eq!(
        context.io().backend_renderer_name().unwrap().to_bytes(),
        b"foreign-renderer"
    );
    assert!(context.io().backend_flags().contains(owned_flags));
    assert!(std::ptr::fn_addr_eq(
        context
            .platform_io()
            .draw_callback_reset_render_state_raw()
            .unwrap(),
        foreign_draw_callback
            as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd)
    ));
    assert!(
        context
            .platform_io()
            .draw_callback_set_sampler_linear_raw()
            .is_none()
    );
    assert!(
        context
            .platform_io()
            .draw_callback_set_sampler_nearest_raw()
            .is_none()
    );

    context.set_renderer_name(None::<String>).unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_draw_callback_reset_render_state_raw(None);
    }
    let io = context.io_mut();
    let mut flags = io.backend_flags();
    flags.remove(owned_flags | BackendFlags::RENDERER_HAS_VIEWPORTS);
    io.set_backend_flags(flags);
}

#[test]
fn drop_preserves_foreign_backend_state_replacements() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let (runtime, owned_flags) = attach_configured_test_runtime(&mut context);
    replace_backend_state_with_foreign_values(&mut context);
    drop(runtime);

    assert_and_clear_foreign_backend_state(&mut context, owned_flags);
}

#[test]
fn explicit_shutdown_preserves_foreign_backend_state_replacements() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let (mut runtime, owned_flags) = attach_configured_test_runtime(&mut context);
    replace_backend_state_with_foreign_values(&mut context);

    runtime.shutdown(&mut context).unwrap();

    assert_and_clear_foreign_backend_state(&mut context, owned_flags);
}

#[test]
fn callback_match_helpers_do_not_confuse_foreign_functions() {
    assert!(unary_callback_matches(Some(renderer_unary), renderer_unary));
    assert!(!unary_callback_matches(
        Some(platform_unary),
        renderer_unary
    ));
    assert!(render_callback_matches(
        Some(renderer_render),
        renderer_render
    ));
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
    assert!(should_clear_viewport(dear_imgui_rs::ViewportFlags::empty()));
    assert!(!should_clear_viewport(
        dear_imgui_rs::ViewportFlags::NO_RENDERER_CLEAR
    ));
}
