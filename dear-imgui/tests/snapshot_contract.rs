use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    ctx.prepare_frame(
        imgui::FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0)
            .framebuffer_scale([1.25, 1.5])
            .renderer_has_textures(),
    );
    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

fn owned_texture() -> imgui::texture::OwnedTextureData {
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 2, 2);
    texture.set_data(&[
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ]);
    texture
}

fn request_for(
    snapshot: &imgui::render::FrameSnapshot,
    id: imgui::ManagedTextureId,
) -> &imgui::render::TextureRequest {
    snapshot
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(id))
        .expect("snapshot should contain the user texture request")
}

#[test]
fn snapshot_preserves_draw_metadata_and_legacy_texture_binding() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    frame.ui().get_foreground_draw_list().add_image(
        imgui::TextureId::new(77),
        [0.0, 0.0],
        [32.0, 16.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    let snapshot = frame.render_snapshot(&consumer).unwrap();
    assert_eq!(snapshot.epoch().context_id(), ctx.id());
    assert_eq!(snapshot.draw_data().display_size, [640.0, 480.0]);
    assert_eq!(snapshot.draw_data().framebuffer_scale, [1.25, 1.5]);
    assert!(snapshot.draw_data().draw_lists.iter().any(|list| {
        list.commands.iter().any(|command| {
            matches!(
                command,
                imgui::render::DrawCmdSnapshot::Elements {
                    texture: imgui::render::TextureBinding::Legacy(id),
                    count,
                    ..
                } if *id == imgui::TextureId::new(77) && *count > 0
            )
        })
    }));
}

#[test]
fn context_consumer_captures_managed_texture_bytes_and_binding() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [24.0, 24.0]);
    let snapshot = frame.render_snapshot(&consumer).unwrap();

    let request = request_for(&snapshot, texture_id);
    assert!(matches!(
        request.operation(),
        imgui::render::TextureOp::Create { pixels, .. } if pixels.len() == 16
    ));
    assert!(snapshot.draw_data().draw_lists.iter().any(|list| {
        list.commands.iter().any(|command| {
            matches!(
                command,
                imgui::render::DrawCmdSnapshot::Elements {
                    texture: imgui::render::TextureBinding::Managed(
                        imgui::render::SnapshotTextureId::User(id)
                    ),
                    ..
                } if *id == texture_id
            )
        })
    }));
}

#[test]
fn snapshot_preserves_standard_sampler_callbacks() {
    unsafe extern "C" fn linear(
        _parent_list: *const imgui::sys::ImDrawList,
        _cmd: *const imgui::sys::ImDrawCmd,
    ) {
    }
    unsafe extern "C" fn nearest(
        _parent_list: *const imgui::sys::ImDrawList,
        _cmd: *const imgui::sys::ImDrawCmd,
    ) {
    }

    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    unsafe {
        ctx.platform_io_mut()
            .set_draw_callback_set_sampler_linear_raw(Some(linear));
        ctx.platform_io_mut()
            .set_draw_callback_set_sampler_nearest_raw(Some(nearest));
    }
    let consumer = ctx.create_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    let draw_list = frame.ui().get_foreground_draw_list();
    unsafe {
        draw_list.add_callback(Some(linear), std::ptr::null_mut(), 0);
        draw_list.add_callback(Some(nearest), std::ptr::null_mut(), 0);
    }
    drop(draw_list);
    let snapshot = frame.render_snapshot(&consumer).unwrap();
    assert!(snapshot.draw_data().draw_lists.iter().any(|list| {
        list.commands
            .iter()
            .any(|command| matches!(command, imgui::render::DrawCmdSnapshot::SetSamplerLinear))
    }));
    assert!(snapshot.draw_data().draw_lists.iter().any(|list| {
        list.commands
            .iter()
            .any(|command| matches!(command, imgui::render::DrawCmdSnapshot::SetSamplerNearest))
    }));
}

#[test]
fn unsupported_user_callback_is_an_explicit_capture_error() {
    unsafe extern "C" fn user_callback(
        _parent_list: *const imgui::sys::ImDrawList,
        _cmd: *const imgui::sys::ImDrawCmd,
    ) {
    }

    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    unsafe {
        frame.ui().get_foreground_draw_list().add_callback(
            Some(user_callback),
            std::ptr::null_mut(),
            0,
        );
    }
    assert!(matches!(
        frame.render_snapshot(&consumer),
        Err(imgui::render::SnapshotError::UserCallbackUnsupported)
    ));
}

#[test]
fn abandoned_epoch_repeats_destroy_and_blocks_consumer_replacement_until_drained() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_renderer_consumer().unwrap();

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let first = frame.render_snapshot(&consumer).unwrap();
    ctx.remove_texture(texture_id).unwrap();
    drop(first);
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 1);
    assert_eq!(progress.abandoned(), 1);

    let second = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    assert!(matches!(
        request_for(&second, texture_id).operation(),
        imgui::render::TextureOp::Destroy
    ));
    drop(consumer);
    assert!(matches!(
        ctx.create_renderer_consumer(),
        Err(imgui::render::RendererConsumerError::ConsumerDraining)
    ));
    drop(second);
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.abandoned(), 1);
    let _replacement = ctx
        .create_renderer_consumer()
        .expect("a fully drained consumer should be replaceable");
}

#[test]
fn out_of_order_completion_applies_only_after_the_contiguous_gap_closes() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let first_feedback = request_for(&first, texture_id)
        .uploaded(imgui::TextureId::new(41))
        .unwrap();
    let second = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let second_feedback = request_for(&second, texture_id)
        .uploaded(imgui::TextureId::new(42))
        .unwrap();

    second.commit([second_feedback]).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 0);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    first.commit([first_feedback]).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.committed(), 2);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(42));
    })
    .unwrap();
}

#[test]
fn out_of_order_dynamic_font_resize_keeps_every_atlas_allocation_reconcilable() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first
        .ui()
        .text("Initial detached atlas allocation: the quick brown fox.");
    let first = first.render_snapshot(&consumer).unwrap();
    let first_atlas = first
        .texture_requests()
        .iter()
        .filter_map(|request| match request.texture() {
            id @ imgui::render::SnapshotTextureId::FontAtlas { .. } => Some(id),
            imgui::render::SnapshotTextureId::User(_) => None,
        })
        .collect::<std::collections::HashSet<_>>();
    assert!(!first_atlas.is_empty());
    let first_feedback = first
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.operation() {
            imgui::render::TextureOp::Create { .. } | imgui::render::TextureOp::Update { .. } => {
                request
                    .uploaded(imgui::TextureId::new(3_000 + index as u64))
                    .unwrap()
            }
            imgui::render::TextureOp::Destroy => request.destroyed().unwrap(),
        })
        .collect::<Vec<_>>();

    ctx.style_mut().set_font_size_base(96.0);
    let second = ctx.begin_frame();
    second
        .ui()
        .text("Resized detached atlas allocation: THE QUICK BROWN FOX 0123456789.");
    let second = second.render_snapshot(&consumer).unwrap();
    let second_atlas = second
        .texture_requests()
        .iter()
        .filter_map(|request| match request.texture() {
            id @ imgui::render::SnapshotTextureId::FontAtlas { .. } => Some(id),
            imgui::render::SnapshotTextureId::User(_) => None,
        })
        .collect::<std::collections::HashSet<_>>();
    assert!(
        second_atlas.len() >= 2,
        "font resize should expose both retiring and replacement atlas allocations"
    );
    assert!(
        first_atlas.iter().any(|id| second_atlas.contains(id)),
        "the retiring allocation must remain addressable until its destroy feedback is applied"
    );
    assert!(
        second_atlas.iter().any(|id| !first_atlas.contains(id)),
        "font resize must assign a distinct allocation identity to the replacement texture"
    );
    let second_feedback = second
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.operation() {
            imgui::render::TextureOp::Create { .. } | imgui::render::TextureOp::Update { .. } => {
                request
                    .uploaded(imgui::TextureId::new(4_000 + index as u64))
                    .unwrap()
            }
            imgui::render::TextureOp::Destroy => request.destroyed().unwrap(),
        })
        .collect::<Vec<_>>();

    second.commit(second_feedback).unwrap();
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 0);

    first.commit(first_feedback).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.committed(), 2);

    let third = ctx.begin_frame();
    third.ui().text("The replacement atlas remains renderable.");
    third
        .render_snapshot(&consumer)
        .unwrap()
        .commit(std::iter::empty())
        .unwrap();
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 3);
}

#[test]
fn duplicate_feedback_abandons_the_epoch_without_partial_registry_mutation() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_renderer_consumer().unwrap();

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let snapshot = frame.render_snapshot(&consumer).unwrap();
    let request = request_for(&snapshot, texture_id);
    let first = request.uploaded(imgui::TextureId::new(51)).unwrap();
    let duplicate = request.uploaded(imgui::TextureId::new(52)).unwrap();
    snapshot.commit([first, duplicate]).unwrap();

    assert!(matches!(
        ctx.poll_snapshot_completions(),
        Err(imgui::render::RendererConsumerError::DuplicateFeedback { .. })
    ));
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();
}

#[test]
fn feedback_from_an_old_consumer_generation_cannot_mutate_a_new_generation() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let first_consumer = ctx.create_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let first = first.render_snapshot(&first_consumer).unwrap();
    let stale_feedback = request_for(&first, texture_id)
        .uploaded(imgui::TextureId::new(61))
        .unwrap();
    first.commit(std::iter::empty()).unwrap();
    ctx.poll_snapshot_completions().unwrap();
    drop(first_consumer);
    ctx.poll_snapshot_completions().unwrap();

    let second_consumer = ctx.create_renderer_consumer().unwrap();
    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    second
        .render_snapshot(&second_consumer)
        .unwrap()
        .commit([stale_feedback])
        .unwrap();
    assert!(matches!(
        ctx.poll_snapshot_completions(),
        Err(imgui::render::RendererConsumerError::StaleConsumerGeneration { .. })
    ));
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();
}

#[test]
fn feedback_from_a_foreign_context_is_rejected_before_registry_mutation() {
    let _guard = test_guard();
    let mut context_a = imgui::Context::create();
    prepare_context(&mut context_a);
    let texture_a = context_a.register_texture(owned_texture());
    let consumer_a = context_a.create_renderer_consumer().unwrap();
    let frame_a = context_a.begin_frame();
    frame_a.ui().image(texture_a, [16.0, 16.0]);
    let snapshot_a = frame_a.render_snapshot(&consumer_a).unwrap();
    let foreign_feedback = request_for(&snapshot_a, texture_a)
        .uploaded(imgui::TextureId::new(71))
        .unwrap();
    snapshot_a.commit(std::iter::empty()).unwrap();
    context_a.poll_snapshot_completions().unwrap();
    drop(consumer_a);
    context_a.poll_snapshot_completions().unwrap();
    let suspended_a = context_a.suspend();

    let mut context_b = imgui::Context::create();
    prepare_context(&mut context_b);
    let texture_b = context_b.register_texture(owned_texture());
    let consumer_b = context_b.create_renderer_consumer().unwrap();
    let frame_b = context_b.begin_frame();
    frame_b.ui().image(texture_b, [16.0, 16.0]);
    frame_b
        .render_snapshot(&consumer_b)
        .unwrap()
        .commit([foreign_feedback])
        .unwrap();
    assert!(matches!(
        context_b.poll_snapshot_completions(),
        Err(imgui::render::RendererConsumerError::ForeignContext { .. })
    ));
    context_b
        .with_texture(texture_b, |texture| {
            assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
            assert!(texture.texture_id().is_null());
        })
        .unwrap();

    drop(consumer_b);
    drop(context_b);
    drop(suspended_a.activate().unwrap());
}

#[test]
fn stale_feedback_cannot_mutate_a_texture_in_a_reused_slot() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let first_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first.ui().image(first_id, [16.0, 16.0]);
    let first = first.render_snapshot(&consumer).unwrap();
    let request = request_for(&first, first_id);
    let create = request.uploaded(imgui::TextureId::new(81)).unwrap();
    let stale_create = request.uploaded(imgui::TextureId::new(82)).unwrap();
    first.commit([create]).unwrap();
    ctx.poll_snapshot_completions().unwrap();

    ctx.remove_texture(first_id).unwrap();
    let destroy = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let destroyed = request_for(&destroy, first_id).destroyed().unwrap();
    destroy.commit([destroyed]).unwrap();
    ctx.poll_snapshot_completions().unwrap();

    let replacement_id = ctx.register_texture(owned_texture());
    assert_ne!(replacement_id, first_id);
    let replacement = ctx.begin_frame();
    replacement.ui().image(replacement_id, [16.0, 16.0]);
    replacement
        .render_snapshot(&consumer)
        .unwrap()
        .commit([stale_create])
        .unwrap();
    assert!(matches!(
        ctx.poll_snapshot_completions(),
        Err(imgui::render::RendererConsumerError::FeedbackNotRequested { .. })
    ));
    ctx.with_texture(replacement_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();
}

#[test]
fn remove_after_capture_waits_for_create_then_matching_destroy_completion() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let created = request_for(&first, texture_id)
        .uploaded(imgui::TextureId::new(71))
        .unwrap();
    ctx.remove_texture(texture_id).unwrap();
    first.commit([created]).unwrap();
    ctx.poll_snapshot_completions().unwrap();
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::Retiring(texture_id))
    );

    let destroy = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let destroyed = request_for(&destroy, texture_id).destroyed().unwrap();
    destroy.commit([destroyed]).unwrap();
    ctx.poll_snapshot_completions().unwrap();
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::AlreadyRemoved(texture_id))
    );
}

#[test]
fn foreign_consumer_is_rejected_before_capture() {
    let _guard = test_guard();
    let mut context_a = imgui::Context::create();
    prepare_context(&mut context_a);
    let consumer_a = context_a.create_renderer_consumer().unwrap();
    let suspended_a = context_a.suspend();

    let mut context_b = imgui::Context::create();
    prepare_context(&mut context_b);
    let frame = context_b.begin_frame();
    assert!(matches!(
        frame.render_snapshot(&consumer_a),
        Err(imgui::render::SnapshotError::Consumer(
            imgui::render::RendererConsumerError::ForeignContext { .. }
        ))
    ));
    drop(context_b);
    drop(suspended_a.activate().unwrap());
}

#[test]
fn context_drop_closes_completion_without_cross_thread_panic() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();
    let snapshot = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    drop(consumer);
    drop(ctx);
    assert_eq!(
        snapshot.commit(std::iter::empty()),
        Err(imgui::render::SnapshotCommitError::ContextDropped)
    );
}

#[test]
fn renderer_reset_rejects_an_outstanding_detached_epoch() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_renderer_consumer().unwrap();
    let snapshot = ctx.begin_frame().render_snapshot(&consumer).unwrap();

    assert_eq!(
        ctx.reset_renderer_texture_bindings(&consumer),
        Err(imgui::render::RendererConsumerError::OutstandingEpochs { count: 1 })
    );
    drop(snapshot);
    ctx.poll_snapshot_completions().unwrap();
    ctx.reset_renderer_texture_bindings(&consumer).unwrap();
}
