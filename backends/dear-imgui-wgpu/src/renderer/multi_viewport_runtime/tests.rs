use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextBinding, ContextTeardown, sys,
};

use super::callbacks::{
    destroy_renderer_viewport_resources, framebuffer_size_for_reconfigure, publish_registered_box,
    render_callback_matches, renderer_create_window_sys, renderer_destroy_window_sys,
    renderer_render_window_sys, renderer_set_window_size_sys, renderer_swap_buffers_sys,
    unary_callback_matches,
};
use super::registry::{
    ViewportIdentity, fail_next_viewport_registration, preflight_runtime,
    register_test_viewport_data, register_viewport_data, unregister_viewport_data,
    viewport_data_count,
};
use super::runtime::{RuntimeControl, RuntimeState};
#[cfg(feature = "wgpu-30")]
use super::surface::supports_surface_format;
use super::surface::{
    SurfaceAction, SurfaceEvent, ViewportWgpuData, resolve_alpha_mode, resolve_present_mode,
    should_clear_viewport, surface_action, surface_config_from_capabilities,
};
use super::{OwningViewportRuntime, WgpuViewportError, logical_size_to_framebuffer};
use crate::{WgpuViewportSurfaceConfig, renderer::WgpuRenderer};

struct TestPlatformMarker;
struct TestPlatformAttachment;

impl ContextAttachment for TestPlatformAttachment {
    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(clear_test_main_handle_raw);
        Ok(())
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
    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.platform_phase_count
            .set(self.platform_phase_count.get() + 1);
        self.renderer_released_first.set(
            self.control
                .borrow()
                .as_ref()
                .is_some_and(|control| control.state() == RuntimeState::ResourceDropped),
        );
        context.with_bound_context(clear_test_main_handle_raw);
        Ok(())
    }
}

struct RegistryOrderingPlatformMarker;

struct RegistryOrderingPlatformAttachment {
    binding: ContextBinding,
    drops: Rc<Cell<u32>>,
    released_before_platform: Rc<Cell<bool>>,
}

impl ContextAttachment for RegistryOrderingPlatformAttachment {
    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.released_before_platform
            .set(self.drops.get() == 1 && viewport_data_count(self.binding.id()) == 0);
        context.with_bound_context(clear_test_main_handle_raw);
        Ok(())
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

#[derive(Clone, Copy)]
enum MissingRuntimeDependency {
    RendererCapability,
    PlatformCapability,
    PlatformCreate,
    PlatformDestroy,
    MainViewportHandle,
}

fn remove_runtime_dependency(context: &mut Context, dependency: MissingRuntimeDependency) {
    match dependency {
        MissingRuntimeDependency::RendererCapability => {
            let mut flags = context.io().backend_flags();
            flags.remove(BackendFlags::RENDERER_HAS_VIEWPORTS);
            context.io_mut().set_backend_flags(flags);
        }
        MissingRuntimeDependency::PlatformCapability => {
            let mut flags = context.io().backend_flags();
            flags.remove(BackendFlags::PLATFORM_HAS_VIEWPORTS);
            context.io_mut().set_backend_flags(flags);
        }
        MissingRuntimeDependency::PlatformCreate => unsafe {
            context
                .platform_io_mut()
                .set_platform_create_window_raw(None);
        },
        MissingRuntimeDependency::PlatformDestroy => unsafe {
            context
                .platform_io_mut()
                .set_platform_destroy_window_raw(None);
        },
        MissingRuntimeDependency::MainViewportHandle => unsafe {
            context
                .main_viewport()
                .set_platform_handle(std::ptr::null_mut());
        },
    }
}

fn assert_runtime_dependency_error(dependency: MissingRuntimeDependency, error: WgpuViewportError) {
    match dependency {
        MissingRuntimeDependency::RendererCapability => {
            assert!(matches!(
                error,
                WgpuViewportError::RendererViewportCapabilityLost
            ));
        }
        MissingRuntimeDependency::PlatformCapability => {
            assert!(matches!(
                error,
                WgpuViewportError::PlatformBackendUnavailable
            ));
        }
        MissingRuntimeDependency::PlatformCreate => assert!(matches!(
            error,
            WgpuViewportError::PlatformCallbackUnavailable {
                callback: "Platform_CreateWindow"
            }
        )),
        MissingRuntimeDependency::PlatformDestroy => assert!(matches!(
            error,
            WgpuViewportError::PlatformCallbackUnavailable {
                callback: "Platform_DestroyWindow"
            }
        )),
        MissingRuntimeDependency::MainViewportHandle => {
            assert!(matches!(
                error,
                WgpuViewportError::MainViewportHandleUnavailable
            ));
        }
    }
}

fn lock_context() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn viewport_identity(viewport: &mut sys::ImGuiViewport) -> ViewportIdentity {
    ViewportIdentity::capture(unsafe {
        dear_imgui_rs::platform_io::Viewport::from_raw_mut(viewport)
    })
}

fn publish_drop_probe(context: &Context, viewport: &mut sys::ImGuiViewport, drops: Rc<Cell<u32>>) {
    let identity = viewport_identity(viewport);
    let viewport = std::ptr::from_mut(viewport);
    publish_registered_box(
        Box::new(DropProbe(drops)),
        |pointer| register_test_viewport_data(&context.binding(), identity, pointer),
        |pointer| unsafe { (*viewport).RendererUserData = pointer.cast() },
    )
    .unwrap();
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

fn attach_registry_ordering_platform(
    context: &mut Context,
    drops: Rc<Cell<u32>>,
    released_before_platform: Rc<Cell<bool>>,
) -> TestPlatformLease {
    let binding = context.binding();
    let lease = context
        .register_attachment::<RegistryOrderingPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RegistryOrderingPlatformAttachment {
                binding: binding.clone(),
                drops,
                released_before_platform,
            }),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    TestPlatformLease {
        binding,
        _lease: lease,
    }
}

fn shutdown_returned_renderer(renderer: &mut WgpuRenderer, context: &mut Context) {
    renderer
        .shutdown(context)
        .expect("returned test renderer should remain usable");
}

fn configured_test_renderer(context: &mut Context) -> (WgpuRenderer, BackendFlags) {
    let (owned_flags, _) = WgpuRenderer::configure_imgui_context(context).unwrap();
    let mut renderer = WgpuRenderer::empty();
    renderer.bind_context(context, owned_flags).unwrap();
    renderer.renderer_consumer = Some(context.create_renderer_consumer().unwrap());
    (renderer, owned_flags)
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

fn replace_complete_renderer_takeover(context: &mut Context) {
    context
        .set_renderer_name(Some("foreign-renderer".to_owned()))
        .unwrap();
    unsafe {
        context
            .io_mut()
            .set_backend_renderer_user_data(std::ptr::dangling_mut::<u8>().cast());
        let platform_io = context.platform_io_mut();
        platform_io.set_draw_callback_reset_render_state_raw(Some(foreign_draw_callback));
        platform_io.set_draw_callback_set_sampler_linear_raw(Some(foreign_draw_callback));
        platform_io.set_draw_callback_set_sampler_nearest_raw(Some(foreign_draw_callback));
        platform_io.set_renderer_create_window_raw(Some(renderer_unary));
        platform_io.set_renderer_destroy_window_raw(Some(renderer_unary));
        platform_io.set_renderer_set_window_size_raw(Some(renderer_set_size));
        platform_io.set_renderer_render_window_raw(Some(renderer_render));
        platform_io.set_renderer_swap_buffers_raw(Some(renderer_render));
    }
}

fn assert_complete_renderer_takeover_preserves_viewport_capability(context: &Context) {
    assert_eq!(
        context.io().backend_renderer_name().unwrap().to_bytes(),
        b"foreign-renderer"
    );
    assert!(!context.io().backend_renderer_user_data().is_null());
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "a complete foreign renderer takeover owns its advertised viewport capability"
    );
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_TEXTURES | BackendFlags::RENDERER_HAS_VTX_OFFSET)
    );
    let platform_io = context.platform_io();
    for callback in [
        platform_io.draw_callback_reset_render_state_raw(),
        platform_io.draw_callback_set_sampler_linear_raw(),
        platform_io.draw_callback_set_sampler_nearest_raw(),
    ] {
        assert!(callback.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_draw_callback
                    as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
            )
        }));
    }
    assert!(unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_unary
    ));
    assert!(unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_unary
    ));
    assert!(platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_size));
    assert!(render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render
    ));
    assert!(render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_render
    ));
}

fn clear_complete_renderer_takeover(context: &mut Context) {
    context.set_renderer_name::<String>(None).unwrap();
    unsafe {
        context
            .io_mut()
            .set_backend_renderer_user_data(std::ptr::null_mut());
        let platform_io = context.platform_io_mut();
        platform_io.clear_renderer_handlers();
        platform_io.set_draw_callback_reset_render_state_raw(None);
        platform_io.set_draw_callback_set_sampler_linear_raw(None);
        platform_io.set_draw_callback_set_sampler_nearest_raw(None);
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
fn auto_no_vsync_is_preserved_for_secondary_viewports() {
    assert_eq!(
        resolve_present_mode(wgpu::PresentMode::AutoNoVsync, &[wgpu::PresentMode::Fifo]),
        wgpu::PresentMode::AutoNoVsync
    );
}

#[test]
fn secondary_surface_configuration_does_not_force_fifo() {
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let requested = WgpuViewportSurfaceConfig {
        present_mode: wgpu::PresentMode::AutoNoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        desired_maximum_frame_latency: 1,
    };
    let capabilities = capabilities_for_policy(
        format,
        vec![wgpu::PresentMode::Fifo],
        vec![wgpu::CompositeAlphaMode::Opaque],
    );

    let config =
        surface_config_from_capabilities(format, requested, &capabilities, [320, 200]).unwrap();
    assert_eq!(config.present_mode, wgpu::PresentMode::AutoNoVsync);
    assert_eq!(config.alpha_mode, wgpu::CompositeAlphaMode::Opaque);
    assert_eq!(config.desired_maximum_frame_latency, 1);
}

#[test]
fn unsupported_explicit_present_modes_keep_the_requested_vsync_policy() {
    assert_eq!(
        resolve_present_mode(wgpu::PresentMode::Immediate, &[wgpu::PresentMode::Fifo]),
        wgpu::PresentMode::AutoNoVsync
    );
    assert_eq!(
        resolve_present_mode(wgpu::PresentMode::FifoRelaxed, &[wgpu::PresentMode::Fifo]),
        wgpu::PresentMode::AutoVsync
    );
}

#[test]
fn unsupported_explicit_alpha_mode_falls_back_without_panicking() {
    assert_eq!(
        resolve_alpha_mode(
            wgpu::CompositeAlphaMode::PreMultiplied,
            &[wgpu::CompositeAlphaMode::Opaque]
        ),
        wgpu::CompositeAlphaMode::Auto
    );
    assert_eq!(
        resolve_alpha_mode(
            wgpu::CompositeAlphaMode::PreMultiplied,
            &[wgpu::CompositeAlphaMode::PreMultiplied]
        ),
        wgpu::CompositeAlphaMode::PreMultiplied
    );
}

fn capabilities_for_policy(
    format: wgpu::TextureFormat,
    present_modes: Vec<wgpu::PresentMode>,
    alpha_modes: Vec<wgpu::CompositeAlphaMode>,
) -> wgpu::SurfaceCapabilities {
    wgpu::SurfaceCapabilities {
        formats: vec![format],
        #[cfg(feature = "wgpu-30")]
        format_capabilities: vec![wgpu::SurfaceFormatCapabilities {
            format,
            color_spaces: wgpu::SurfaceColorSpaces::SRGB,
        }],
        present_modes,
        alpha_modes,
        usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
    }
}

#[test]
fn surface_config_is_renegotiated_from_current_capabilities() {
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let requested = WgpuViewportSurfaceConfig {
        present_mode: wgpu::PresentMode::Immediate,
        alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
        desired_maximum_frame_latency: 3,
    };
    let initial = capabilities_for_policy(
        format,
        vec![wgpu::PresentMode::Immediate, wgpu::PresentMode::Fifo],
        vec![wgpu::CompositeAlphaMode::PreMultiplied],
    );
    let initial_config =
        surface_config_from_capabilities(format, requested, &initial, [640, 480]).unwrap();
    assert_eq!(initial_config.present_mode, wgpu::PresentMode::Immediate);
    assert_eq!(
        initial_config.alpha_mode,
        wgpu::CompositeAlphaMode::PreMultiplied
    );

    let changed = capabilities_for_policy(
        format,
        vec![wgpu::PresentMode::Fifo],
        vec![wgpu::CompositeAlphaMode::Opaque],
    );
    let renegotiated =
        surface_config_from_capabilities(format, requested, &changed, [800, 600]).unwrap();
    assert_eq!(renegotiated.present_mode, wgpu::PresentMode::AutoNoVsync);
    assert_eq!(renegotiated.alpha_mode, wgpu::CompositeAlphaMode::Auto);
    assert_eq!([renegotiated.width, renegotiated.height], [800, 600]);
    assert_eq!(renegotiated.desired_maximum_frame_latency, 3);
    #[cfg(feature = "wgpu-30")]
    assert_eq!(renegotiated.color_space, wgpu::SurfaceColorSpace::Srgb);
}

#[cfg(feature = "wgpu-30")]
#[test]
fn secondary_surfaces_require_renderer_supported_srgb_output() {
    let srgb_format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let extended_linear_format = wgpu::TextureFormat::Rgba16Float;
    let capabilities = wgpu::SurfaceCapabilities {
        formats: vec![srgb_format],
        format_capabilities: vec![
            wgpu::SurfaceFormatCapabilities {
                format: srgb_format,
                color_spaces: wgpu::SurfaceColorSpaces::SRGB,
            },
            wgpu::SurfaceFormatCapabilities {
                format: extended_linear_format,
                color_spaces: wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR,
            },
        ],
        present_modes: vec![wgpu::PresentMode::Fifo],
        alpha_modes: vec![wgpu::CompositeAlphaMode::Opaque],
        usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
    };

    assert!(supports_surface_format(&capabilities, srgb_format));
    assert!(!supports_surface_format(
        &capabilities,
        extended_linear_format
    ));
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
fn existing_renderer_viewport_capability_rejects_attach_transactionally() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let (renderer, _) = configured_test_renderer(&mut context);
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    let failure = OwningViewportRuntime::attach_for_test(&mut context, renderer).unwrap_err();
    assert!(matches!(
        failure.error(),
        WgpuViewportError::RendererViewportCapabilityOccupied
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let mut renderer = failure.into_renderer();
    shutdown_returned_renderer(&mut renderer, &mut context);
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() & !BackendFlags::RENDERER_HAS_VIEWPORTS);
}

#[test]
fn every_occupied_renderer_slot_rejects_attach_without_partial_claim() {
    let _guard = lock_context();
    for slot in 0..5 {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let (renderer, _) = configured_test_renderer(&mut context);
        occupy_renderer_slot(&mut context, slot);

        let failure = OwningViewportRuntime::attach_for_test(&mut context, renderer).unwrap_err();
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
    let _platform = attach_test_platform(&mut context);
    let (mut runtime, _) = attach_configured_test_runtime(&mut context);
    let control = runtime.control_for_test();
    let drops = Rc::new(Cell::new(0));
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));
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
    assert_eq!(runtime.state_for_test(), RuntimeState::Attached);
    assert!(control.has_renderer_for_test());
    assert!(
        control
            .borrow_renderer_for_test()
            .as_ref()
            .is_some_and(|renderer| renderer.renderer_consumer.is_some()),
        "a rejected reset permit must leave the renderer consumer available for retry"
    );
    assert!(
        !context.io().backend_renderer_user_data().is_null()
            && context.io().backend_renderer_name().is_some(),
        "a rejected reset permit must not detach Context renderer bindings"
    );
    assert_eq!(
        drops.get(),
        0,
        "preflight failure must not destroy viewport sidecars"
    );
    assert_eq!(viewport_data_count(context.id()), 1);
    assert!(!unsafe { (*viewport).RendererUserData }.is_null());
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "preflight failure must keep the runtime capability claimed"
    );
    let platform_io = context.platform_io();
    assert!(unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys
    ));
    assert!(unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));
    assert!(
        platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
    );
    assert!(render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render_window_sys
    ));
    assert!(render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_swap_buffers_sys
    ));

    drop(snapshot);
    context.poll_snapshot_completions().unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(drops.get(), 1);
    assert!(context.io().backend_renderer_user_data().is_null());
    assert!(context.io().backend_renderer_name().is_none());
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
    let mut context = Context::create();
    let binding = context.binding();
    let identity = ViewportIdentity::capture(context.main_viewport());
    let drops = Rc::new(Cell::new(0));
    let published = Cell::new(false);
    fail_next_viewport_registration();

    let result = publish_registered_box(
        Box::new(DropProbe(Rc::clone(&drops))),
        |pointer| register_test_viewport_data(&binding, identity, pointer),
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
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    // Work callbacks are inert, while Destroy remains a cleanup entry and reports the foreign
    // slot without clearing it.
    unsafe { renderer_destroy_window_sys(&mut viewport) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert_eq!(viewport.RendererUserData, foreign);

    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn cleared_renderer_user_data_is_terminal_but_destroy_still_reclaims_sidecar() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let drops = Rc::new(Cell::new(0));
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));
    unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };

    let size = sys::ImVec2 { x: 32.0, y: 24.0 };
    unsafe { renderer_set_window_size_sys(viewport, &size) };

    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_SetWindowSize"
        })
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert_eq!(drops.get(), 0);
    assert_eq!(viewport_data_count(context.id()), 1);

    // Destroy remains reachable as cleanup while ordinary callbacks are fail-closed.
    unsafe { renderer_destroy_window_sys(viewport) };
    assert_eq!(drops.get(), 1);
    assert_eq!(viewport_data_count(context.id()), 0);
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        })
    ));

    // No sidecar plus a null slot is a valid idempotent Destroy.
    unsafe { renderer_destroy_window_sys(viewport) };
    assert_eq!(drops.get(), 1);
    assert!(runtime.poll_fault().is_ok());
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn shutdown_rejects_foreign_reachable_sidecar_before_mutating_runtime() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    let drops = Rc::new(Cell::new(0));
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));
    let owned = unsafe { (*viewport).RendererUserData };
    let foreign = std::ptr::dangling_mut::<c_void>();
    unsafe { (*viewport).RendererUserData = foreign };

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::Attached);
    assert!(control.has_renderer_for_test());
    assert!(control.transition_log_for_test().is_empty());
    assert_eq!(drops.get(), 0);
    assert_eq!(viewport_data_count(context.id()), 1);
    assert_eq!(unsafe { (*viewport).RendererUserData }, foreign);
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "a rejected ownership preflight must leave the capability publication intact"
    );
    assert!(unary_callback_matches(
        context.platform_io().renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));

    // The destructive helper itself is defensive: a future caller cannot accidentally ignore
    // ownership loss after adding a new shutdown path.
    assert!(matches!(
        control
            .binding()
            .try_with_bound_context(|| destroy_renderer_viewport_resources(&control)),
        Ok(Err(WgpuViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        }))
    ));
    assert_eq!(drops.get(), 0);
    assert_eq!(viewport_data_count(context.id()), 1);
    assert!(unary_callback_matches(
        context.platform_io().renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));

    // Restoring the exact slot makes the original runtime safely teardown-able again.
    unsafe { (*viewport).RendererUserData = owned };
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(drops.get(), 1);
}

#[test]
fn repeated_destroy_is_idempotent_after_exact_sidecar_cleanup() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let drops = Rc::new(Cell::new(0));
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));

    unsafe { renderer_destroy_window_sys(viewport) };
    assert_eq!(drops.get(), 1);
    assert_eq!(viewport_data_count(context.id()), 0);
    assert!(unsafe { (*viewport).RendererUserData }.is_null());
    assert!(runtime.poll_fault().is_ok());

    unsafe { renderer_destroy_window_sys(viewport) };
    assert_eq!(drops.get(), 1);
    assert!(runtime.poll_fault().is_ok());
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn context_teardown_drops_viewport_allocations_before_platform_windows() {
    let _guard = lock_context();
    let mut context = Context::create();
    let drops = Rc::new(Cell::new(0));
    let released_before_platform = Rc::new(Cell::new(false));
    let _platform = attach_registry_ordering_platform(
        &mut context,
        Rc::clone(&drops),
        Rc::clone(&released_before_platform),
    );
    let runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));

    drop(context);

    assert_eq!(drops.get(), 1);
    assert!(released_before_platform.get());
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    drop(runtime);
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
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "callback drift must revoke renderer viewport capability immediately"
    );
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
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "a detached runtime must not leave a renderer viewport capability behind"
    );
    unsafe { context.platform_io_mut().clear_renderer_handlers() };
}

#[test]
fn complete_foreign_renderer_takeover_preserves_viewport_capability_after_fault_and_shutdown() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let (mut runtime, _) = attach_configured_test_runtime(&mut context);
    replace_complete_renderer_takeover(&mut context);

    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::Renderer(
            crate::RendererError::RendererStateDrift {
                field: "BackendRendererUserData"
            }
        ))
    ));
    assert_complete_renderer_takeover_preserves_viewport_capability(&context);

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
    assert_complete_renderer_takeover_preserves_viewport_capability(&context);
    clear_complete_renderer_takeover(&mut context);
}

#[test]
fn ffi_callback_rejects_work_after_another_renderer_slot_drifts() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
    let mut viewport = sys::ImGuiViewport {
        RendererUserData: pointer.cast(),
        ..Default::default()
    };
    register_viewport_data(
        &context.binding(),
        viewport_identity(&mut viewport),
        pointer,
    )
    .unwrap();

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(renderer_unary));
    }
    unsafe { renderer_render_window_sys(&mut viewport, std::ptr::null_mut()) };
    unregister_viewport_data(pointer);

    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::RendererCallbackReplaced {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::RendererCallbackReplaced {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert!(unary_callback_matches(
        context.platform_io().renderer_destroy_window_raw(),
        renderer_unary
    ));
    unsafe { context.platform_io_mut().clear_renderer_handlers() };
}

#[test]
fn direct_callback_fail_closes_when_a_runtime_dependency_disappears() {
    let _guard = lock_context();
    for dependency in [
        MissingRuntimeDependency::RendererCapability,
        MissingRuntimeDependency::PlatformCapability,
        MissingRuntimeDependency::PlatformCreate,
        MissingRuntimeDependency::PlatformDestroy,
        MissingRuntimeDependency::MainViewportHandle,
    ] {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let mut runtime =
            OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
        remove_runtime_dependency(&mut context, dependency);

        let viewport = unsafe { sys::igGetMainViewport() };
        unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };

        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
            "a missing runtime dependency must revoke renderer viewport capability"
        );
        assert_runtime_dependency_error(dependency, runtime.poll_fault().unwrap_err());
        runtime.shutdown(&mut context).unwrap();
    }
}

#[test]
fn direct_callback_fail_closes_before_work_when_core_renderer_identity_drifts() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let foreign = std::ptr::dangling_mut::<c_void>();
    unsafe { context.io_mut().set_backend_renderer_user_data(foreign) };

    let viewport = unsafe { sys::igGetMainViewport() };
    unsafe { renderer_create_window_sys(viewport) };

    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::Renderer(
            crate::RendererError::RendererStateDrift {
                field: "BackendRendererUserData"
            }
        ))
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert_eq!(context.io().backend_renderer_user_data(), foreign);
    assert!(unsafe { (*viewport).RendererUserData }.is_null());

    runtime.shutdown(&mut context).unwrap();
    assert_eq!(context.io().backend_renderer_user_data(), foreign);
    unsafe {
        context
            .io_mut()
            .set_backend_renderer_user_data(std::ptr::null_mut())
    };
}

#[test]
fn rust_runtime_entry_records_core_drift_and_enters_shutdown() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let mut flags = context.io().backend_flags();
    flags.remove(BackendFlags::RENDERER_HAS_TEXTURES);
    context.io_mut().set_backend_flags(flags);

    assert!(matches!(
        runtime.new_frame(),
        Err(WgpuViewportError::Renderer(
            crate::RendererError::RendererStateDrift {
                field: "RENDERER_HAS_TEXTURES"
            }
        ))
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    runtime.shutdown(&mut context).unwrap();
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
    let mut viewport = sys::ImGuiViewport {
        RendererUserData: pointer.cast(),
        ..Default::default()
    };
    register_viewport_data(
        &context.binding(),
        viewport_identity(&mut viewport),
        pointer,
    )
    .unwrap();
    let renderer_borrow = control.borrow_renderer_for_test();

    unsafe { renderer_render_window_sys(&mut viewport, std::ptr::null_mut()) };
    drop(renderer_borrow);
    unregister_viewport_data(pointer);
    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::CallbackReentered {
            callback: "renderer contract validation"
        })
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
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
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn terminal_surface_fault_revokes_capability_and_stays_shutdown() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();

    control.record_entry_fault(WgpuViewportError::SurfaceRejected {
        event: "validation error",
    });

    assert!(matches!(
        runtime.poll_fault(),
        Err(WgpuViewportError::SurfaceRejected {
            event: "validation error"
        })
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.new_frame(),
        Err(WgpuViewportError::RuntimeDetached)
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
fn dropping_wrapper_defers_to_context_with_outstanding_snapshot_and_rejects_replacement() {
    let _guard = lock_context();
    let mut context = Context::create();
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
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
    let drops = Rc::new(Cell::new(0));
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));
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

    drop(runtime);

    assert_eq!(control.state(), RuntimeState::Attached);
    assert!(control.has_renderer_for_test());
    assert!(control.transition_log_for_test().is_empty());
    assert_eq!(drops.get(), 0);
    assert_eq!(viewport_data_count(context.id()), 1);
    assert!(
        !context.io().backend_renderer_user_data().is_null()
            && context.io().backend_renderer_name().is_some(),
        "wrapper Drop must not change the core renderer publication"
    );
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "wrapper Drop must not revoke the viewport capability"
    );
    assert!(unary_callback_matches(
        context.platform_io().renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));
    assert!(matches!(
        OwningViewportRuntime::attach(&mut context, WgpuRenderer::empty()),
        Err(error) if matches!(error.error(), WgpuViewportError::RuntimeAlreadyAttached)
    ));
    assert!(matches!(
        preflight_runtime(context.id()),
        Err(WgpuViewportError::RuntimeAlreadyAttached)
    ));
    assert_eq!(platform_phase_count.get(), 0);

    // The snapshot is intentionally outstanding during wrapper Drop. Its release does not grant
    // a replacement runtime ownership; only Context teardown may complete the deferred cleanup.
    drop(snapshot);
    context.poll_snapshot_completions().unwrap();
    assert_eq!(drops.get(), 0);
    assert_eq!(control.state(), RuntimeState::Attached);

    drop(context);

    assert_eq!(drops.get(), 1);
    assert!(renderer_released_first.get());
    assert_eq!(platform_phase_count.get(), 1);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(
        control.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
}

#[test]
fn dropping_wrapper_preserves_foreign_renderer_user_data_for_context_preflight() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let runtime =
        OwningViewportRuntime::attach_for_test(&mut context, WgpuRenderer::empty()).unwrap();
    let control = runtime.control_for_test();
    let drops = Rc::new(Cell::new(0));
    let viewport = unsafe { sys::igGetMainViewport() };
    publish_drop_probe(&context, unsafe { &mut *viewport }, Rc::clone(&drops));

    let owned = unsafe { (*viewport).RendererUserData };
    let foreign = std::ptr::dangling_mut::<c_void>();
    unsafe { (*viewport).RendererUserData = foreign };

    // Wrapper Drop has neither an exclusive Context nor permission to release a sidecar whose
    // native slot no longer proves ownership. It must leave the Context attachment intact so the
    // later renderer-resource phase can preflight before it changes callbacks or allocations.
    drop(runtime);

    assert_eq!(control.state(), RuntimeState::Attached);
    assert!(control.has_renderer_for_test());
    assert_eq!(drops.get(), 0);
    assert_eq!(unsafe { (*viewport).RendererUserData }, foreign);
    assert!(unary_callback_matches(
        context.platform_io().renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));

    // A real foreign takeover is fail-stop during Context teardown. Restore the test fixture's
    // exact pointer so this test can verify that deferred teardown remains able to reclaim it.
    unsafe { (*viewport).RendererUserData = owned };
    drop(context);

    assert_eq!(drops.get(), 1);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
}

fn attach_configured_test_runtime(context: &mut Context) -> (OwningViewportRuntime, BackendFlags) {
    let (renderer, owned_flags) = configured_test_renderer(context);
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

fn assert_foreign_backend_state_is_preserved(context: &mut Context, owned_flags: BackendFlags) {
    assert_eq!(
        context.io().backend_renderer_name().unwrap().to_bytes(),
        b"foreign-renderer"
    );
    assert!(
        !context.io().backend_flags().intersects(owned_flags),
        "WGPU-owned capability bits must not survive renderer teardown"
    );
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
}

fn assert_foreign_backend_state_remains_published_after_owner_drop(
    context: &Context,
    owned_flags: BackendFlags,
) {
    assert_eq!(
        context.io().backend_renderer_name().unwrap().to_bytes(),
        b"foreign-renderer"
    );
    assert!(
        context.io().backend_flags().contains(owned_flags),
        "wrapper Drop must not clear Context-owned renderer capabilities"
    );
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "wrapper Drop must not revoke the runtime viewport capability"
    );
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
            .is_some()
            && context
                .platform_io()
                .draw_callback_set_sampler_nearest_raw()
                .is_some(),
        "wrapper Drop must leave the remaining core renderer callbacks intact"
    );
    assert!(!context.platform_io().renderer_callbacks_are_empty());
}

#[test]
fn drop_leaves_foreign_backend_state_and_owned_publication_for_context_teardown() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let (runtime, owned_flags) = attach_configured_test_runtime(&mut context);
    replace_backend_state_with_foreign_values(&mut context);
    drop(runtime);

    assert_foreign_backend_state_remains_published_after_owner_drop(&context, owned_flags);
    drop(context);
}

#[test]
fn explicit_shutdown_preserves_foreign_backend_state_replacements() {
    let _guard = lock_context();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let (mut runtime, owned_flags) = attach_configured_test_runtime(&mut context);
    replace_backend_state_with_foreign_values(&mut context);

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WgpuViewportError::Renderer(
            crate::RendererError::RendererStateDrift {
                field: "BackendRendererName"
            }
        ))
    ));

    assert_foreign_backend_state_is_preserved(&mut context, owned_flags);
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
