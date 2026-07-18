use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentRole, ContextBindingError,
    ContextDestroyed, ContextTeardown, binding::with_bound_context,
};

struct PlatformMarker;
struct RendererMarker;
struct ExtensionMarker;
struct PanickingExtensionMarker;

struct RecordingAttachment {
    log: Rc<RefCell<Vec<&'static str>>>,
    binding: Option<super::ContextBinding>,
    ordinary_rejected: Rc<Cell<bool>>,
    teardown_bound: Rc<Cell<bool>>,
    panic_during_quiesce: bool,
}

impl RecordingAttachment {
    fn new(log: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            log,
            binding: None,
            ordinary_rejected: Rc::new(Cell::new(false)),
            teardown_bound: Rc::new(Cell::new(false)),
            panic_during_quiesce: false,
        }
    }
}

impl ContextAttachment for RecordingAttachment {
    fn quiesce(&self, context: &ContextTeardown<'_>) {
        assert_eq!(context.phase(), super::ContextAttachmentPhase::Quiesce);
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
    }

    fn release_renderer_resources(&self, context: &ContextTeardown<'_>) {
        assert_eq!(
            context.phase(),
            super::ContextAttachmentPhase::RendererResources
        );
        self.log.borrow_mut().push("renderer");
    }

    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        assert_eq!(
            context.phase(),
            super::ContextAttachmentPhase::PlatformWindows
        );
        self.log.borrow_mut().push("platform");
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.log.borrow_mut().push("post");
    }
}

#[test]
fn platform_io_shared_and_mut_views_match() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let shared = ctx.platform_io().as_raw();
    let mutable = ctx.platform_io_mut().as_raw();
    assert_eq!(shared, mutable);
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
fn set_monitors_replaces_and_clears_imgui_owned_storage() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut first = crate::sys::ImGuiPlatformMonitor::default();
    first.MainPos = crate::sys::ImVec2 { x: 10.0, y: 20.0 };
    first.DpiScale = 1.5;
    let mut second = crate::sys::ImGuiPlatformMonitor::default();
    second.MainPos = crate::sys::ImVec2 { x: 30.0, y: 40.0 };
    second.DpiScale = 2.0;

    ctx.platform_io_mut().set_monitors(&[first, second]);

    let raw = unsafe { &(*ctx.platform_io().as_raw()).Monitors };
    assert_eq!(raw.Size, 2);
    assert_eq!(raw.Capacity, 2);
    assert!(!raw.Data.is_null());
    let stored = unsafe { std::slice::from_raw_parts(raw.Data, raw.Size as usize) };
    assert_eq!(stored[0].MainPos.x, 10.0);
    assert_eq!(stored[1].MainPos.y, 40.0);
    assert_eq!(stored[1].DpiScale, 2.0);

    ctx.platform_io_mut().set_monitors(&[]);

    let raw = unsafe { &(*ctx.platform_io().as_raw()).Monitors };
    assert_eq!(raw.Size, 0);
    assert_eq!(raw.Capacity, 0);
    assert!(raw.Data.is_null());
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

    binding.with_bound_context(|| drop(active));

    assert!(unsafe { crate::sys::igGetCurrentContext() }.is_null());
    let replacement = Context::create();
    drop(replacement);
    drop(suspended);
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
fn attachment_panics_do_not_skip_remaining_teardown() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let log = Rc::new(RefCell::new(Vec::new()));

    let mut panicking = RecordingAttachment::new(Rc::clone(&log));
    panicking.panic_during_quiesce = true;
    let normal = Rc::new(RecordingAttachment::new(Rc::clone(&log)));
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

    drop(ctx);

    let log = log.borrow();
    assert_eq!(log.iter().filter(|entry| **entry == "quiesce").count(), 2);
    assert_eq!(log.iter().filter(|entry| **entry == "renderer").count(), 2);
    assert_eq!(log.iter().filter(|entry| **entry == "platform").count(), 2);
    assert_eq!(log.iter().filter(|entry| **entry == "post").count(), 2);
    assert_eq!(
        binding.lifecycle(),
        super::ContextLifecycle::NativeDestroyed
    );
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

    assert!(platform_lease.detach());
    assert!(!platform_lease.detach());
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
    assert!(lease.detach());
    assert_eq!(Rc::strong_count(&attachment), 1);
    drop(ctx);
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
    ctx_a.io_mut().set_backend_language_user_data(marker_a);
    let pio_a = ctx_a.platform_io().as_raw();
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    let marker_b = std::ptr::NonNull::<u16>::dangling().as_ptr().cast();
    ctx_b.io_mut().set_backend_language_user_data(marker_b);
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

        assert_ne!(owner_color, color_a);
        assert_eq!(current_color, color_b);
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
    let _ = ctx.font_atlas().build();
    ctx.prepare_frame(super::FramePrepareOptions::new([320.0, 240.0], 1.0 / 60.0));
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
        pio.set_platform_get_window_pos_raw(Some(get_pos));
        pio.set_platform_get_window_size_raw(Some(get_size));
        pio.set_platform_get_window_framebuffer_scale_raw(Some(get_scale));
        pio.set_platform_get_window_work_area_insets_raw(Some(get_insets));

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
    pio.set_platform_get_window_pos_raw(None);
    pio.set_platform_get_window_size_raw(None);
    pio.set_platform_get_window_framebuffer_scale_raw(None);
    pio.set_platform_get_window_work_area_insets_raw(None);

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_GetWindowPos.is_none());
    assert!(raw.Platform_GetWindowSize.is_none());
    assert!(raw.Platform_GetWindowFramebufferScale.is_none());
    assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());
}
