use std::cell::Cell;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextTeardown, sys,
};

use super::callbacks::renderer_render_window_sys;
use super::runtime::RuntimeState;
use super::{GlowViewportError, GlowViewportRuntime};
use crate::renderer::callbacks::{
    draw_callback_reset_render_state, draw_callback_set_sampler_linear,
    draw_callback_set_sampler_nearest,
};
use crate::{GlowRenderer, RenderError};
use crate::{shaders::Shaders, texture::SimpleTextureMap, versions::GlVersion};

static DELETED_TEXTURES: AtomicU32 = AtomicU32::new(0);

struct TestPlatformMarker;
struct TestPlatformAttachment;

impl ContextAttachment for TestPlatformAttachment {
    fn release_platform_windows(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        Ok(())
    }
}

struct OrderingPlatformMarker;

struct OrderingPlatformAttachment {
    renderer_deletes_seen: Rc<Cell<u32>>,
}

impl ContextAttachment for OrderingPlatformAttachment {
    fn release_platform_windows(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.renderer_deletes_seen
            .set(DELETED_TEXTURES.load(Ordering::SeqCst));
        Ok(())
    }
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

unsafe extern "C" fn platform_unary(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn platform_render(_viewport: *mut sys::ImGuiViewport, _argument: *mut c_void) {}

unsafe extern "C" fn renderer_unary(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn renderer_set_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}

unsafe extern "C" fn renderer_render(_viewport: *mut sys::ImGuiViewport, _argument: *mut c_void) {}

fn claim_test_platform_callbacks(context: &mut Context) {
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_platform_create_window_raw(Some(platform_unary));
        platform_io.set_platform_destroy_window_raw(Some(platform_unary));
        platform_io.set_platform_render_window_raw(Some(platform_render));
        platform_io.set_platform_swap_buffers_raw(Some(platform_render));
    }
}

fn attach_test_platform(context: &mut Context) -> ContextAttachmentLease {
    let lease = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    lease
}

fn attach_ordering_platform(
    context: &mut Context,
    renderer_deletes_seen: Rc<Cell<u32>>,
) -> ContextAttachmentLease {
    let lease = context
        .register_attachment::<OrderingPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(OrderingPlatformAttachment {
                renderer_deletes_seen,
            }),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    lease
}

fn fake_gl() -> Rc<glow::Context> {
    unsafe extern "system" fn get_string(_name: u32) -> *const u8 {
        c"4.6".as_ptr().cast()
    }

    unsafe extern "system" fn get_string_i(_name: u32, _index: u32) -> *const u8 {
        c"".as_ptr().cast()
    }

    unsafe extern "system" fn get_integer(_name: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe { *value = 0 };
        }
    }

    unsafe extern "system" fn delete_textures(count: i32, _textures: *const u32) {
        DELETED_TEXTURES.fetch_add(count.max(0) as u32, Ordering::SeqCst);
    }

    Rc::new(unsafe {
        glow::Context::from_loader_function(|name| {
            match name {
                "glGetString" => get_string as *const (),
                "glGetStringi" => get_string_i as *const (),
                "glGetIntegerv" => get_integer as *const (),
                "glDeleteTextures" => delete_textures as *const (),
                _ => std::ptr::null(),
            }
            .cast()
        })
    })
}

fn test_renderer(
    context: &mut Context,
    gl: Option<Rc<glow::Context>>,
    owned_texture: bool,
) -> GlowRenderer {
    let renderer_consumer = context.create_renderer_consumer().unwrap();
    // The synthetic renderer has not installed a managed texture mapping yet; the fake native
    // texture below is added only after this empty reset transaction commits.
    let reset = context
        .prepare_renderer_texture_reset(&renderer_consumer)
        .unwrap();
    let _ = reset.commit();
    let mut flags = context.io().backend_flags();
    flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES);
    context.io_mut().set_backend_flags(flags);

    GlowRenderer {
        shaders: Shaders {
            program: None,
            attrib_location_tex: None,
            attrib_location_proj_mtx: None,
            attrib_location_color_gamma: None,
            attrib_location_vtx_pos: 0,
            attrib_location_vtx_uv: 0,
            attrib_location_vtx_color: 0,
        },
        vbo_handle: None,
        ebo_handle: None,
        owned_textures: owned_texture
            .then(|| glow::NativeTexture(NonZeroU32::new(91).unwrap()))
            .into_iter()
            .collect(),
        samplers: None,
        gl_version: GlVersion {
            major: 3,
            minor: 3,
            is_es: false,
        },
        has_clip_origin_support: false,
        has_separate_polygon_modes: false,
        has_sampler_object_support: true,
        is_destroyed: false,
        gl_context: gl,
        context_binding: None,
        backend_user_data: Box::default(),
        renderer_name_ptr: std::ptr::null(),
        renderer_texture_max: [0, 0],
        renderer_state_fault: None,
        synthetic_test_renderer: true,
        texture_map: Some(Box::new(SimpleTextureMap::default())),
        managed_textures: std::collections::HashMap::new(),
        destroyed_managed_textures: std::collections::HashMap::new(),
        renderer_consumer: Some(renderer_consumer),
        framebuffer_srgb: false,
        color_gamma_override: None,
        viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
    }
}

fn publish_test_renderer_core(context: &mut Context, renderer: &mut GlowRenderer) {
    renderer.synthetic_test_renderer = false;
    renderer.context_binding = Some(context.binding());
    context
        .set_renderer_name(Some("dear-imgui-glow test".to_owned()))
        .unwrap();
    renderer.renderer_name_ptr = context.io().backend_renderer_name().unwrap().as_ptr();
    unsafe {
        context
            .io_mut()
            .set_backend_renderer_user_data(renderer.backend_user_data_ptr());
        let platform_io = context.platform_io_mut();
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
    }
}

fn destroy_returned_renderer(
    renderer: &mut GlowRenderer,
    gl: &glow::Context,
    context: &mut Context,
) {
    renderer.destroy(gl, context).unwrap();
}

#[test]
fn attach_requires_a_registered_platform_role_and_returns_the_renderer() {
    let _guard = test_guard();
    let mut context = Context::create();
    claim_test_platform_callbacks(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);

    let failure = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap_err();
    assert!(matches!(
        failure.error(),
        GlowViewportError::Attachment(dear_imgui_rs::ContextAttachmentError::MissingPlatform)
    ));
    let (_error, mut renderer) = failure.into_parts();
    assert!(renderer.renderer_consumer.is_some());
    destroy_returned_renderer(&mut renderer, &gl, &mut context);
}

#[test]
fn external_context_renderer_is_rejected_without_losing_it() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, None, false);

    let failure = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap_err();
    assert!(matches!(
        failure.error(),
        GlowViewportError::ExternalContextUnsupported
    ));
    let (_error, mut renderer) = failure.into_parts();
    assert!(renderer.gl_context().is_none());
    destroy_returned_renderer(&mut renderer, &gl, &mut context);
}

#[test]
fn missing_platform_gl_callbacks_reject_attach_transactionally() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    unsafe {
        context
            .platform_io_mut()
            .set_platform_create_window_raw(Some(platform_unary));
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(platform_unary));
    }
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);

    let failure = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap_err();
    assert!(matches!(
        failure.error(),
        GlowViewportError::PlatformCallbackUnavailable {
            callback: "Platform_RenderWindow"
        }
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let (_error, mut renderer) = failure.into_parts();
    destroy_returned_renderer(&mut renderer, &gl, &mut context);
}

#[test]
fn preexisting_renderer_viewport_flag_rejects_attach_without_mutation() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut flags = context.io().backend_flags();
    flags.insert(BackendFlags::RENDERER_HAS_VIEWPORTS);
    context.io_mut().set_backend_flags(flags);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);

    let failure = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap_err();
    assert!(matches!(
        failure.error(),
        GlowViewportError::RendererViewportCapabilityOccupied
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let (_error, mut renderer) = failure.into_parts();
    destroy_returned_renderer(&mut renderer, &gl, &mut context);
}

#[test]
fn attach_exposes_the_platform_gl_context_contract_as_unsafe() {
    let attach: unsafe fn(
        &mut Context,
        GlowRenderer,
    ) -> Result<GlowViewportRuntime, super::GlowViewportAttachError> = GlowViewportRuntime::attach;
    let _ = attach;
}

#[test]
fn frame_trace_is_instance_bound_non_nested_and_drop_abortable() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();

    let trace = runtime.begin_frame_trace().unwrap();
    assert!(matches!(
        runtime.begin_frame_trace(),
        Err(GlowViewportError::FrameTraceAlreadyActive)
    ));
    control.record_rendered_viewport(17);
    control.record_rendered_viewport(17);
    control.record_rendered_viewport(9);
    let report = trace.finish();
    assert_eq!(
        report.rendered_viewports(),
        &[dear_imgui_rs::Id::from(9), dear_imgui_rs::Id::from(17)]
    );

    drop(runtime.begin_frame_trace().unwrap());
    let report = runtime.begin_frame_trace().unwrap().finish();
    assert!(report.rendered_viewports().is_empty());

    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn attach_rejects_core_drift_before_publishing_viewport_state() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let mut renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    publish_test_renderer_core(&mut context, &mut renderer);
    context
        .set_renderer_name(Some("foreign renderer".to_owned()))
        .unwrap();

    let failure = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap_err();
    assert!(matches!(
        failure.error(),
        GlowViewportError::Renderer(RenderError::RendererStateDrift {
            field: "BackendRendererName"
        })
    ));
    assert!(!context.io().backend_flags().intersects(
        BackendFlags::RENDERER_HAS_VTX_OFFSET
            | BackendFlags::RENDERER_HAS_TEXTURES
            | BackendFlags::RENDERER_HAS_VIEWPORTS
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());

    let (_error, mut renderer) = failure.into_parts();
    destroy_returned_renderer(&mut renderer, &gl, &mut context);
}

#[test]
fn runtime_texture_entry_fails_closed_on_core_drift() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let mut renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    publish_test_renderer_core(&mut context, &mut renderer);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();
    context
        .set_renderer_name(Some("foreign renderer".to_owned()))
        .unwrap();

    assert!(matches!(
        runtime.register_texture(
            1,
            1,
            dear_imgui_rs::TextureFormat::RGBA32,
            &[255, 255, 255, 255],
        ),
        Err(GlowViewportError::Renderer(
            RenderError::RendererStateDrift {
                field: "BackendRendererName"
            }
        ))
    ));
    {
        let mut renderer = control.borrow_renderer_for_test();
        assert!(matches!(
            renderer.as_deref_mut().unwrap().ensure_operational(),
            Err(RenderError::RendererStateDrift {
                field: "BackendRendererName"
            })
        ));
    }
    assert!(!context.io().backend_flags().intersects(
        BackendFlags::RENDERER_HAS_VTX_OFFSET
            | BackendFlags::RENDERER_HAS_TEXTURES
            | BackendFlags::RENDERER_HAS_VIEWPORTS
    ));
    let _ = runtime.shutdown(&mut context);
}

#[test]
fn direct_c_entry_fails_closed_on_first_core_drift() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let mut renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    publish_test_renderer_core(&mut context, &mut renderer);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();
    context
        .set_renderer_name(Some("foreign renderer".to_owned()))
        .unwrap();
    runtime.panic_next_callback_for_test();

    let viewport = unsafe { sys::igGetMainViewport() };
    unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };

    assert!(control.callback_panic_pending_for_test());
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(!context.io().backend_flags().intersects(
        BackendFlags::RENDERER_HAS_VTX_OFFSET
            | BackendFlags::RENDERER_HAS_TEXTURES
            | BackendFlags::RENDERER_HAS_VIEWPORTS
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::Renderer(
            RenderError::RendererStateDrift {
                field: "BackendRendererName"
            }
        ))
    ));
    let _ = runtime.shutdown(&mut context);
}

#[test]
fn ordinary_callback_render_failure_does_not_shutdown_the_runtime() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();

    control.with_renderer_callback("test render failure", |_, _| {
        Err(RenderError::OpenGLError(
            "injected rendering failure".to_owned(),
        ))
    });

    assert_eq!(runtime.state_for_test(), RuntimeState::Attached);
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::Renderer(RenderError::OpenGLError(message)))
            if message == "injected rendering failure"
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[derive(Clone, Copy)]
enum OccupiedRendererSlot {
    Create,
    Destroy,
    SetSize,
    Render,
    Swap,
}

impl OccupiedRendererSlot {
    const fn callback_name(self) -> &'static str {
        match self {
            Self::Create => "Renderer_CreateWindow",
            Self::Destroy => "Renderer_DestroyWindow",
            Self::SetSize => "Renderer_SetWindowSize",
            Self::Render => "Renderer_RenderWindow",
            Self::Swap => "Renderer_SwapBuffers",
        }
    }
}

fn occupy_renderer_slot(context: &mut Context, slot: OccupiedRendererSlot) {
    let platform_io = context.platform_io_mut();
    unsafe {
        match slot {
            OccupiedRendererSlot::Create => {
                platform_io.set_renderer_create_window_raw(Some(renderer_unary));
            }
            OccupiedRendererSlot::Destroy => {
                platform_io.set_renderer_destroy_window_raw(Some(renderer_unary));
            }
            OccupiedRendererSlot::SetSize => {
                platform_io.set_renderer_set_window_size_raw(Some(renderer_set_size));
            }
            OccupiedRendererSlot::Render => {
                platform_io.set_renderer_render_window_raw(Some(renderer_render));
            }
            OccupiedRendererSlot::Swap => {
                platform_io.set_renderer_swap_buffers_raw(Some(renderer_render));
            }
        }
    }
}

#[derive(Clone, Copy)]
enum MissingRuntimeDependency {
    RendererCapability,
    PlatformCapability,
    PlatformCreate,
    PlatformDestroy,
    PlatformRender,
    PlatformSwap,
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
        MissingRuntimeDependency::PlatformRender => unsafe {
            context
                .platform_io_mut()
                .set_platform_render_window_raw(None);
        },
        MissingRuntimeDependency::PlatformSwap => unsafe {
            context
                .platform_io_mut()
                .set_platform_swap_buffers_raw(None);
        },
    }
}

fn assert_dependency_error(dependency: MissingRuntimeDependency, error: GlowViewportError) {
    match dependency {
        MissingRuntimeDependency::RendererCapability => {
            assert!(matches!(
                error,
                GlowViewportError::RendererViewportCapabilityLost
            ));
        }
        MissingRuntimeDependency::PlatformCapability => {
            assert!(matches!(
                error,
                GlowViewportError::PlatformBackendUnavailable
            ));
        }
        MissingRuntimeDependency::PlatformCreate => assert!(matches!(
            error,
            GlowViewportError::PlatformCallbackUnavailable {
                callback: "Platform_CreateWindow"
            }
        )),
        MissingRuntimeDependency::PlatformDestroy => assert!(matches!(
            error,
            GlowViewportError::PlatformCallbackUnavailable {
                callback: "Platform_DestroyWindow"
            }
        )),
        MissingRuntimeDependency::PlatformRender => assert!(matches!(
            error,
            GlowViewportError::PlatformCallbackUnavailable {
                callback: "Platform_RenderWindow"
            }
        )),
        MissingRuntimeDependency::PlatformSwap => assert!(matches!(
            error,
            GlowViewportError::PlatformCallbackUnavailable {
                callback: "Platform_SwapBuffers"
            }
        )),
    }
}

#[test]
fn every_occupied_renderer_slot_rejects_attach_without_partial_claim() {
    let _guard = test_guard();
    for slot in [
        OccupiedRendererSlot::Create,
        OccupiedRendererSlot::Destroy,
        OccupiedRendererSlot::SetSize,
        OccupiedRendererSlot::Render,
        OccupiedRendererSlot::Swap,
    ] {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        occupy_renderer_slot(&mut context, slot);
        let gl = fake_gl();
        let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);

        let failure = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap_err();
        assert!(matches!(
            failure.error(),
            GlowViewportError::RendererCallbackOccupied { .. }
        ));
        assert!(!context.platform_io().renderer_callbacks_are_empty());
        let (_error, mut renderer) = failure.into_parts();
        destroy_returned_renderer(&mut renderer, &gl, &mut context);
        drop(context);
    }
}

#[test]
fn direct_callback_fail_closes_before_rendering_for_every_foreign_slot() {
    let _guard = test_guard();
    for slot in [
        OccupiedRendererSlot::Create,
        OccupiedRendererSlot::Destroy,
        OccupiedRendererSlot::SetSize,
        OccupiedRendererSlot::Render,
        OccupiedRendererSlot::Swap,
    ] {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let gl = fake_gl();
        let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
        let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
        let control = runtime.control_for_test();
        runtime.panic_next_callback_for_test();
        occupy_renderer_slot(&mut context, slot);

        let viewport = unsafe { sys::igGetMainViewport() };
        unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };

        assert!(control.callback_panic_pending_for_test());
        assert_eq!(
            context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
            matches!(slot, OccupiedRendererSlot::Render)
        );
        assert!(matches!(
            runtime.poll_fault(),
            Err(GlowViewportError::RendererCallbackReplaced { callback })
                if callback == slot.callback_name()
        ));
        assert!(matches!(
            runtime.shutdown(&mut context),
            Err(GlowViewportError::RendererCallbackReplaced { callback })
                if callback == slot.callback_name()
        ));
    }
}

#[test]
fn direct_callback_fail_closes_when_any_platform_dependency_disappears() {
    let _guard = test_guard();
    for dependency in [
        MissingRuntimeDependency::RendererCapability,
        MissingRuntimeDependency::PlatformCapability,
        MissingRuntimeDependency::PlatformCreate,
        MissingRuntimeDependency::PlatformDestroy,
        MissingRuntimeDependency::PlatformRender,
        MissingRuntimeDependency::PlatformSwap,
    ] {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let gl = fake_gl();
        let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
        let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
        let control = runtime.control_for_test();
        runtime.panic_next_callback_for_test();
        remove_runtime_dependency(&mut context, dependency);

        let viewport = unsafe { sys::igGetMainViewport() };
        unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };

        assert!(control.callback_panic_pending_for_test());
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
        );
        assert_dependency_error(dependency, runtime.poll_fault().unwrap_err());
        runtime.shutdown(&mut context).unwrap();
    }
}

#[test]
fn rust_entry_fail_closes_when_a_platform_callback_disappears() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    remove_runtime_dependency(&mut context, MissingRuntimeDependency::PlatformRender);

    assert_dependency_error(
        MissingRuntimeDependency::PlatformRender,
        runtime.new_frame().unwrap_err(),
    );
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn moving_the_wrapper_keeps_runtime_owned_renderer_storage_stable() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let mut renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    publish_test_renderer_core(&mut context, &mut renderer);
    let runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let renderer_address = runtime.renderer_address_for_test();

    fn move_runtime(runtime: GlowViewportRuntime) -> GlowViewportRuntime {
        runtime
    }
    let mut moved = move_runtime(runtime);
    assert_eq!(moved.renderer_address_for_test(), renderer_address);
    assert_eq!(moved.state_for_test(), RuntimeState::Attached);

    let mut viewport = sys::ImGuiViewport {
        Flags: dear_imgui_rs::ViewportFlags::NO_RENDERER_CLEAR.bits(),
        ..Default::default()
    };
    unsafe { renderer_render_window_sys(&mut viewport, std::ptr::null_mut()) };
    moved.poll_fault().unwrap();

    moved.shutdown(&mut context).unwrap();
    moved.shutdown(&mut context).unwrap();
    assert_eq!(moved.state_for_test(), RuntimeState::ResourceDropped);
    assert_eq!(
        moved.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
}

#[test]
fn shutdown_with_an_outstanding_snapshot_keeps_the_renderer_for_retry() {
    let _guard = test_guard();
    let mut context = Context::create();
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    DELETED_TEXTURES.store(0, Ordering::SeqCst);
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), true);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
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
        Err(GlowViewportError::Renderer(RenderError::RendererConsumer(
            dear_imgui_rs::render::RendererConsumerError::OutstandingEpochs { count: 1 }
        )))
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::Attached);
    assert!(control.has_renderer_for_test());
    assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 0);
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(std::ptr::fn_addr_eq(
        context.platform_io().renderer_render_window_raw().unwrap(),
        renderer_render_window_sys as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
    ));
    {
        let renderer = control.borrow_renderer_for_test();
        let renderer = renderer.as_ref().unwrap();
        assert_eq!(renderer.owned_textures.len(), 1);
        assert!(!renderer.is_destroyed);
        assert!(renderer.renderer_consumer.is_some());
    }

    drop(snapshot);
    context.poll_snapshot_completions().unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 1);
}

#[test]
fn foreign_callback_replacement_is_preserved_and_reported() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_render_window_raw(Some(renderer_render));
    }
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_RenderWindow"
        })
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(std::ptr::fn_addr_eq(
        context.platform_io().renderer_render_window_raw().unwrap(),
        renderer_render as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
    ));

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_RenderWindow"
        })
    ));
    assert!(std::ptr::fn_addr_eq(
        context.platform_io().renderer_render_window_raw().unwrap(),
        renderer_render as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
}

#[test]
fn complete_foreign_renderer_takeover_preserves_viewport_capability_on_shutdown() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();

    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_renderer_create_window_raw(Some(renderer_unary));
        platform_io.set_renderer_destroy_window_raw(Some(renderer_unary));
        platform_io.set_renderer_set_window_size_raw(Some(renderer_set_size));
        platform_io.set_renderer_render_window_raw(Some(renderer_render));
        platform_io.set_renderer_swap_buffers_raw(Some(renderer_render));
    }

    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
    let platform_io = context.platform_io();
    assert!(std::ptr::fn_addr_eq(
        platform_io.renderer_create_window_raw().unwrap(),
        renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport)
    ));
    assert!(std::ptr::fn_addr_eq(
        platform_io.renderer_destroy_window_raw().unwrap(),
        renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport)
    ));
    assert!(unsafe { (&*platform_io.as_raw()).Renderer_SetWindowSize }.is_some());
    assert!(std::ptr::fn_addr_eq(
        platform_io.renderer_render_window_raw().unwrap(),
        renderer_render as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
    ));
    assert!(std::ptr::fn_addr_eq(
        platform_io.renderer_swap_buffers_raw().unwrap(),
        renderer_render as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
}

#[test]
fn foreign_callback_inserted_into_an_unclaimed_slot_is_preserved() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_create_window_raw(Some(renderer_unary));
    }
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
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
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
    assert!(std::ptr::fn_addr_eq(
        context.platform_io().renderer_create_window_raw().unwrap(),
        renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport)
    ));
    assert!(context.platform_io().renderer_render_window_raw().is_none());
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
}

#[test]
fn callback_reentry_is_deferred_to_the_next_rust_entry() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();
    let renderer_borrow = control.borrow_renderer_for_test();
    let viewport = unsafe { sys::igGetMainViewport() };

    unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };
    drop(renderer_borrow);
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::CallbackReentered {
            callback: "Renderer_RenderWindow"
        })
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn callback_panic_is_contained_and_deferred() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    runtime.panic_next_callback_for_test();
    let viewport = unsafe { sys::igGetMainViewport() };

    unsafe { renderer_render_window_sys(viewport, std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::CallbackPanicked {
            callback: "Renderer_RenderWindow"
        })
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn context_first_shutdown_drops_gpu_resources_before_platform_teardown() {
    let _guard = test_guard();
    DELETED_TEXTURES.store(0, Ordering::SeqCst);
    let mut context = Context::create();
    let renderer_deletes_seen = Rc::new(Cell::new(0));
    let _platform = attach_ordering_platform(&mut context, Rc::clone(&renderer_deletes_seen));
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), true);
    let runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();

    drop(context);

    assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 1);
    assert_eq!(renderer_deletes_seen.get(), 1);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(
        control.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
    drop(runtime);
}

#[test]
fn dropping_wrapper_defers_resource_release_to_context_teardown() {
    let _guard = test_guard();
    DELETED_TEXTURES.store(0, Ordering::SeqCst);
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), true);
    let runtime = unsafe { GlowViewportRuntime::attach(&mut context, renderer) }.unwrap();
    let control = runtime.control_for_test();

    drop(runtime);

    assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 0);
    assert_eq!(control.state(), RuntimeState::Attached);
    assert!(control.has_renderer_for_test());
    assert!(control.transition_log_for_test().is_empty());

    drop(context);

    assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 1);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(
        control.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
}

#[test]
fn renderer_failures_are_returned_as_typed_runtime_errors() {
    let error = GlowViewportError::Renderer(RenderError::RendererDestroyed);
    assert!(matches!(
        error,
        GlowViewportError::Renderer(RenderError::RendererDestroyed)
    ));
}
