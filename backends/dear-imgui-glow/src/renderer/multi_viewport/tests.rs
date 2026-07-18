use std::cell::Cell;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextTeardown, sys,
};

use super::callbacks::renderer_render_window_sys;
use super::runtime::RuntimeState;
use super::{GlowViewportError, GlowViewportRuntime};
use crate::{GlowRenderer, RenderError};
use crate::{
    shaders::Shaders, state::GlStateBackup, texture::SimpleTextureMap, versions::GlVersion,
};

static DELETED_TEXTURES: AtomicU32 = AtomicU32::new(0);

struct TestPlatformMarker;
struct TestPlatformAttachment;

impl ContextAttachment for TestPlatformAttachment {
    fn release_platform_windows(&self, _context: &ContextTeardown<'_>) {}
}

struct OrderingPlatformMarker;

struct OrderingPlatformAttachment {
    renderer_deletes_seen: Rc<Cell<u32>>,
}

impl ContextAttachment for OrderingPlatformAttachment {
    fn release_platform_windows(&self, _context: &ContextTeardown<'_>) {
        self.renderer_deletes_seen
            .set(DELETED_TEXTURES.load(Ordering::SeqCst));
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
    platform_io.set_platform_create_window_raw(Some(platform_unary));
    platform_io.set_platform_destroy_window_raw(Some(platform_unary));
    platform_io.set_platform_render_window_raw(Some(platform_render));
    platform_io.set_platform_swap_buffers_raw(Some(platform_render));
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
        state_backup: GlStateBackup::default(),
        vbo_handle: None,
        ebo_handle: None,
        owned_textures: owned_texture
            .then(|| glow::NativeTexture(NonZeroU32::new(91).unwrap()))
            .into_iter()
            .collect(),
        #[cfg(feature = "bind_vertex_array_support")]
        vertex_array_object: None,
        gl_version: GlVersion {
            major: 3,
            minor: 3,
            is_es: false,
        },
        has_clip_origin_support: false,
        is_destroyed: false,
        gl_context: gl,
        texture_map: Some(Box::new(SimpleTextureMap::default())),
        managed_textures: std::collections::HashMap::new(),
        renderer_consumer: Some(context.create_renderer_consumer().unwrap()),
        framebuffer_srgb: false,
        color_gamma_override: None,
        viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
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

    let failure = GlowViewportRuntime::attach(&mut context, renderer).unwrap_err();
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

    let failure = GlowViewportRuntime::attach(&mut context, renderer).unwrap_err();
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
    context
        .platform_io_mut()
        .set_platform_create_window_raw(Some(platform_unary));
    context
        .platform_io_mut()
        .set_platform_destroy_window_raw(Some(platform_unary));
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);

    let failure = GlowViewportRuntime::attach(&mut context, renderer).unwrap_err();
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
fn window_only_winit_platform_runtime_is_rejected() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    context
        .set_platform_name(Some("dear-imgui-winit test"))
        .unwrap();
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);

    let failure = GlowViewportRuntime::attach(&mut context, renderer).unwrap_err();
    assert!(matches!(
        failure.error(),
        GlowViewportError::PlatformGlContextUnsupported { backend }
            if backend == "dear-imgui-winit test"
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    let (_error, mut renderer) = failure.into_parts();
    destroy_returned_renderer(&mut renderer, &gl, &mut context);
}

#[derive(Clone, Copy)]
enum OccupiedRendererSlot {
    Create,
    Destroy,
    SetSize,
    Render,
    Swap,
}

fn occupy_renderer_slot(context: &mut Context, slot: OccupiedRendererSlot) {
    let platform_io = context.platform_io_mut();
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

        let failure = GlowViewportRuntime::attach(&mut context, renderer).unwrap_err();
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
fn moving_the_wrapper_keeps_runtime_owned_renderer_storage_stable() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
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
    assert!(context.font_atlas().build());
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
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
    assert_eq!(runtime.state_for_test(), RuntimeState::Detached);
    assert!(control.has_renderer_for_test());

    drop(snapshot);
    context.poll_snapshot_completions().unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
}

#[test]
fn foreign_callback_replacement_is_preserved_and_reported() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();

    context
        .platform_io_mut()
        .set_renderer_render_window_raw(Some(renderer_render));
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_RenderWindow"
        })
    ));
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
}

#[test]
fn foreign_callback_inserted_into_an_unclaimed_slot_is_preserved() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();

    context
        .platform_io_mut()
        .set_renderer_create_window_raw(Some(renderer_unary));
    assert!(matches!(
        runtime.poll_fault(),
        Err(GlowViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
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
}

#[test]
fn callback_reentry_is_deferred_to_the_next_rust_entry() {
    let _guard = test_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
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
    let mut runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
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
    let runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
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
fn dropping_wrapper_releases_resources_and_allows_a_new_runtime() {
    let _guard = test_guard();
    DELETED_TEXTURES.store(0, Ordering::SeqCst);
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let gl = fake_gl();
    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), true);
    let runtime = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
    let control = runtime.control_for_test();

    drop(runtime);

    assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 1);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert!(!control.has_renderer_for_test());
    assert_eq!(
        control.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );

    let renderer = test_renderer(&mut context, Some(Rc::clone(&gl)), false);
    let mut replacement = GlowViewportRuntime::attach(&mut context, renderer).unwrap();
    replacement.shutdown(&mut context).unwrap();
}

#[test]
fn renderer_failures_are_returned_as_typed_runtime_errors() {
    let error = GlowViewportError::Renderer(RenderError::RendererDestroyed);
    assert!(matches!(
        error,
        GlowViewportError::Renderer(RenderError::RendererDestroyed)
    ));
}
