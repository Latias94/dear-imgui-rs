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
    let _consumer = ctx.create_renderer_consumer().unwrap();

    let frame = ctx.begin_frame();
    assert_eq!(frame.lifecycle_state(), imgui::FrameLifecycleState::InFrame);
    frame.ui().text("first system");
    frame.ui().text("second system");

    let rendered_frame = frame.render();
    assert!(rendered_frame.valid());
    drop(rendered_frame);
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn context_can_snapshot_an_engine_owned_main_viewport_frame() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();

    let ui = ctx.frame();
    ui.text("engine-owned frame without a retained FrameToken");
    let snapshot = ctx.render_snapshot(&consumer).unwrap();

    assert!(!snapshot.draw_data().draw_lists.is_empty());
    snapshot.commit(std::iter::empty()).unwrap();
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
    let _consumer = ctx.create_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    frame.ui().text("next frame still starts cleanly");
    let rendered_frame = frame.render();
    assert!(rendered_frame.valid());
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

    assert_panics!({
        let _ = ctx.render();
    });
}

#[test]
fn frame_token_captures_font_managed_draws_with_a_context_consumer() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();

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
    let _consumer = ctx.create_renderer_consumer().unwrap();

    let result = ctx.frame_with_result(|ui| {
        ui.text("system A");
        ui.text("system B");
        42usize
    });

    assert_eq!(result.value, 42);
    assert!(result.rendered_frame.valid());
    drop(result);
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn synchronous_rendered_frame_reconciles_request_bound_feedback() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let _consumer = ctx.create_renderer_consumer().unwrap();
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let mut rendered = frame.render();
    let request = rendered
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .expect("synchronous frame should expose the managed create request");
    let feedback = request.uploaded(imgui::TextureId::new(99)).unwrap();
    rendered.reconcile_texture_feedback([feedback]).unwrap();
    assert!(rendered.valid());
    drop(rendered);

    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(99));
    })
    .unwrap();
}

#[test]
fn synchronous_rendered_frame_completion_binds_its_owner_context() {
    let _guard = test_guard();

    let ctx_b = imgui::Context::create();
    let binding_b = ctx_b.binding();
    let raw_b = ctx_b.as_raw();
    let suspended_b = ctx_b.suspend();

    let mut ctx_a = imgui::Context::create();
    prepare_context(&mut ctx_a);
    let binding_a = ctx_a.binding();
    let _consumer_a = ctx_a.create_renderer_consumer().unwrap();
    let user_textures_before = unsafe { (*ctx_a.as_raw()).UserTextures.Size };

    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx_a.register_texture(texture);
    assert_eq!(
        unsafe { (*ctx_a.as_raw()).UserTextures.Size },
        user_textures_before + 1
    );

    let mut created = binding_a.with_bound_context(|| {
        let frame = ctx_a.begin_frame();
        frame.ui().image(texture_id, [16.0, 16.0]);
        frame.render()
    });
    let uploaded = created
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(141))
        .unwrap();
    binding_b.with_bound_context(|| {
        created.reconcile_texture_feedback([uploaded]).unwrap();
        drop(created);
        assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, raw_b);
    });

    binding_a.with_bound_context(|| ctx_a.remove_texture(texture_id).unwrap());
    let mut destroyed = binding_a.with_bound_context(|| ctx_a.begin_frame().render());
    let feedback = destroyed
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .destroyed()
        .unwrap();
    binding_b.with_bound_context(|| {
        destroyed.reconcile_texture_feedback([feedback]).unwrap();
        drop(destroyed);
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
fn synchronous_rendered_frame_abandon_reissues_unacknowledged_requests() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let _consumer = ctx.create_renderer_consumer().unwrap();
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx.register_texture(texture);

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let rendered = first.render();
    assert!(rendered.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
            && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
    }));
    drop(rendered);

    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    let mut rendered = second.render();
    let request = rendered
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .expect("the abandoned create request should be emitted again");
    let feedback = request.uploaded(imgui::TextureId::new(101)).unwrap();
    rendered.reconcile_texture_feedback([feedback]).unwrap();
    drop(rendered);

    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(101));
    })
    .unwrap();
}

#[test]
fn renderer_consumer_generation_cannot_switch_render_modes() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();

    let mut rendered = ctx.begin_frame().render();
    rendered
        .reconcile_texture_feedback(std::iter::empty())
        .unwrap();
    drop(rendered);

    let result = ctx.begin_frame().render_snapshot(&consumer);
    assert!(matches!(
        result,
        Err(imgui::render::SnapshotError::Consumer(
            imgui::render::RendererConsumerError::ConsumerModeMismatch
        ))
    ));
}

#[test]
fn renderer_reset_permit_is_inert_until_committed() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx.register_texture(texture);

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let mut first = first.render();
    let created = first
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(111))
        .unwrap();
    first.reconcile_texture_feedback([created]).unwrap();
    drop(first);

    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    drop(reset);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(111));
    })
    .unwrap();

    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    assert!(reset.commit() >= 1);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    let second = second.render();
    assert!(second.texture_requests().iter().any(|request| {
        request.texture() == imgui::render::SnapshotTextureId::User(texture_id)
            && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
    }));
}

#[test]
fn renderer_reset_commit_binds_its_owner_context() {
    let _guard = test_guard();

    let foreign = imgui::Context::create();
    let foreign_binding = foreign.binding();
    let foreign_raw = foreign.as_raw();
    let suspended_foreign = foreign.suspend();

    let mut owner = imgui::Context::create();
    prepare_context(&mut owner);
    let consumer = owner.create_renderer_consumer().unwrap();
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = owner.register_texture(texture);

    let frame = owner.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let mut frame = frame.render();
    let created = frame
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(121))
        .unwrap();
    frame.reconcile_texture_feedback([created]).unwrap();
    drop(frame);

    let reset = owner.prepare_renderer_texture_reset(&consumer).unwrap();
    foreign_binding.with_bound_context(|| {
        assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, foreign_raw);
        assert!(reset.commit() >= 1);
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
    let consumer = ctx.create_renderer_consumer().unwrap();
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let mut frame = frame.render();
    let created = frame
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture_id))
        .unwrap()
        .uploaded(imgui::TextureId::new(121))
        .unwrap();
    frame.reconcile_texture_feedback([created]).unwrap();
    drop(frame);

    ctx.remove_texture(texture_id).unwrap();
    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    assert!(reset.commit() >= 1);
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::AlreadyRemoved(texture_id))
    );
}

#[test]
fn dynamic_font_resize_reconciles_overlapping_atlas_allocations() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let _consumer = ctx.create_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first
        .ui()
        .text("Initial atlas allocation: the quick brown fox jumps over the lazy dog.");
    let mut first = first.render();
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
    first.reconcile_texture_feedback(first_feedback).unwrap();
    drop(first);

    ctx.style_mut().set_font_size_base(96.0);
    let second = ctx.begin_frame();
    second
        .ui()
        .text("Resized atlas allocation: THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789.");
    let mut second = second.render();

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
    let feedback = atlas_requests
        .into_iter()
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
    second.reconcile_texture_feedback(feedback).unwrap();
}
