use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[cfg(feature = "multi-viewport")]
use std::ffi::{c_char, c_void};
#[cfg(feature = "multi-viewport")]
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    Context, ContextAttachment, ContextAttachmentDetachError, ContextAttachmentError,
    ContextAttachmentRole, ContextBindingError, ContextDestroyed,
    ContextPlatformAttachmentReleaseError, ContextTeardown, binding::with_bound_context,
};

struct PlatformMarker;
struct RendererMarker;
struct ExtensionMarker;
struct PanickingExtensionMarker;
struct PanickingRendererExtensionMarker;
struct PanickingDropPlatformMarker;
struct PanickingDropRendererMarker;

struct PanickingDropAttachment;

impl ContextAttachment for PanickingDropAttachment {}

impl Drop for PanickingDropAttachment {
    fn drop(&mut self) {
        panic!("attachment destructor panic");
    }
}

#[cfg(feature = "multi-viewport")]
struct TestViewportPlatformMarker;

#[cfg(feature = "multi-viewport")]
struct TestViewportPlatformAttachment;

#[cfg(feature = "multi-viewport")]
struct ExplicitPlatformWindowTeardownMarker;

#[cfg(feature = "multi-viewport")]
struct ExplicitPlatformWindowTeardownAttachment {
    log: Rc<RefCell<Vec<&'static str>>>,
    reject: Rc<Cell<bool>>,
    reject_after_native_teardown: Rc<Cell<bool>>,
    bound: Rc<Cell<bool>>,
}

#[cfg(feature = "multi-viewport")]
impl ContextAttachment for ExplicitPlatformWindowTeardownAttachment {
    fn begin_platform_window_teardown(
        &self,
        context: &super::ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            self.bound
                .set(!unsafe { crate::sys::igGetCurrentContext() }.is_null());
        });
        self.log.borrow_mut().push("begin");
        if self.reject.get() {
            return Err(super::ContextAttachmentTeardownError::new(
                "test platform teardown rejection",
            ));
        }
        Ok(())
    }

    fn end_platform_window_teardown(
        &self,
        _context: &super::ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        self.log.borrow_mut().push("end");
        if self.reject_after_native_teardown.get() {
            return Err(super::ContextAttachmentTeardownError::new(
                "test platform teardown postflight rejection",
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "multi-viewport")]
impl ContextAttachment for TestViewportPlatformAttachment {
    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        context.with_bound_context(|| unsafe {
            crate::sys::igDestroyPlatformWindows();
        });
        Ok(())
    }
}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn test_platform_unary(_viewport: *mut crate::sys::ImGuiViewport) {}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn test_platform_get_vec2(
    _viewport: *mut crate::sys::ImGuiViewport,
    output: *mut crate::sys::ImVec2,
) {
    if let Some(output) = unsafe { output.as_mut() } {
        *output = crate::sys::ImVec2 { x: 0.0, y: 0.0 };
    }
}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn test_platform_set_vec2(
    _viewport: *mut crate::sys::ImGuiViewport,
    _value: *const crate::sys::ImVec2,
) {
}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn test_platform_set_title(
    _viewport: *mut crate::sys::ImGuiViewport,
    _title: *const c_char,
) {
}

#[cfg(feature = "multi-viewport")]
static TEST_RENDER_WINDOW_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn test_renderer_render_window(
    _viewport: *mut crate::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    TEST_RENDER_WINDOW_CALLS.fetch_add(1, Ordering::SeqCst);
}

#[cfg(feature = "multi-viewport")]
fn install_complete_test_viewport_backend(ctx: &mut Context) -> super::ContextAttachmentLease {
    let lease = ctx
        .register_attachment::<TestViewportPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestViewportPlatformAttachment),
        )
        .expect("test platform attachment must be unique");
    let mut backend_flags = ctx.io().backend_flags();
    backend_flags.insert(
        crate::BackendFlags::PLATFORM_HAS_VIEWPORTS | crate::BackendFlags::RENDERER_HAS_VIEWPORTS,
    );
    ctx.io_mut().set_backend_flags(backend_flags);

    let mut monitor = crate::sys::ImGuiPlatformMonitor::default();
    monitor.MainSize = crate::sys::ImVec2 {
        x: 1920.0,
        y: 1080.0,
    };
    monitor.WorkSize = monitor.MainSize;
    monitor.DpiScale = 1.0;

    unsafe {
        let platform_io = ctx.platform_io_mut();
        platform_io.set_platform_create_window_raw(Some(test_platform_unary));
        platform_io.set_platform_destroy_window_raw(Some(test_platform_unary));
        platform_io.set_platform_show_window_raw(Some(test_platform_unary));
        platform_io.set_platform_get_window_pos_raw(Some(test_platform_get_vec2));
        platform_io.set_platform_set_window_pos_raw(Some(test_platform_set_vec2));
        platform_io.set_platform_get_window_size_raw(Some(test_platform_get_vec2));
        platform_io.set_platform_set_window_size_raw(Some(test_platform_set_vec2));
        platform_io.set_platform_set_window_title_raw(Some(test_platform_set_title));
        platform_io.set_monitors(&[monitor]);
        ctx.main_viewport()
            .set_platform_handle(std::ptr::dangling_mut::<c_void>());
    }
    lease
}

struct RecordingAttachment {
    log: Rc<RefCell<Vec<&'static str>>>,
    binding: Option<super::ContextBinding>,
    ordinary_rejected: Rc<Cell<bool>>,
    teardown_bound: Rc<Cell<bool>>,
    frame_closed_before_quiesce: Rc<Cell<bool>>,
    inspect_native_state_during_quiesce: bool,
    panic_during_quiesce: bool,
    fail_during_renderer_release: bool,
}

impl RecordingAttachment {
    fn new(log: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            log,
            binding: None,
            ordinary_rejected: Rc::new(Cell::new(false)),
            teardown_bound: Rc::new(Cell::new(false)),
            frame_closed_before_quiesce: Rc::new(Cell::new(false)),
            inspect_native_state_during_quiesce: true,
            panic_during_quiesce: false,
            fail_during_renderer_release: false,
        }
    }
}

impl ContextAttachment for RecordingAttachment {
    fn quiesce(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        assert_eq!(context.phase(), super::ContextAttachmentPhase::Quiesce);
        if self.inspect_native_state_during_quiesce {
            context.with_bound_context(|| {
                self.frame_closed_before_quiesce
                    .set(!unsafe { (*context.as_raw_for_test()).WithinFrameScope });
            });
        }
        if let Some(binding) = &self.binding {
            self.ordinary_rejected.set(matches!(
                binding.try_with_bound_context(|| ()),
                Err(ContextBindingError::Dropping)
            ));
            context.with_bound_context(|| {
                self.teardown_bound
                    .set(unsafe { crate::sys::igGetCurrentContext() } == context.as_raw_for_test());
            });
        }
        self.log.borrow_mut().push("quiesce");
        assert!(!self.panic_during_quiesce, "attachment panic");
        Ok(())
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        assert_eq!(
            context.phase(),
            super::ContextAttachmentPhase::RendererResources
        );
        self.log.borrow_mut().push("renderer");
        if self.fail_during_renderer_release {
            return Err(super::ContextAttachmentTeardownError::new(
                "renderer attachment failure",
            ));
        }
        Ok(())
    }

    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        assert_eq!(
            context.phase(),
            super::ContextAttachmentPhase::PlatformWindows
        );
        self.log.borrow_mut().push("platform");
        Ok(())
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.log.borrow_mut().push("post");
    }
}

struct RendererTextureResetMarker;
struct WrongPhaseRendererTextureResetMarker;

#[derive(Debug)]
struct RendererTextureResetObservation {
    release_calls: Cell<usize>,
    release_saw_expected_binding: Cell<bool>,
    reset_rejected: Cell<bool>,
    invalidated: Cell<Option<usize>>,
    binding_after_call: Cell<crate::TextureId>,
    nested_reset_rejected: Cell<bool>,
}

impl RendererTextureResetObservation {
    fn new() -> Self {
        Self {
            release_calls: Cell::new(0),
            release_saw_expected_binding: Cell::new(false),
            reset_rejected: Cell::new(false),
            invalidated: Cell::new(None),
            binding_after_call: Cell::new(crate::TextureId::null()),
            nested_reset_rejected: Cell::new(false),
        }
    }
}

struct RendererTextureResetAttachment {
    consumer: crate::render::RendererConsumer,
    expected_binding: crate::TextureId,
    release_fails: bool,
    attempts_reentry: bool,
    observation: Rc<RendererTextureResetObservation>,
}

impl ContextAttachment for RendererTextureResetAttachment {
    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        let consumer = &self.consumer;
        let expected_binding = self.expected_binding;
        let observation = Rc::clone(&self.observation);
        let result = context.with_renderer_texture_reset(consumer, || {
            observation
                .release_calls
                .set(observation.release_calls.get().saturating_add(1));
            observation
                .release_saw_expected_binding
                .set(font_texture_id_during_teardown(context) == expected_binding);

            if self.attempts_reentry {
                observation.nested_reset_rejected.set(
                    context
                        .with_renderer_texture_reset(consumer, || Ok(()))
                        .is_err(),
                );
            }
            if self.release_fails {
                return Err(super::ContextAttachmentTeardownError::new(
                    "test renderer resource release failed",
                ));
            }
            Ok(())
        });

        match result {
            Ok(invalidated) => self.observation.invalidated.set(Some(invalidated)),
            Err(_) => self.observation.reset_rejected.set(true),
        }
        self.observation
            .binding_after_call
            .set(font_texture_id_during_teardown(context));
        Ok(())
    }
}

struct WrongPhaseRendererTextureResetAttachment {
    consumer: crate::render::RendererConsumer,
    expected_binding: crate::TextureId,
    observation: Rc<RendererTextureResetObservation>,
}

impl ContextAttachment for WrongPhaseRendererTextureResetAttachment {
    fn quiesce(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), super::ContextAttachmentTeardownError> {
        let result = context.with_renderer_texture_reset(&self.consumer, || {
            self.observation
                .release_calls
                .set(self.observation.release_calls.get().saturating_add(1));
            Ok(())
        });
        self.observation.reset_rejected.set(result.is_err());
        self.observation
            .binding_after_call
            .set(font_texture_id_during_teardown(context));
        assert_eq!(
            self.observation.binding_after_call.get(),
            self.expected_binding,
            "a wrong-phase reset attempt must not touch native bindings"
        );
        Ok(())
    }
}

fn font_texture_id_during_teardown(context: &ContextTeardown<'_>) -> crate::TextureId {
    context.with_bound_context(|| unsafe {
        let io = crate::sys::igGetIO_Nil();
        assert!(!io.is_null(), "teardown Context must retain ImGuiIO");
        let atlas = (*io).Fonts;
        assert!(
            !atlas.is_null(),
            "teardown Context must retain the font atlas"
        );
        crate::texture::effective_texture_id(&(*atlas).TexRef)
    })
}

fn prepare_managed_font_atlas(
    context: &mut Context,
) -> (crate::render::RendererConsumer, crate::TextureId) {
    context.prepare_frame(
        super::FramePrepareOptions::new([320.0, 240.0], 1.0 / 60.0).renderer_has_textures(),
    );
    assert!(context.font_atlas().build());
    let consumer = context
        .create_renderer_consumer()
        .expect("test Context must create a renderer consumer");

    let frame = context.begin_frame();
    frame.ui().text("initialize the managed font atlas");
    let mut rendered = frame.render();
    let binding = crate::TextureId::new(0xC0FFEE);
    let feedback = rendered
        .texture_requests()
        .iter()
        .find(|request| {
            matches!(
                request.texture(),
                crate::render::SnapshotTextureId::FontAtlas { .. }
            )
        })
        .expect("first managed frame must request the font atlas")
        .uploaded(binding)
        .expect("font atlas upload feedback must match the request");
    rendered
        .reconcile_texture_feedback([feedback])
        .expect("test font atlas feedback must reconcile");
    drop(rendered);

    assert_eq!(context.font_atlas().texture_id(), binding);
    (consumer, binding)
}

fn prepare_managed_font_atlas_for_detached_rendering(
    context: &mut Context,
) -> (crate::render::RendererConsumer, crate::TextureId) {
    context.prepare_frame(
        super::FramePrepareOptions::new([320.0, 240.0], 1.0 / 60.0).renderer_has_textures(),
    );
    assert!(context.font_atlas().build());
    let consumer = context
        .create_renderer_consumer()
        .expect("test Context must create a renderer consumer");

    let frame = context.begin_frame();
    frame.ui().text("initialize detached managed font atlas");
    let snapshot = frame
        .render_snapshot(&consumer)
        .expect("test frame must create a detached snapshot");
    let binding = crate::TextureId::new(0xD37AC4ED);
    let feedback = snapshot
        .texture_requests()
        .iter()
        .find(|request| {
            matches!(
                request.texture(),
                crate::render::SnapshotTextureId::FontAtlas { .. }
            )
        })
        .expect("first managed snapshot must request the font atlas")
        .uploaded(binding)
        .expect("font atlas upload feedback must match the request");
    snapshot
        .commit([feedback])
        .expect("test snapshot completion must reach the Context");
    context
        .poll_snapshot_completions()
        .expect("test snapshot completion must reconcile");

    assert_eq!(context.font_atlas().texture_id(), binding);
    (consumer, binding)
}

#[test]
fn platform_io_shared_and_mut_views_match() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let shared = ctx.platform_io().as_raw();
    let mutable = ctx.platform_io_mut().as_raw();
    assert_eq!(shared, mutable);
    let raw = ctx.as_raw();
    let native_io = unsafe { crate::sys::igGetIO_ContextPtr(raw) };
    let embedded_io = unsafe { std::ptr::addr_of!((*raw).IO) };
    assert_eq!(
        native_io,
        embedded_io.cast_mut(),
        "the generated ImGuiContext IO layout must match the linked native library"
    );
    assert_eq!(
        shared,
        unsafe { std::ptr::addr_of!((*ctx.as_raw()).PlatformIO) },
        "the generated ImGuiContext layout must match the linked native library"
    );
}

#[test]
fn suspend_rejects_an_open_frame_and_context_drop_recovers() {
    let _guard = crate::test_support::imgui_context_guard();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ctx = Context::create();
        assert!(ctx.font_atlas().build());
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.frame();
        let _ = ctx.suspend();
    }));
    assert!(result.is_err());

    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.frame().text("context recovered after rejected suspend");
    assert!(ctx.render().valid());
}

#[cfg(feature = "multi-viewport")]
#[test]
fn enable_multi_viewport_does_not_enable_docking() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut flags = ctx.io().config_flags();
    flags.remove(crate::ConfigFlags::VIEWPORTS_ENABLE | crate::ConfigFlags::DOCKING_ENABLE);
    ctx.io_mut().set_config_flags(flags);

    ctx.enable_multi_viewport();

    let flags = ctx.io().config_flags();
    assert!(flags.contains(crate::ConfigFlags::VIEWPORTS_ENABLE));
    assert!(!flags.contains(crate::ConfigFlags::DOCKING_ENABLE));
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_preserves_imgui_fallback_when_backends_decline_multi_viewport_support() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    ctx.enable_multi_viewport();

    ctx.frame().text("fallback to the main viewport");
    assert!(
        !ctx.io()
            .config_flags()
            .contains(crate::ConfigFlags::VIEWPORTS_ENABLE)
    );
    drop(ctx.render());
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_rejects_missing_required_platform_callbacks_before_entering_native_code() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    ctx.enable_multi_viewport();
    let mut backend_flags = ctx.io().backend_flags();
    backend_flags.insert(
        crate::BackendFlags::PLATFORM_HAS_VIEWPORTS | crate::BackendFlags::RENDERER_HAS_VIEWPORTS,
    );
    ctx.io_mut().set_backend_flags(backend_flags);

    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));

    assert!(rejected.is_err());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        super::FrameLifecycleState::Idle
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_rejects_transparent_docking_without_window_alpha_before_native_code() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    let platform_attachment = install_complete_test_viewport_backend(&mut ctx);

    let mut config_flags = ctx.io().config_flags();
    config_flags.insert(crate::ConfigFlags::VIEWPORTS_ENABLE | crate::ConfigFlags::DOCKING_ENABLE);
    ctx.io_mut().set_config_flags(config_flags);
    ctx.io_mut().set_config_docking_transparent_payload(true);

    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));

    assert!(rejected.is_err());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        super::FrameLifecycleState::Idle
    );
    drop(ctx);
    drop(platform_attachment);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_rejects_enabling_multi_viewport_between_the_first_and_second_frames() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));

    ctx.frame().text("first frame without viewports");
    drop(ctx.render());
    ctx.enable_multi_viewport();

    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));

    assert!(rejected.is_err());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        super::FrameLifecycleState::Rendered
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_prepares_a_completed_disabled_frame_for_late_multi_viewport_enablement() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    let platform_attachment = install_complete_test_viewport_backend(&mut ctx);

    let create_callback = unsafe { (*ctx.platform_io().as_raw()).Platform_CreateWindow };
    assert!(create_callback.is_some());

    for label in ["first disabled frame", "second disabled frame"] {
        ctx.frame().text(label);
        drop(ctx.render());
        assert_eq!(
            unsafe { (*ctx.platform_io().as_raw()).Platform_CreateWindow }
                .map(|callback| callback as usize),
            create_callback.map(|callback| callback as usize)
        );
    }

    ctx.enable_multi_viewport();
    ctx.frame().text("late-enabled viewport frame");
    drop(ctx.render());
    ctx.update_platform_windows();
    drop(ctx);
    drop(platform_attachment);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_window_calls_enforce_the_native_frame_order_in_rust() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    TEST_RENDER_WINDOW_CALLS.store(0, Ordering::SeqCst);
    unsafe {
        ctx.platform_io_mut()
            .set_renderer_render_window_raw(Some(test_renderer_render_window));
    }

    ctx.frame().text("platform lifecycle");
    let update_before_render = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.update_platform_windows();
    }));
    assert!(update_before_render.is_err());

    ctx.destroy_platform_windows().unwrap();
    assert!(!ctx.end_frame());

    ctx.frame()
        .text("platform lifecycle after normalized teardown");
    drop(ctx.render());
    let render_before_update = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.render_platform_windows_default();
    }));
    assert!(render_before_update.is_err());

    ctx.update_platform_windows();
    ctx.render_platform_windows_default();
    let duplicate_update = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.update_platform_windows();
    }));
    assert!(duplicate_update.is_err());

    ctx.frame().text("ended without rendering");
    assert!(ctx.end_frame());
    ctx.update_platform_windows();
    let render_after_end_only = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.render_platform_windows_default();
    }));
    assert!(render_after_end_only.is_err());
    assert_eq!(TEST_RENDER_WINDOW_CALLS.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn explicit_platform_window_teardown_notifies_the_active_platform_attachment() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let reject = Rc::new(Cell::new(false));
    let bound = Rc::new(Cell::new(false));
    let _lease = context
        .register_attachment::<ExplicitPlatformWindowTeardownMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(ExplicitPlatformWindowTeardownAttachment {
                log: Rc::clone(&log),
                reject,
                reject_after_native_teardown: Rc::new(Cell::new(false)),
                bound: Rc::clone(&bound),
            }),
        )
        .expect("test platform attachment must register");

    context.destroy_platform_windows().unwrap();

    assert!(bound.get());
    assert_eq!(*log.borrow(), ["begin", "end"]);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn rejected_platform_window_teardown_does_not_leave_the_scope_active() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let reject = Rc::new(Cell::new(true));
    let _lease = context
        .register_attachment::<ExplicitPlatformWindowTeardownMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(ExplicitPlatformWindowTeardownAttachment {
                log: Rc::clone(&log),
                reject: Rc::clone(&reject),
                reject_after_native_teardown: Rc::new(Cell::new(false)),
                bound: Rc::new(Cell::new(false)),
            }),
        )
        .expect("test platform attachment must register");

    let error = context
        .destroy_platform_windows()
        .expect_err("attachment rejection must prevent native teardown");
    assert!(matches!(
        error,
        super::ContextPlatformWindowTeardownError::AttachmentPreflight(_)
    ));
    reject.set(false);
    context
        .destroy_platform_windows()
        .expect("the rejected scope must not remain active");

    assert_eq!(*log.borrow(), ["begin", "begin", "end"]);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn postflight_platform_window_teardown_error_does_not_leave_the_scope_active() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let reject_after_native_teardown = Rc::new(Cell::new(true));
    let _lease = context
        .register_attachment::<ExplicitPlatformWindowTeardownMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(ExplicitPlatformWindowTeardownAttachment {
                log: Rc::clone(&log),
                reject: Rc::new(Cell::new(false)),
                reject_after_native_teardown: Rc::clone(&reject_after_native_teardown),
                bound: Rc::new(Cell::new(false)),
            }),
        )
        .expect("test platform attachment must register");

    let error = context
        .destroy_platform_windows()
        .expect_err("postflight rejection must reach the caller");
    assert!(matches!(
        error,
        super::ContextPlatformWindowTeardownError::AttachmentPostflight(_)
    ));
    reject_after_native_teardown.set(false);
    context
        .destroy_platform_windows()
        .expect("a postflight rejection must not leave the scope active");

    assert_eq!(*log.borrow(), ["begin", "end", "begin", "end"]);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn default_platform_render_requires_a_callback_render_path() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    ctx.frame().text("no default renderer callback");
    drop(ctx.render());
    ctx.update_platform_windows();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.render_platform_windows_default();
    }));
    assert!(result.is_err());
}

#[test]
fn end_frame_is_idempotent_and_allows_a_new_frame() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));

    ctx.frame().text("first frame");
    assert!(ctx.end_frame());
    assert!(!ctx.end_frame());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        super::FrameLifecycleState::Idle
    );

    ctx.frame().text("replacement frame");
    drop(ctx.render());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        super::FrameLifecycleState::Rendered
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn set_monitors_replaces_and_clears_imgui_owned_storage() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut first = crate::sys::ImGuiPlatformMonitor::default();
    first.MainPos = crate::sys::ImVec2 { x: 10.0, y: 20.0 };
    first.MainSize = crate::sys::ImVec2 { x: 100.0, y: 80.0 };
    first.WorkPos = first.MainPos;
    first.WorkSize = first.MainSize;
    first.DpiScale = 1.5;
    let mut second = crate::sys::ImGuiPlatformMonitor::default();
    second.MainPos = crate::sys::ImVec2 { x: 30.0, y: 40.0 };
    second.MainSize = crate::sys::ImVec2 { x: 120.0, y: 90.0 };
    second.WorkPos = second.MainPos;
    second.WorkSize = second.MainSize;
    second.DpiScale = 2.0;

    unsafe { ctx.platform_io_mut().set_monitors(&[first, second]) };

    let raw = unsafe { &(*ctx.platform_io().as_raw()).Monitors };
    assert_eq!(raw.Size, 2);
    assert_eq!(raw.Capacity, 2);
    assert!(!raw.Data.is_null());
    let stored = unsafe { std::slice::from_raw_parts(raw.Data, raw.Size as usize) };
    assert_eq!(stored[0].MainPos.x, 10.0);
    assert_eq!(stored[1].MainPos.y, 40.0);
    assert_eq!(stored[1].DpiScale, 2.0);

    unsafe { ctx.platform_io_mut().set_monitors(&[]) };

    let raw = unsafe { &(*ctx.platform_io().as_raw()).Monitors };
    assert_eq!(raw.Size, 0);
    assert_eq!(raw.Capacity, 0);
    assert!(raw.Data.is_null());
}

#[cfg(feature = "multi-viewport")]
#[test]
fn set_monitors_rejects_invalid_geometry_without_discarding_the_previous_list() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut valid = crate::sys::ImGuiPlatformMonitor::default();
    valid.MainSize = crate::sys::ImVec2 {
        x: 1920.0,
        y: 1080.0,
    };
    valid.WorkSize = valid.MainSize;
    valid.DpiScale = 1.0;
    unsafe { ctx.platform_io_mut().set_monitors(&[valid]) };
    let previous_data = unsafe { (*ctx.platform_io().as_raw()).Monitors.Data };

    let mut invalid = valid;
    invalid.WorkPos.x = -1.0;
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        ctx.platform_io_mut().set_monitors(&[invalid]);
    }));

    assert!(rejected.is_err());
    let monitors = unsafe { &(*ctx.platform_io().as_raw()).Monitors };
    assert_eq!(monitors.Size, 1);
    assert_eq!(monitors.Data, previous_data);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn set_monitors_rejects_overflowing_geometry_bounds() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut monitor = crate::sys::ImGuiPlatformMonitor::default();
    monitor.MainPos = crate::sys::ImVec2 {
        x: f32::MAX,
        y: 0.0,
    };
    monitor.MainSize = crate::sys::ImVec2 {
        x: f32::MAX,
        y: 1.0,
    };
    monitor.WorkPos = monitor.MainPos;
    monitor.WorkSize = crate::sys::ImVec2 { x: 0.0, y: 1.0 };
    monitor.DpiScale = 1.0;

    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        ctx.platform_io_mut().set_monitors(&[monitor]);
    }));

    assert!(rejected.is_err());
    let monitors = unsafe { &(*ctx.platform_io().as_raw()).Monitors };
    assert_eq!(monitors.Size, 0);
    assert!(monitors.Data.is_null());
}

#[test]
fn with_bound_context_restores_previous_context_after_panic() {
    let _guard = crate::test_support::imgui_context_guard();
    let ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_a = ctx_a.suspend();
    let ctx_b = Context::create();
    let raw_b = ctx_b.raw;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_bound_context(raw_a, || panic!("forced panic while context is rebound"));
    }));

    assert!(result.is_err());
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
fn suspended_context_identity_and_activation_without_a_foreign_context_are_stable() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended = super::SuspendedContext::create();
    let expected_id = suspended.id();
    let expected_raw = suspended.0.as_raw();

    let observed_id = suspended
        .try_with_active(|context| {
            assert_eq!(context.id(), expected_id);
            assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, expected_raw);
            Ok::<_, ()>(context.id())
        })
        .expect("scoped Context activation must succeed");

    assert_eq!(observed_id, expected_id);
    assert!(unsafe { crate::sys::igGetCurrentContext() }.is_null());
}

#[test]
fn suspended_context_activation_rejects_a_foreign_context_before_the_closure() {
    let _guard = crate::test_support::imgui_context_guard();
    let active = Context::create();
    let active_raw = active.as_raw();
    let active_id = active.id();
    let mut suspended = super::SuspendedContext::create();
    let suspended_raw = suspended.0.as_raw();
    let suspended_id = suspended.id();
    let closure_called = std::cell::Cell::new(false);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = suspended.try_with_active::<(), ()>(|_| {
            closure_called.set(true);
            Ok(())
        });
    }));
    assert!(panic.is_err());
    assert!(!closure_called.get());
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, active_raw);
    assert_eq!(active.id(), active_id);
    assert_eq!(suspended.id(), suspended_id);
    assert_eq!(suspended.0.as_raw(), suspended_raw);

    drop(active);
    suspended
        .try_with_active(|context| {
            assert_eq!(context.as_raw(), suspended_raw);
            Ok::<_, ()>(())
        })
        .expect("activation must succeed after the current Context is dropped");
    drop(suspended);
}

#[test]
fn suspended_context_error_closes_an_open_frame_and_can_reenter() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended = super::SuspendedContext::create();

    let error = suspended.try_with_active(|context| {
        assert!(context.font_atlas().build());
        context.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
        context.frame().text("frame left open by an error");
        Err::<(), _>("stop")
    });
    assert_eq!(error, Err("stop"));

    suspended
        .try_with_active(|context| {
            assert_ne!(
                context.frame_lifecycle_state(),
                super::FrameLifecycleState::InFrame
            );
            context.frame().text("context remains reusable");
            assert!(context.end_frame());
            Ok::<_, ()>(())
        })
        .expect("an error-cleaned Context must be reusable");
}

#[test]
fn suspended_context_success_rejects_and_closes_an_open_frame() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended = super::SuspendedContext::create();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = suspended.try_with_active::<(), ()>(|context| {
            assert!(context.font_atlas().build());
            context.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
            context
                .frame()
                .text("successful closure left this frame open");
            Ok(())
        });
    }));

    assert!(panic.is_err());
    assert!(unsafe { crate::sys::igGetCurrentContext() }.is_null());
    suspended
        .try_with_active(|context| {
            assert_ne!(
                context.frame_lifecycle_state(),
                super::FrameLifecycleState::InFrame
            );
            Ok::<_, ()>(())
        })
        .expect("a contract-violation cleanup must leave the Context reusable");

    drop(suspended);
}

#[test]
fn suspended_context_panic_closes_an_open_frame_and_preserves_the_payload() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended = super::SuspendedContext::create();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = suspended.try_with_active::<(), ()>(|context| {
            assert!(context.font_atlas().build());
            context.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
            context.frame().text("panicking frame");
            std::panic::panic_any(0xC0FFEE_u32);
        });
    }))
    .expect_err("closure panic must propagate");

    assert_eq!(panic.downcast_ref::<u32>(), Some(&0xC0FFEE));
    suspended
        .try_with_active(|context| {
            assert_ne!(
                context.frame_lifecycle_state(),
                super::FrameLifecycleState::InFrame
            );
            context.frame().text("context recovered after panic");
            assert!(context.end_frame());
            Ok::<_, ()>(())
        })
        .expect("a panic-cleaned Context must be reusable");
}

#[test]
fn nested_suspended_context_activation_is_rejected_before_the_inner_closure() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended_a = super::SuspendedContext::create();
    let raw_a = suspended_a.0.as_raw();
    let mut suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.as_raw();
    let inner_called = std::cell::Cell::new(false);

    suspended_a
        .try_with_active(|context_a| {
            assert_eq!(context_a.as_raw(), raw_a);
            assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_a);

            let _panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = suspended_b.try_with_active::<(), ()>(|context_b| {
                    inner_called.set(true);
                    assert_eq!(context_b.as_raw(), raw_b);
                    Ok(())
                });
            }))
            .expect_err("nested activation must be rejected");
            assert!(!inner_called.get());
            assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_a);
            Ok::<_, ()>(())
        })
        .expect("outer Context activation must remain valid");

    assert!(unsafe { crate::sys::igGetCurrentContext() }.is_null());
    suspended_b
        .try_with_active(|context_b| {
            assert_eq!(context_b.as_raw(), raw_b);
            Ok::<_, ()>(())
        })
        .expect("the rejected nested scope must leave its Context reusable");
    drop(suspended_b);
    drop(suspended_a);
}

#[test]
fn suspended_context_rejects_a_potential_owner_swap_before_the_closure() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut active = Context::create();
    let original_active_id = active.id();
    let original_active_raw = active.as_raw();
    let mut suspended = super::SuspendedContext::create();
    let original_suspended_id = suspended.id();
    let original_suspended_raw = suspended.0.as_raw();
    let swap_attempted = std::cell::Cell::new(false);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = suspended.try_with_active::<(), ()>(|context| {
            swap_attempted.set(true);
            std::mem::swap(context, &mut active);
            Ok(())
        });
    }));
    assert!(panic.is_err());
    assert!(!swap_attempted.get());

    assert_eq!(active.id(), original_active_id);
    assert_eq!(active.as_raw(), original_active_raw);
    assert_eq!(suspended.id(), original_suspended_id);
    assert_eq!(suspended.0.as_raw(), original_suspended_raw);
    assert_eq!(
        unsafe { crate::sys::igGetCurrentContext() },
        original_active_raw
    );

    drop(active);
    suspended
        .try_with_active(|context| {
            assert_eq!(context.id(), original_suspended_id);
            Ok::<_, ()>(())
        })
        .expect("the rejected swap must leave the suspended Context reusable");
}

#[test]
fn suspended_context_can_be_entered_repeatedly() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended = super::SuspendedContext::create();
    let expected_id = suspended.id();

    for entry in 0..4 {
        let observed = suspended
            .try_with_active(|context| Ok::<_, ()>((context.id(), entry)))
            .expect("repeated scoped activation must succeed");
        assert_eq!(observed, (expected_id, entry));
        assert!(unsafe { crate::sys::igGetCurrentContext() }.is_null());
    }
}

#[test]
fn context_binding_rejects_destroyed_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let ctx = Context::create();
    let binding = ctx.binding();
    let alive = ctx.alive_token();

    assert!(binding.is_alive());
    assert!(alive.is_alive());
    drop(ctx);

    assert!(!binding.is_alive());
    assert!(!alive.is_alive());
    assert!(matches!(
        binding.try_with_bound_context(|| ()),
        Err(ContextBindingError::NativeDestroyed)
    ));
}

#[test]
fn context_binding_identity_and_nested_restoration_are_stable() {
    let _guard = crate::test_support::imgui_context_guard();
    let suspended_a = super::SuspendedContext::create();
    let suspended_b = super::SuspendedContext::create();
    let binding_a = suspended_a.0.binding();
    let binding_b = suspended_b.0.binding();
    let raw_a = suspended_a.0.as_raw();
    let raw_b = suspended_b.0.as_raw();
    let active = Context::create();
    let active_raw = active.as_raw();

    assert_ne!(binding_a.id(), binding_b.id());
    assert_ne!(binding_a.id(), active.id());
    binding_a.with_bound_context(|| {
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_a);
        binding_b.with_bound_context(|| {
            assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
        });
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_a);
    });
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, active_raw);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        binding_a.with_bound_context(|| {
            binding_b.with_bound_context(|| panic!("forced nested binding panic"));
        });
    }));
    assert!(panic.is_err());
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, active_raw);
    assert!(binding_b.try_with_bound_context(|| ()).is_ok());

    drop(active);
    drop(suspended_b);
    drop(suspended_a);
}

#[test]
fn binding_does_not_restore_a_previous_context_destroyed_inside_the_scope() {
    let _guard = crate::test_support::imgui_context_guard();
    let active = Context::create();
    let suspended = super::SuspendedContext::create();
    let binding = suspended.0.binding();

    let error = binding.with_bound_context(|| {
        drop(active);
        Context::try_create().expect_err("binding scopes must reject replacement Contexts")
    });
    assert!(matches!(
        error,
        crate::error::ImGuiError::ContextBindingScopeActive
    ));

    assert!(unsafe { crate::sys::igGetCurrentContext() }.is_null());
    let replacement = Context::create();
    drop(replacement);
    drop(suspended);
}

#[test]
fn live_contexts_confine_process_global_imgui_state_to_one_thread() {
    let _guard = crate::test_support::imgui_context_guard();
    let suspended = super::SuspendedContext::create();

    let error = std::thread::spawn(|| {
        Context::try_create().expect_err("another thread must not share process-global GImGui")
    })
    .join()
    .expect("thread-conflict probe must not panic");
    assert!(matches!(
        error,
        crate::error::ImGuiError::ContextThreadConflict
    ));

    drop(suspended);
    std::thread::spawn(|| {
        let context = Context::try_create()
            .expect("ownership may migrate after the last Context is destroyed");
        drop(context);
    })
    .join()
    .expect("post-release Context creation must not panic");
}

#[test]
fn attachment_registration_preflight_is_non_mutating() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();

    assert_eq!(
        ctx.preflight_attachment_registration::<RendererMarker>(ContextAttachmentRole::Renderer),
        Err(ContextAttachmentError::MissingPlatform)
    );
    assert_eq!(
        ctx.preflight_attachment_registration::<PlatformMarker>(ContextAttachmentRole::Platform),
        Ok(())
    );

    let platform = Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new()))));
    let mut platform_lease = ctx
        .register_attachment::<PlatformMarker>(ContextAttachmentRole::Platform, platform)
        .unwrap();

    assert_eq!(
        ctx.preflight_attachment_registration::<PlatformMarker>(ContextAttachmentRole::Platform),
        Err(ContextAttachmentError::DuplicateAttachment)
    );
    assert_eq!(
        ctx.preflight_attachment_registration::<ExtensionMarker>(ContextAttachmentRole::Platform),
        Err(ContextAttachmentError::RoleOccupied(
            ContextAttachmentRole::Platform
        ))
    );
    assert_eq!(
        ctx.preflight_attachment_registration::<RendererMarker>(ContextAttachmentRole::Renderer),
        Ok(())
    );

    assert_eq!(platform_lease.detach(), Ok(true));
    let shared_context = &ctx;
    assert_eq!(
        shared_context
            .preflight_attachment_registration::<PlatformMarker>(ContextAttachmentRole::Platform),
        Ok(())
    );
    let replacement = Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new()))));
    let _replacement_lease = ctx
        .register_attachment::<PlatformMarker>(ContextAttachmentRole::Platform, replacement)
        .unwrap();
}

#[test]
fn platform_release_is_generation_bound_and_rejects_active_renderer_dependencies() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut platform_lease = ctx
        .register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::clone(&log))),
        )
        .unwrap();
    let platform = platform_lease.handle();
    let mut renderer_lease = ctx
        .register_attachment::<RendererMarker>(
            ContextAttachmentRole::Renderer,
            Rc::new(RecordingAttachment::new(Rc::clone(&log))),
        )
        .unwrap();

    assert!(matches!(
        ctx.prepare_platform_attachment_release(&platform),
        Err(ContextPlatformAttachmentReleaseError::RendererActive)
    ));
    assert!(platform.is_attached());
    assert_eq!(
        platform_lease.detach(),
        Err(ContextAttachmentDetachError::RendererActive)
    );
    assert!(platform_lease.is_attached());

    assert_eq!(renderer_lease.detach(), Ok(true));
    {
        let mut permit = ctx
            .prepare_platform_attachment_release(&platform)
            .expect("renderer release must make platform shutdown retryable");
        assert!(platform.is_attached());
        assert_eq!(
            platform_lease.detach(),
            Err(ContextAttachmentDetachError::ReleaseInProgress)
        );
        assert!(matches!(
            permit.context_mut().register_attachment::<RendererMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(RecordingAttachment::new(Rc::clone(&log))),
            ),
            Err(ContextAttachmentError::MissingPlatform)
        ));
        drop(permit);
    }
    assert!(platform.is_attached());

    ctx.prepare_platform_attachment_release(&platform)
        .unwrap()
        .commit();
    assert!(!platform.is_attached());
    assert!(!platform_lease.is_attached());
    assert!(log.borrow().is_empty());
}

#[test]
fn context_teardown_reclaims_a_forgotten_platform_release_permit() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let platform_lease = ctx
        .register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::clone(&log))),
        )
        .unwrap();
    let handle = platform_lease.handle();

    let permit = ctx
        .prepare_platform_attachment_release(&handle)
        .expect("a platform without renderer dependencies must be releasable");
    std::mem::forget(permit);
    drop(ctx);

    assert_eq!(
        log.borrow().as_slice(),
        ["quiesce", "renderer", "platform", "post"]
    );
    assert!(!handle.is_attached());
    assert!(!platform_lease.is_attached());
}

#[test]
fn renderer_detach_commits_role_state_before_dropping_user_attachment() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut platform_lease = ctx
        .register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new())))),
        )
        .unwrap();
    let renderer = Rc::new(PanickingDropAttachment);
    let mut renderer_lease = ctx
        .register_attachment::<PanickingDropRendererMarker>(
            ContextAttachmentRole::Renderer,
            renderer.clone(),
        )
        .unwrap();
    drop(renderer);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = renderer_lease.detach();
    }));
    assert!(panic.is_err());
    assert!(!renderer_lease.is_attached());
    assert_eq!(platform_lease.detach(), Ok(true));
}

#[test]
fn platform_release_commits_before_dropping_user_attachment() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let platform = Rc::new(PanickingDropAttachment);
    let platform_lease = ctx
        .register_attachment::<PanickingDropPlatformMarker>(
            ContextAttachmentRole::Platform,
            platform.clone(),
        )
        .unwrap();
    let handle = platform_lease.handle();
    drop(platform);

    let permit = ctx
        .prepare_platform_attachment_release(&handle)
        .expect("platform release should be available without a renderer");
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| permit.commit()));
    assert!(panic.is_err());
    assert!(!handle.is_attached());
    assert!(matches!(
        ctx.register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new())))),
        ),
        Ok(_)
    ));
}

#[test]
fn platform_release_rejects_a_foreign_context_generation_without_mutation() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut owner = Context::create();
    let owner_lease = owner
        .register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new())))),
        )
        .unwrap();
    let owner_handle = owner_lease.handle();
    let suspended_owner = owner.suspend();

    let mut foreign = Context::create();
    let foreign_lease = foreign
        .register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new())))),
        )
        .unwrap();
    assert!(matches!(
        foreign.prepare_platform_attachment_release(&owner_handle),
        Err(ContextPlatformAttachmentReleaseError::PlatformGenerationMismatch)
    ));
    assert!(owner_handle.is_attached());
    assert!(foreign_lease.is_attached());

    drop(foreign);
    let mut owner = suspended_owner
        .activate()
        .expect("the owner Context should reactivate");
    owner
        .prepare_platform_attachment_release(&owner_handle)
        .unwrap()
        .commit();
    assert!(!owner_handle.is_attached());
    drop(owner_lease);
}

#[test]
fn attachments_use_phased_teardown_and_reject_ordinary_binding() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));

    let platform = Rc::new(RecordingAttachment::new(Rc::clone(&log)));
    let mut extension = RecordingAttachment::new(Rc::clone(&log));
    let ordinary_rejected = Rc::clone(&extension.ordinary_rejected);
    let teardown_bound = Rc::clone(&extension.teardown_bound);
    extension.binding = Some(ctx.binding());
    let extension = Rc::new(extension);
    let renderer = Rc::new(RecordingAttachment::new(Rc::clone(&log)));

    let _platform_lease = ctx
        .register_attachment::<PlatformMarker>(ContextAttachmentRole::Platform, platform)
        .unwrap();
    let _extension_lease = ctx
        .register_attachment::<ExtensionMarker>(ContextAttachmentRole::Extension, extension)
        .unwrap();
    let _renderer_lease = ctx
        .register_attachment::<RendererMarker>(ContextAttachmentRole::Renderer, renderer)
        .unwrap();

    drop(ctx);

    assert!(ordinary_rejected.get());
    assert!(teardown_bound.get());
    assert_eq!(
        log.borrow().as_slice(),
        [
            "quiesce", "quiesce", "quiesce", "renderer", "renderer", "renderer", "platform",
            "platform", "platform", "post", "post", "post",
        ]
    );
}

#[test]
fn context_drop_ends_an_open_frame_before_attachment_quiesce() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    assert!(ctx.font_atlas().build());
    ctx.prepare_frame(super::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    let attachment = Rc::new(RecordingAttachment::new(Rc::new(RefCell::new(Vec::new()))));
    let frame_closed = Rc::clone(&attachment.frame_closed_before_quiesce);
    let _lease = ctx
        .register_attachment::<ExtensionMarker>(ContextAttachmentRole::Extension, attachment)
        .unwrap();

    ctx.frame().text("drop an open frame");
    drop(ctx);

    assert!(frame_closed.get());
}

#[test]
fn attachment_quiesce_panic_finishes_its_phase_but_blocks_later_phases() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));

    let mut panicking = RecordingAttachment::new(Rc::clone(&log));
    panicking.inspect_native_state_during_quiesce = false;
    panicking.panic_during_quiesce = true;
    let mut normal = RecordingAttachment::new(Rc::clone(&log));
    normal.inspect_native_state_during_quiesce = false;
    let normal = Rc::new(normal);
    let binding = ctx.binding();

    let _panicking_lease = ctx
        .register_attachment::<PanickingExtensionMarker>(
            ContextAttachmentRole::Extension,
            Rc::new(panicking),
        )
        .unwrap();
    let _normal_lease = ctx
        .register_attachment::<ExtensionMarker>(ContextAttachmentRole::Extension, normal)
        .unwrap();

    let controls = ctx.attachments.begin_teardown();
    assert!(!super::attachment::run_pre_destroy_phase(
        &controls,
        &mut ctx,
        super::ContextAttachmentPhase::Quiesce,
    ));

    let log = log.borrow();
    assert_eq!(log.iter().filter(|entry| **entry == "quiesce").count(), 2);
    assert!(
        !log.iter()
            .any(|entry| matches!(*entry, "renderer" | "platform" | "post"))
    );
    drop(log);
    drop(controls);
    drop(ctx);
    assert_eq!(
        binding.lifecycle(),
        super::ContextLifecycle::NativeDestroyed
    );
}

#[test]
fn attachment_renderer_error_finishes_its_phase_but_blocks_platform_release() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));

    let mut panicking = RecordingAttachment::new(Rc::clone(&log));
    panicking.inspect_native_state_during_quiesce = false;
    panicking.fail_during_renderer_release = true;
    let mut normal = RecordingAttachment::new(Rc::clone(&log));
    normal.inspect_native_state_during_quiesce = false;
    let normal = Rc::new(normal);
    let _panicking_lease = ctx
        .register_attachment::<PanickingRendererExtensionMarker>(
            ContextAttachmentRole::Extension,
            Rc::new(panicking),
        )
        .unwrap();
    let _normal_lease = ctx
        .register_attachment::<ExtensionMarker>(ContextAttachmentRole::Extension, normal)
        .unwrap();

    let controls = ctx.attachments.begin_teardown();
    assert!(super::attachment::run_pre_destroy_phase(
        &controls,
        &mut ctx,
        super::ContextAttachmentPhase::Quiesce,
    ));
    assert!(!super::attachment::run_pre_destroy_phase(
        &controls,
        &mut ctx,
        super::ContextAttachmentPhase::RendererResources,
    ));
    let log = log.borrow();
    assert_eq!(log.iter().filter(|entry| **entry == "quiesce").count(), 2);
    assert_eq!(log.iter().filter(|entry| **entry == "renderer").count(), 2);
    assert!(
        !log.iter()
            .any(|entry| matches!(*entry, "platform" | "post"))
    );
    drop(log);
    drop(controls);
    drop(ctx);
}

#[test]
fn attachment_registration_enforces_roles_and_detach_is_idempotent() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));

    let missing_platform = ctx.register_attachment::<RendererMarker>(
        ContextAttachmentRole::Renderer,
        Rc::new(RecordingAttachment::new(Rc::clone(&log))),
    );
    assert!(matches!(
        missing_platform,
        Err(ContextAttachmentError::MissingPlatform)
    ));

    let mut platform_lease = ctx
        .register_attachment::<PlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(RecordingAttachment::new(Rc::clone(&log))),
        )
        .unwrap();
    let duplicate = ctx.register_attachment::<PlatformMarker>(
        ContextAttachmentRole::Extension,
        Rc::new(RecordingAttachment::new(Rc::clone(&log))),
    );
    assert!(matches!(
        duplicate,
        Err(ContextAttachmentError::DuplicateAttachment)
    ));
    let occupied = ctx.register_attachment::<ExtensionMarker>(
        ContextAttachmentRole::Platform,
        Rc::new(RecordingAttachment::new(Rc::clone(&log))),
    );
    assert!(matches!(
        occupied,
        Err(ContextAttachmentError::RoleOccupied(
            ContextAttachmentRole::Platform
        ))
    ));

    assert_eq!(platform_lease.detach(), Ok(true));
    assert_eq!(platform_lease.detach(), Ok(false));
    drop(ctx);
    assert!(log.borrow().is_empty());
}

#[test]
fn detaching_attachment_releases_context_ownership_immediately() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let attachment = Rc::new(RecordingAttachment::new(log));
    let mut lease = ctx
        .register_attachment::<ExtensionMarker>(
            ContextAttachmentRole::Extension,
            attachment.clone(),
        )
        .unwrap();

    assert_eq!(Rc::strong_count(&attachment), 2);
    assert_eq!(lease.detach(), Ok(true));
    assert_eq!(Rc::strong_count(&attachment), 1);
    drop(ctx);
}

#[test]
fn attachment_lease_can_defer_cleanup_to_context_teardown() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let attachment = Rc::new(RecordingAttachment::new(Rc::clone(&log)));
    let lease = ctx
        .register_attachment::<PlatformMarker>(ContextAttachmentRole::Platform, attachment.clone())
        .unwrap();

    lease.defer_to_context();
    assert_eq!(Rc::strong_count(&attachment), 2);
    drop(ctx);

    assert_eq!(Rc::strong_count(&attachment), 1);
    assert_eq!(
        log.borrow().as_slice(),
        ["quiesce", "renderer", "platform", "post"]
    );
}

#[test]
fn renderer_attachment_reset_releases_before_commit_and_rejects_reentry() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let (consumer, binding) = prepare_managed_font_atlas(&mut context);
    let observation = Rc::new(RendererTextureResetObservation::new());
    let attachment = Rc::new(RendererTextureResetAttachment {
        consumer,
        expected_binding: binding,
        release_fails: false,
        attempts_reentry: true,
        observation: Rc::clone(&observation),
    });
    let _lease = context
        .register_attachment::<RendererTextureResetMarker>(
            ContextAttachmentRole::Extension,
            attachment,
        )
        .unwrap();

    drop(context);

    assert_eq!(observation.release_calls.get(), 1);
    assert!(observation.release_saw_expected_binding.get());
    assert!(observation.nested_reset_rejected.get());
    assert!(observation.invalidated.get().is_some_and(|count| count > 0));
    assert_eq!(
        observation.binding_after_call.get(),
        crate::TextureId::null(),
        "native binding must reset only after resource release succeeds"
    );
}

#[test]
fn renderer_attachment_reset_preserves_native_bindings_when_release_fails() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let (consumer, binding) = prepare_managed_font_atlas(&mut context);
    let observation = Rc::new(RendererTextureResetObservation::new());
    let attachment = Rc::new(RendererTextureResetAttachment {
        consumer,
        expected_binding: binding,
        release_fails: true,
        attempts_reentry: false,
        observation: Rc::clone(&observation),
    });
    let _lease = context
        .register_attachment::<RendererTextureResetMarker>(
            ContextAttachmentRole::Extension,
            attachment,
        )
        .unwrap();

    drop(context);

    assert_eq!(observation.release_calls.get(), 1);
    assert!(observation.release_saw_expected_binding.get());
    assert!(observation.reset_rejected.get());
    assert_eq!(observation.invalidated.get(), None);
    assert_eq!(
        observation.binding_after_call.get(),
        binding,
        "a release error must leave native bindings intact for fail-stop teardown"
    );
}

#[test]
fn renderer_attachment_reset_rejects_wrong_phase_without_calling_release() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let (consumer, binding) = prepare_managed_font_atlas(&mut context);
    let observation = Rc::new(RendererTextureResetObservation::new());
    let attachment = Rc::new(WrongPhaseRendererTextureResetAttachment {
        consumer,
        expected_binding: binding,
        observation: Rc::clone(&observation),
    });
    let _lease = context
        .register_attachment::<WrongPhaseRendererTextureResetMarker>(
            ContextAttachmentRole::Extension,
            attachment,
        )
        .unwrap();

    drop(context);

    assert!(observation.reset_rejected.get());
    assert_eq!(observation.release_calls.get(), 0);
    assert_eq!(observation.binding_after_call.get(), binding);
}

#[test]
fn renderer_attachment_reset_rejects_a_foreign_consumer_without_mutating_bindings() {
    let _guard = crate::test_support::imgui_context_guard();

    let mut foreign_context = Context::create();
    let (foreign_consumer, _) = prepare_managed_font_atlas(&mut foreign_context);
    let foreign_context = foreign_context.suspend();

    let mut context = Context::create();
    let (local_consumer, binding) = prepare_managed_font_atlas(&mut context);
    let observation = Rc::new(RendererTextureResetObservation::new());
    let attachment = Rc::new(RendererTextureResetAttachment {
        consumer: foreign_consumer,
        expected_binding: binding,
        release_fails: false,
        attempts_reentry: false,
        observation: Rc::clone(&observation),
    });
    let _lease = context
        .register_attachment::<RendererTextureResetMarker>(
            ContextAttachmentRole::Extension,
            attachment,
        )
        .unwrap();

    drop(context);

    assert!(observation.reset_rejected.get());
    assert_eq!(observation.release_calls.get(), 0);
    assert_eq!(observation.binding_after_call.get(), binding);

    drop(local_consumer);
    drop(foreign_context);
}

#[test]
fn renderer_attachment_reset_rejects_an_active_snapshot_without_mutating_bindings() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let (consumer, binding) = prepare_managed_font_atlas_for_detached_rendering(&mut context);
    let snapshot = context
        .begin_frame()
        .render_snapshot(&consumer)
        .expect("test Context must produce an outstanding detached snapshot");
    let observation = Rc::new(RendererTextureResetObservation::new());
    let attachment = Rc::new(RendererTextureResetAttachment {
        consumer,
        expected_binding: binding,
        release_fails: false,
        attempts_reentry: false,
        observation: Rc::clone(&observation),
    });
    let _lease = context
        .register_attachment::<RendererTextureResetMarker>(
            ContextAttachmentRole::Extension,
            attachment,
        )
        .unwrap();

    drop(context);

    assert!(observation.reset_rejected.get());
    assert_eq!(observation.release_calls.get(), 0);
    assert_eq!(observation.binding_after_call.get(), binding);
    drop(snapshot);
}

#[test]
fn renderer_attachment_reset_restores_a_foreign_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = Context::create();
    let (consumer, binding) = prepare_managed_font_atlas(&mut context);
    let observation = Rc::new(RendererTextureResetObservation::new());
    let attachment = Rc::new(RendererTextureResetAttachment {
        consumer,
        expected_binding: binding,
        release_fails: false,
        attempts_reentry: false,
        observation: Rc::clone(&observation),
    });
    let _lease = context
        .register_attachment::<RendererTextureResetMarker>(
            ContextAttachmentRole::Extension,
            attachment,
        )
        .unwrap();
    let context = context.suspend();

    let foreign = Context::create();
    let foreign_raw = foreign.as_raw();
    drop(context);

    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, foreign_raw);
    assert!(observation.invalidated.get().is_some_and(|count| count > 0));
    assert_eq!(
        observation.binding_after_call.get(),
        crate::TextureId::null()
    );
    drop(foreign);
}

#[test]
fn dropping_suspended_context_restores_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut suspended = super::SuspendedContext::create();
    let log = Rc::new(RefCell::new(Vec::new()));
    let _lease = suspended
        .0
        .register_attachment::<ExtensionMarker>(
            ContextAttachmentRole::Extension,
            Rc::new(RecordingAttachment::new(log)),
        )
        .unwrap();
    let ctx = Context::create();
    let raw = ctx.as_raw();

    drop(suspended);

    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw);
    drop(ctx);
}

#[test]
fn io_and_platform_io_accessors_use_self_context_not_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let marker_a = std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    unsafe {
        // The test only round-trips this opaque marker and never dereferences it.
        ctx_a.io_mut().set_backend_language_user_data(marker_a);
    }
    let pio_a = ctx_a.platform_io().as_raw();
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    let marker_b = std::ptr::NonNull::<u16>::dangling().as_ptr().cast();
    unsafe {
        // The test only round-trips this opaque marker and never dereferences it.
        ctx_b.io_mut().set_backend_language_user_data(marker_b);
    }
    let pio_b = ctx_b.platform_io().as_raw();

    assert_ne!(marker_a, marker_b);
    assert_ne!(pio_a, pio_b);

    let ctx_a = suspended_a.activate().expect_err("ctx_b is still active");
    assert_eq!(ctx_a.0.io().backend_language_user_data(), marker_a);
    assert_eq!(ctx_a.0.platform_io().as_raw(), pio_a);
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(ctx_a);
}

#[test]
fn style_and_main_viewport_accessors_use_self_context_not_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    ctx_a.style_mut().set_alpha(0.25);
    let viewport_a = ctx_a.main_viewport().as_raw();
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    ctx_b.style_mut().set_alpha(0.75);
    let viewport_b = ctx_b.main_viewport().as_raw();

    assert_ne!(viewport_a, viewport_b);

    let mut ctx_a = suspended_a.activate().expect_err("ctx_b is still active");
    assert_eq!(ctx_a.0.style().alpha(), 0.25);
    assert_eq!(ctx_a.0.main_viewport().as_raw(), viewport_a);
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(ctx_a);
}

#[test]
fn io_font_global_scale_uses_owner_context_not_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    ctx_a.style_mut().set_font_scale_main(1.25);
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    ctx_b.style_mut().set_font_scale_main(2.0);

    let mut ctx_a = suspended_a.activate().expect_err("ctx_b is still active");
    assert_eq!(ctx_a.0.io().font_global_scale(), 1.25);

    ctx_a.0.io_mut().set_font_global_scale(1.5);

    assert_eq!(ctx_a.0.style().font_scale_main(), 1.5);
    assert_eq!(ctx_b.style().font_scale_main(), 2.0);
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(ctx_a);
}

#[test]
fn frame_lifecycle_requires_receiver_to_be_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let ctx_a = Context::create();
    let suspended_a = ctx_a.suspend();
    let ctx_b = Context::create();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = suspended_a.0.frame_lifecycle_state();
    }));

    assert!(result.is_err());
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
fn ui_stack_tokens_drop_on_owner_context_and_restore_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.raw;

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.font_atlas().build();
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);

    {
        let ui_a = ctx_a.frame();
        let style_alpha =
            |raw| with_bound_context(raw, || unsafe { (*crate::sys::igGetStyle()).Alpha });
        let original_alpha_a = style_alpha(raw_a);
        let original_alpha_b = style_alpha(raw_b);
        let token = ui_a.push_style_var(crate::StyleVar::Alpha(0.25));
        assert_eq!(style_alpha(raw_a), 0.25);

        unsafe { crate::sys::igSetCurrentContext(raw_b) };
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
        assert_eq!(style_alpha(raw_b), original_alpha_b);

        drop(token);

        assert_eq!(style_alpha(raw_a), original_alpha_a);
        assert_eq!(style_alpha(raw_b), original_alpha_b);
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
    }

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.render();

    drop(ctx_a);
    drop(suspended_b);
}

#[test]
fn ui_methods_run_on_owner_context_and_restore_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.raw;

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.font_atlas().build();
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);

    {
        let ui_a = ctx_a.frame();

        let color_a = [0.1, 0.2, 0.3, 1.0];
        let color_b = [0.8, 0.7, 0.6, 1.0];
        unsafe {
            with_bound_context(raw_a, || {
                (&mut *(crate::sys::igGetStyle() as *mut crate::Style))
                    .set_color(crate::StyleColor::Text, color_a)
            });
            with_bound_context(raw_b, || {
                (&mut *(crate::sys::igGetStyle() as *mut crate::Style))
                    .set_color(crate::StyleColor::Text, color_b)
            });
        }

        unsafe { crate::sys::igSetCurrentContext(raw_b) };
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

        assert_eq!(ui_a.style_color(crate::StyleColor::Text), color_a);
        assert_eq!(ui_a.clone_style().color(crate::StyleColor::Text), color_a);
        ui_a.style_colors_dark();
        ui_a.set_mouse_draw_cursor(true);
        ui_a.set_mouse_cursor(Some(crate::MouseCursor::Hand));

        let owner_color = unsafe {
            with_bound_context(raw_a, || {
                (&*(crate::sys::igGetStyle() as *const crate::Style)).color(crate::StyleColor::Text)
            })
        };
        let current_color = unsafe {
            with_bound_context(raw_b, || {
                (&*(crate::sys::igGetStyle() as *const crate::Style)).color(crate::StyleColor::Text)
            })
        };
        let owner_mouse = with_bound_context(raw_a, || unsafe {
            (
                (*crate::sys::igGetIO_Nil()).MouseDrawCursor,
                crate::sys::igGetMouseCursor(),
            )
        });
        let current_mouse = with_bound_context(raw_b, || unsafe {
            (
                (*crate::sys::igGetIO_Nil()).MouseDrawCursor,
                crate::sys::igGetMouseCursor(),
            )
        });

        assert_ne!(owner_color, color_a);
        assert_eq!(current_color, color_b);
        assert_eq!(owner_mouse, (true, crate::MouseCursor::Hand as i32));
        assert_ne!(current_mouse, owner_mouse);
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
    }

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.render();

    drop(ctx_a);
    drop(suspended_b);
}

#[test]
fn font_stack_token_drops_on_owner_context_and_restores_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.raw;

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let font = ctx_a.font_atlas().add_font_default(None);
    let _ = ctx_a.font_atlas().build();
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);

    let token = ctx_a.frame().push_font(font);

    unsafe { crate::sys::igSetCurrentContext(raw_b) };
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

    drop(token);

    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.render();

    drop(ctx_a);
    drop(suspended_b);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_viewport_snapshot_requires_rendered_frame_and_reuses_current_draw_data() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    ctx.prepare_frame(
        super::FramePrepareOptions::new([320.0, 240.0], 1.0 / 60.0).renderer_has_textures(),
    );
    let consumer = ctx.create_renderer_consumer().unwrap();

    let before_render = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.platform_viewport_snapshot(&consumer);
    }));
    assert!(before_render.is_err());

    let frame = ctx.begin_frame();
    frame.ui().text("snapshot after render");
    let main_snapshot = frame.render_snapshot(&consumer).unwrap();

    let snapshot = ctx
        .platform_viewport_snapshot(&consumer)
        .expect("rendered platform viewport draw data should snapshot");

    assert_eq!(snapshot.draw_data().display_size, [320.0, 240.0]);
    assert!(
        snapshot
            .viewports()
            .iter()
            .any(|viewport| viewport.draw.display_size == [320.0, 240.0]),
        "platform viewport snapshot should include the rendered main viewport"
    );
    drop(main_snapshot);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_io_get_window_pos_and_size_setters_install_handlers() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn get_pos(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_pos: *mut crate::sys::ImVec2,
    ) {
        if let Some(out_pos) = unsafe { out_pos.as_mut() } {
            *out_pos = crate::sys::ImVec2 { x: 10.0, y: 20.0 };
        }
    }
    unsafe extern "C" fn get_size(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_size: *mut crate::sys::ImVec2,
    ) {
        if let Some(out_size) = unsafe { out_size.as_mut() } {
            *out_size = crate::sys::ImVec2 { x: 30.0, y: 40.0 };
        }
    }
    unsafe extern "C" fn get_scale(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_scale: *mut crate::sys::ImVec2,
    ) {
        if let Some(out_scale) = unsafe { out_scale.as_mut() } {
            *out_scale = crate::sys::ImVec2 { x: 1.0, y: 2.0 };
        }
    }
    unsafe extern "C" fn get_insets(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_insets: *mut crate::sys::ImVec4,
    ) {
        if let Some(out_insets) = unsafe { out_insets.as_mut() } {
            *out_insets = crate::sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
        }
    }

    let mut ctx = Context::create();

    {
        let pio = ctx.platform_io_mut();
        unsafe {
            pio.set_platform_get_window_pos_raw(Some(get_pos));
            pio.set_platform_get_window_size_raw(Some(get_size));
            pio.set_platform_get_window_framebuffer_scale_raw(Some(get_scale));
            pio.set_platform_get_window_work_area_insets_raw(Some(get_insets));
        }

        let raw = unsafe { &*pio.as_raw() };
        assert!(raw.Platform_GetWindowPos.is_some());
        assert!(raw.Platform_GetWindowSize.is_some());
        assert!(raw.Platform_GetWindowFramebufferScale.is_some());
        assert!(raw.Platform_GetWindowWorkAreaInsets.is_some());
    }
    assert!(
        ctx.io().backend_language_user_data().is_null(),
        "PlatformIO out-param helpers must not occupy BackendLanguageUserData"
    );

    let pio = ctx.platform_io_mut();
    unsafe {
        pio.set_platform_get_window_pos_raw(None);
        pio.set_platform_get_window_size_raw(None);
        pio.set_platform_get_window_framebuffer_scale_raw(None);
        pio.set_platform_get_window_work_area_insets_raw(None);
    }

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_GetWindowPos.is_none());
    assert!(raw.Platform_GetWindowSize.is_none());
    assert!(raw.Platform_GetWindowFramebufferScale.is_none());
    assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());
}
