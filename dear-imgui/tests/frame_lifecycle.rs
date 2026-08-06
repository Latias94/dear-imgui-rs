use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    ctx.prepare_frame(
        imgui::FramePrepareOptions::new([800.0, 600.0], 1.0 / 60.0)
            .framebuffer_scale([2.0, 2.0])
            .renderer_has_textures(),
    );
    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

fn reconcile_test_frame<'ctx>(
    frame: imgui::render::PendingFrame<'ctx>,
    user_bindings: &[(imgui::ManagedTextureId, imgui::TextureId)],
) -> imgui::render::ReconciledFrame<'ctx> {
    let feedback = frame
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                let texture_id = match request.texture() {
                    imgui::render::SnapshotTextureId::User(id) => user_bindings
                        .iter()
                        .find_map(|(candidate, binding)| (*candidate == id).then_some(*binding))
                        .unwrap_or_else(|| imgui::TextureId::new(10_000 + index as u64)),
                    imgui::render::SnapshotTextureId::FontAtlas { .. } => {
                        imgui::TextureId::new(20_000 + index as u64)
                    }
                };
                request.uploaded(texture_id)
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("test renderer feedback must match every request");
    frame
        .reconcile_texture_feedback(feedback)
        .expect("test renderer feedback must reconcile")
}

macro_rules! assert_panics {
    ($body:block) => {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err());
    };
}

#[test]
fn prepare_frame_sets_engine_owned_io_before_beginning_frame() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    ctx.prepare_frame(
        imgui::FramePrepareOptions::new([320.0, 240.0], 1.0 / 120.0)
            .framebuffer_scale([1.5, 2.0])
            .renderer_has_textures(),
    );

    assert_eq!(ctx.io().display_size(), [320.0, 240.0]);
    assert_eq!(ctx.io().delta_time(), 1.0 / 120.0);
    assert_eq!(ctx.io().display_framebuffer_scale(), [1.5, 2.0]);
    assert!(
        ctx.io()
            .backend_flags()
            .contains(imgui::BackendFlags::RENDERER_HAS_TEXTURES)
    );
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Idle
    );
}

#[test]
fn frame_token_allows_engine_owned_begin_ui_end_flow() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();

    let frame = ctx.begin_frame();
    assert_eq!(frame.lifecycle_state(), imgui::FrameLifecycleState::InFrame);
    frame.ui().text("first system");
    frame.ui().text("second system");

    let pending = frame.render(&consumer);
    let feedback = pending
        .texture_requests()
        .iter()
        .map(|request| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                request.uploaded(imgui::TextureId::new(81))
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let reconciled = pending.reconcile_texture_feedback(feedback).unwrap();
    assert!(reconciled.valid());
    drop(reconciled);
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn reconciliation_consumes_the_pending_frame_and_returns_draw_access() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let context_id = ctx.id();
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();

    let unreconciled = ctx.begin_frame().render(&consumer);
    assert!(!unreconciled.texture_requests().is_empty());
    drop(unreconciled);

    prepare_context(&mut ctx);
    let pending = ctx.begin_frame().render(&consumer);
    let feedback = pending
        .texture_requests()
        .iter()
        .map(|request| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                request.uploaded(imgui::TextureId::new(91))
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let reconciled = pending.reconcile_texture_feedback(feedback).unwrap();
    assert_eq!(reconciled.context_id(), context_id);
    assert!(reconciled.epoch().is_some());
    assert!(reconciled.valid());
    drop(reconciled);
}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn test_platform_renderer_callback(
    _viewport: *mut imgui::sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
}

#[cfg(feature = "multi-viewport")]
#[test]
fn managed_platform_pump_is_only_available_after_reconciliation() {
    let _guard = test_guard();
    let mut context = imgui::Context::create();
    prepare_context(&mut context);
    let consumer = context.create_synchronous_renderer_consumer().unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_render_window_raw(Some(test_platform_renderer_callback));
    }

    let pending = context.begin_frame().render(&consumer);
    let mut frame = reconcile_test_frame(pending, &[]);
    frame.update_and_render_platform_windows_default();
}

#[cfg(feature = "multi-viewport")]
#[test]
fn abandoned_managed_frame_cannot_enter_default_platform_renderer_callbacks() {
    let _guard = test_guard();
    let mut context = imgui::Context::create();
    prepare_context(&mut context);
    let consumer = context.create_synchronous_renderer_consumer().unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_render_window_raw(Some(test_platform_renderer_callback));
    }

    drop(context.begin_frame().render(&consumer));
    context.update_platform_windows();
    assert_panics!({
        context.render_platform_windows_default();
    });
}

#[test]
fn context_can_snapshot_an_engine_owned_main_viewport_frame() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let ui = ctx.frame();
    ui.text("engine-owned frame without a retained FrameToken");
    let snapshot = ctx.render_snapshot(&consumer).unwrap();

    assert!(!snapshot.draw_data().draw_lists.is_empty());
    let feedback = snapshot
        .texture_requests()
        .iter()
        .map(|request| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                request.uploaded(imgui::TextureId::new(82))
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    snapshot.commit(feedback).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.abandoned(), 0);
}

#[test]
fn dropping_frame_token_ends_frame_without_rendering() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    {
        let frame = ctx.begin_frame();
        assert_eq!(frame.lifecycle_state(), imgui::FrameLifecycleState::InFrame);
        frame.ui().text("dropped frame");
    }

    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Idle
    );
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    frame.ui().text("next frame still starts cleanly");
    let pending = frame.render(&consumer);
    let feedback = pending
        .texture_requests()
        .iter()
        .map(|request| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                request.uploaded(imgui::TextureId::new(83))
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let reconciled = pending.reconcile_texture_feedback(feedback).unwrap();
    assert!(reconciled.valid());
}

#[test]
fn dropping_frame_token_ends_owner_context_and_restores_previous_current_context() {
    let _guard = test_guard();

    let mut ctx_a = imgui::Context::create();
    prepare_context(&mut ctx_a);
    let raw_a = ctx_a.as_raw();
    let raw_b = unsafe { imgui::sys::igCreateContext(std::ptr::null_mut()) };
    assert!(!raw_b.is_null());

    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    let frame = ctx_a.begin_frame();
    frame.ui().text("owner frame");

    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    drop(frame);

    assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, raw_b);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    assert_eq!(
        ctx_a.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Idle
    );

    unsafe { imgui::sys::igDestroyContext(raw_b) };
    drop(ctx_a);
}

#[test]
fn render_without_beginning_frame_panics_before_entering_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();

    assert_panics!({
        let _ = ctx.render(&consumer);
    });
}

#[test]
fn frame_token_captures_font_managed_draws_with_a_context_consumer() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let frame = ctx.begin_frame();
    frame.ui().text("snapshot me");

    let snapshot = frame.render_snapshot(&consumer).unwrap();
    assert!(snapshot.texture_requests().iter().any(|request| {
        matches!(
            request.texture(),
            imgui::render::SnapshotTextureId::FontAtlas { .. }
        )
    }));
}

#[test]
fn frame_with_result_lets_engines_run_multiple_ui_steps_before_rendering() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();

    let result = ctx.frame_with_result(&consumer, |ui| {
        ui.text("system A");
        ui.text("system B");
        42usize
    });
    let (value, pending_frame) = result.into_parts();

    assert_eq!(value, 42);
    let feedback = pending_frame
        .texture_requests()
        .iter()
        .map(|request| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                request.uploaded(imgui::TextureId::new(84))
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let reconciled = pending_frame.reconcile_texture_feedback(feedback).unwrap();
    assert!(reconciled.valid());
    drop(reconciled);
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn synchronous_pending_frame_reconciles_request_bound_feedback() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let pending = frame.render(&consumer);
    let _request = pending
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .expect("synchronous frame should expose the managed create request");
    let reconciled = reconcile_test_frame(pending, &[(texture_id, imgui::TextureId::new(99))]);
    assert!(reconciled.valid());
    drop(reconciled);

    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(99));
    })
    .unwrap();
}

#[test]
fn synchronous_reconciled_frame_completion_binds_its_owner_context() {
    let _guard = test_guard();

    let ctx_b = imgui::Context::create();
    let binding_b = ctx_b.binding();
    let raw_b = ctx_b.as_raw();
    let suspended_b = ctx_b.suspend_or_panic();

    let mut ctx_a = imgui::Context::create();
    prepare_context(&mut ctx_a);
    let binding_a = ctx_a.binding();
    let consumer_a = ctx_a.create_synchronous_renderer_consumer().unwrap();
    let user_textures_before = unsafe { (*ctx_a.as_raw()).UserTextures.Size };

    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx_a.register_texture(texture);
    assert_eq!(
        unsafe { (*ctx_a.as_raw()).UserTextures.Size },
        user_textures_before + 1
    );

    let created = binding_a.with_bound_context(|| {
        let frame = ctx_a.begin_frame();
        frame.ui().image(texture_id, [16.0, 16.0]);
        frame.render(&consumer_a)
    });
    let _uploaded = created
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(141))
        .unwrap();
    binding_b.with_bound_context(|| {
        drop(reconcile_test_frame(
            created,
            &[(texture_id, imgui::TextureId::new(141))],
        ));
        assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, raw_b);
    });

    binding_a.with_bound_context(|| ctx_a.remove_texture(texture_id).unwrap());
    let destroyed = binding_a.with_bound_context(|| ctx_a.begin_frame().render(&consumer_a));
    let _feedback = destroyed
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .destroyed()
        .unwrap();
    binding_b.with_bound_context(|| {
        drop(reconcile_test_frame(destroyed, &[]));
        assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, raw_b);
    });

    assert_eq!(
        binding_a.with_bound_context(|| unsafe { (*ctx_a.as_raw()).UserTextures.Size }),
        user_textures_before,
        "owner-bound completion must unregister only from the frame's Context"
    );

    drop(ctx_a);
    drop(suspended_b);
}

#[test]
fn synchronous_pending_frame_abandon_reissues_unacknowledged_requests() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let pending = first.render(&consumer);
    assert!(pending.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
            && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
    }));
    drop(pending);

    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    let pending = second.render(&consumer);
    let _request = pending
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .expect("the abandoned create request should be emitted again");
    drop(reconcile_test_frame(
        pending,
        &[(texture_id, imgui::TextureId::new(101))],
    ));

    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(101));
    })
    .unwrap();
}

#[test]
fn synchronous_retry_keeps_the_frame_drawable_and_reissues_the_same_upload() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let pending = first.render(&consumer);
    let first_identity = pending
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .and_then(imgui::render::TextureRequest::upload_identity)
        .expect("create request must expose a stable upload identity");
    let feedback = pending
        .texture_requests()
        .iter()
        .map(imgui::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    let frame = pending.reconcile_texture_feedback(feedback).unwrap();
    assert!(frame.valid());
    drop(frame);

    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    let pending = second.render(&consumer);
    let second_identity = pending
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .and_then(imgui::render::TextureRequest::upload_identity)
        .expect("retried create request must be reissued");
    assert_eq!(second_identity, first_identity);
}

#[test]
fn invalid_synchronous_feedback_abandons_without_wedging_the_consumer() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let pending = frame.render(&consumer);
    let request = pending
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .expect("managed texture request must be present");
    let first = request.uploaded(imgui::TextureId::new(161)).unwrap();
    let duplicate = request.uploaded(imgui::TextureId::new(162)).unwrap();
    let mut invalid = pending
        .texture_requests()
        .iter()
        .map(imgui::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    invalid.push(first);
    invalid.push(duplicate);

    assert!(matches!(
        pending.reconcile_texture_feedback(invalid),
        Err(imgui::render::RendererConsumerError::DuplicateFeedback { .. })
    ));
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    let retry = ctx.begin_frame();
    retry.ui().image(texture_id, [16.0, 16.0]);
    let pending = retry.render(&consumer);
    assert!(pending.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
    }));
}

#[test]
fn synchronous_destroy_retry_does_not_acknowledge_retirement_or_block_drawing() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    drop(reconcile_test_frame(
        first.render(&consumer),
        &[(texture_id, imgui::TextureId::new(151))],
    ));

    ctx.remove_texture(texture_id).unwrap();
    let pending = ctx.begin_frame().render(&consumer);
    assert!(pending.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
            && request.kind() == imgui::render::TextureRequestKind::Destroy
    }));
    let feedback = pending
        .texture_requests()
        .iter()
        .map(imgui::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    let frame = pending.reconcile_texture_feedback(feedback).unwrap();
    assert!(frame.valid());
    drop(frame);
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::Retiring(texture_id))
    );

    let retry = ctx.begin_frame().render(&consumer);
    assert!(retry.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
            && request.kind() == imgui::render::TextureRequestKind::Destroy
    }));
    drop(reconcile_test_frame(retry, &[]));
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::AlreadyRemoved(texture_id))
    );
}

#[test]
fn pending_frame_exposes_pointer_free_raw_callback_requirements() {
    unsafe extern "C" fn callback(
        _draw_list: *const imgui::sys::ImDrawList,
        _command: *const imgui::sys::ImDrawCmd,
    ) {
    }

    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    unsafe {
        frame
            .ui()
            .get_foreground_draw_list()
            .add_callback(callback, std::ptr::null_mut(), 0);
    }
    let pending = frame.render(&consumer);
    assert!(pending.draw_requirements().requires_raw_callback_support());
}

#[test]
fn renderer_consumer_kind_is_selected_at_creation() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    assert!(matches!(
        ctx.create_detached_renderer_consumer(),
        Err(imgui::render::RendererConsumerError::ConsumerAlreadyActive)
    ));

    let pending = ctx.begin_frame().render(&consumer);
    let feedback = pending
        .texture_requests()
        .iter()
        .map(|request| match request.kind() {
            imgui::render::TextureRequestKind::Create
            | imgui::render::TextureRequestKind::Update => {
                request.uploaded(imgui::TextureId::new(102))
            }
            imgui::render::TextureRequestKind::Destroy => request.destroyed(),
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(pending.reconcile_texture_feedback(feedback).unwrap());
    drop(consumer);
    ctx.poll_snapshot_completions().unwrap();

    let detached = ctx.create_detached_renderer_consumer().unwrap();
    let snapshot = ctx.begin_frame().render_snapshot(&detached).unwrap();
    let feedback = snapshot
        .texture_requests()
        .iter()
        .map(|request| request.superseded())
        .collect::<Vec<_>>();
    snapshot.commit(feedback).unwrap();
}

#[test]
fn renderer_reset_permit_is_inert_until_committed() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let first = first.render(&consumer);
    let _created = first
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(111))
        .unwrap();
    drop(reconcile_test_frame(
        first,
        &[(texture_id, imgui::TextureId::new(111))],
    ));

    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    drop(reset);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(111));
    })
    .unwrap();

    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    let committed: () = reset.commit();
    assert_eq!(committed, ());
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    let second = second.render(&consumer);
    assert!(second.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
            && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
    }));
}

#[test]
fn renderer_reset_commit_is_unit_for_an_empty_context() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    let committed: () = reset.commit();
    assert_eq!(committed, ());
}

#[test]
fn renderer_reset_commit_binds_its_owner_context() {
    let _guard = test_guard();

    let foreign = imgui::Context::create();
    let foreign_binding = foreign.binding();
    let foreign_raw = foreign.as_raw();
    let suspended_foreign = foreign.suspend_or_panic();

    let mut owner = imgui::Context::create();
    prepare_context(&mut owner);
    let consumer = owner.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = owner.register_texture(texture);

    let frame = owner.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let frame = frame.render(&consumer);
    let _created = frame
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(121))
        .unwrap();
    drop(reconcile_test_frame(
        frame,
        &[(texture_id, imgui::TextureId::new(121))],
    ));

    let reset = owner.prepare_renderer_texture_reset(&consumer).unwrap();
    foreign_binding.with_bound_context(|| {
        assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, foreign_raw);
        reset.commit();
        assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, foreign_raw);
    });

    owner
        .with_texture(texture_id, |texture| {
            assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
            assert!(texture.texture_id().is_null());
        })
        .unwrap();

    drop(owner);
    drop(suspended_foreign);
}

#[test]
fn renderer_reset_acknowledges_retiring_textures_after_the_last_epoch() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let frame = frame.render(&consumer);
    let _created = frame
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(121))
        .unwrap();
    drop(reconcile_test_frame(
        frame,
        &[(texture_id, imgui::TextureId::new(121))],
    ));

    ctx.remove_texture(texture_id).unwrap();
    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    reset.commit();
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::AlreadyRemoved(texture_id))
    );
}

#[test]
fn renderer_reset_handles_atlas_active_and_retiring_bindings_as_one_transaction() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let active = ctx.register_texture(
        imgui::texture::OwnedTextureData::from_pixels(
            imgui::texture::TextureFormat::RGBA32,
            1,
            1,
            &[1, 2, 3, 4],
        )
        .unwrap(),
    );
    let retiring = ctx.register_texture(
        imgui::texture::OwnedTextureData::from_pixels(
            imgui::texture::TextureFormat::RGBA32,
            1,
            1,
            &[4, 3, 2, 1],
        )
        .unwrap(),
    );

    let frame = ctx.begin_frame();
    frame.ui().image(active, [16.0, 16.0]);
    frame.ui().image(retiring, [16.0, 16.0]);
    let frame = reconcile_test_frame(
        frame.render(&consumer),
        &[
            (active, imgui::TextureId::new(131)),
            (retiring, imgui::TextureId::new(132)),
        ],
    );
    drop(frame);
    ctx.remove_texture(retiring).unwrap();

    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    let committed: () = reset.commit();
    assert_eq!(committed, ());
    ctx.with_texture(active, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();
    assert_eq!(
        ctx.with_texture(retiring, |_| ()),
        Err(imgui::ManagedTextureError::AlreadyRemoved(retiring))
    );

    let frame = ctx.begin_frame();
    frame.ui().image(active, [16.0, 16.0]);
    let pending = frame.render(&consumer);
    assert!(pending.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(active)
            && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
    }));
    assert!(pending.texture_requests().iter().any(|request| {
        matches!(
            request.texture(),
            imgui::render::SnapshotTextureId::FontAtlas { .. }
        ) && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
    }));
}

#[test]
fn dynamic_font_resize_reconciles_overlapping_atlas_allocations() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first
        .ui()
        .text("Initial atlas allocation: the quick brown fox jumps over the lazy dog.");
    let first = first.render(&consumer);
    let first_feedback = first
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.operation() {
            imgui::render::TextureOp::Create { .. } | imgui::render::TextureOp::Update { .. } => {
                request
                    .uploaded(imgui::TextureId::new(1_000 + index as u64))
                    .unwrap()
            }
            imgui::render::TextureOp::Destroy => request.destroyed().unwrap(),
        })
        .collect::<Vec<_>>();
    drop(first.reconcile_texture_feedback(first_feedback).unwrap());

    ctx.style_mut().set_font_size_base(96.0);
    let second = ctx.begin_frame();
    second
        .ui()
        .text("Resized atlas allocation: THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789.");
    let second = second.render(&consumer);

    let atlas_requests = second
        .texture_requests()
        .iter()
        .filter(|request| {
            matches!(
                request.texture(),
                imgui::render::SnapshotTextureId::FontAtlas { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        atlas_requests.len() >= 2,
        "resizing should retain the old atlas allocation while creating its replacement"
    );
    let feedback = second
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.operation() {
            imgui::render::TextureOp::Create { .. } | imgui::render::TextureOp::Update { .. } => {
                request
                    .uploaded(imgui::TextureId::new(2_000 + index as u64))
                    .unwrap()
            }
            imgui::render::TextureOp::Destroy => request.destroyed().unwrap(),
        })
        .collect::<Vec<_>>();
    drop(second.reconcile_texture_feedback(feedback).unwrap());
}
