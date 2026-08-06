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

fn retry_all(snapshot: &imgui::render::FrameSnapshot) -> Vec<imgui::render::TextureFeedback> {
    snapshot
        .texture_requests()
        .iter()
        .map(imgui::render::TextureRequest::retry)
        .collect()
}

fn upload_user(
    snapshot: &imgui::render::FrameSnapshot,
    id: imgui::ManagedTextureId,
    binding: imgui::TextureId,
) -> Vec<imgui::render::TextureFeedback> {
    snapshot
        .texture_requests()
        .iter()
        .map(|request| {
            if request.texture() == imgui::render::SnapshotTextureId::User(id) {
                request
                    .uploaded(binding)
                    .expect("user texture upload feedback must match the request")
            } else {
                request.retry()
            }
        })
        .collect()
}

fn destroy_user(
    snapshot: &imgui::render::FrameSnapshot,
    id: imgui::ManagedTextureId,
) -> Vec<imgui::render::TextureFeedback> {
    snapshot
        .texture_requests()
        .iter()
        .map(|request| {
            if request.texture() == imgui::render::SnapshotTextureId::User(id) {
                request
                    .destroyed()
                    .expect("user texture destroy feedback must match the request")
            } else {
                request.retry()
            }
        })
        .collect()
}

fn acknowledge_all(
    snapshot: &imgui::render::FrameSnapshot,
    binding_base: u64,
) -> Vec<imgui::render::TextureFeedback> {
    snapshot
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.operation() {
            imgui::render::TextureOp::Create { .. } | imgui::render::TextureOp::Update { .. } => {
                request
                    .uploaded(imgui::TextureId::new(binding_base + index as u64))
                    .expect("upload feedback must match the request")
            }
            imgui::render::TextureOp::Destroy => request
                .destroyed()
                .expect("destroy feedback must match the request"),
        })
        .collect()
}

#[test]
fn snapshot_preserves_draw_metadata_and_legacy_texture_binding() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
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
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
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
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    let draw_list = frame.ui().get_foreground_draw_list();
    unsafe {
        draw_list.add_callback(linear, std::ptr::null_mut(), 0);
        draw_list.add_callback(nearest, std::ptr::null_mut(), 0);
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
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    unsafe {
        frame
            .ui()
            .get_foreground_draw_list()
            .add_callback(user_callback, std::ptr::null_mut(), 0);
    }
    assert!(matches!(
        frame.render_snapshot(&consumer),
        Err(imgui::render::SnapshotError::UserCallbackUnsupported)
    ));
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 0);

    let snapshot = ctx
        .begin_frame()
        .render_snapshot(&consumer)
        .expect("callback rejection must not claim detached mode or allocate an epoch");
    assert_eq!(snapshot.epoch().sequence(), 1);
}

#[test]
fn abandoned_epoch_repeats_destroy_and_blocks_consumer_replacement_until_drained() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

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
        ctx.create_detached_renderer_consumer(),
        Err(imgui::render::RendererConsumerError::ConsumerDraining)
    ));
    drop(second);
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.abandoned(), 1);
    let _replacement = ctx
        .create_detached_renderer_consumer()
        .expect("a fully drained consumer should be replaceable");
}

#[test]
fn out_of_order_completion_applies_only_after_the_contiguous_gap_closes() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let first_feedback = upload_user(&first, texture_id, imgui::TextureId::new(41));
    let second = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let second_feedback = upload_user(&second, texture_id, imgui::TextureId::new(42));

    second.commit(second_feedback).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 0);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    first.commit(first_feedback).unwrap();
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
fn upload_identity_tracks_normalized_operation_content_instead_of_capture_count() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let first_identity = request_for(&first, texture_id)
        .upload_identity()
        .expect("create request has an upload identity");
    drop(first);
    ctx.poll_snapshot_completions().unwrap();

    let retry = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let retry_identity = request_for(&retry, texture_id)
        .upload_identity()
        .expect("retried create request has an upload identity");
    assert_eq!(retry_identity, first_identity);
    drop(retry);
    ctx.poll_snapshot_completions().unwrap();

    ctx.with_texture_mut(texture_id, |mut texture| {
        texture.set_data(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    })
    .unwrap();
    let changed = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let changed_identity = request_for(&changed, texture_id)
        .upload_identity()
        .expect("changed create request has an upload identity");
    assert_ne!(changed_identity, retry_identity);
}

#[test]
fn stale_upload_feedback_cannot_write_a_newer_texture_revision() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let stale = upload_user(&first, texture_id, imgui::TextureId::new(201));
    ctx.with_texture_mut(texture_id, |mut texture| {
        texture.set_data(&[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    })
    .unwrap();
    let second = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let current = upload_user(&second, texture_id, imgui::TextureId::new(202));

    first.commit(stale).unwrap();
    let stale_progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(stale_progress.watermark(), 1);
    assert_eq!(stale_progress.feedback_applied(), 0);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
        assert!(texture.texture_id().is_null());
    })
    .unwrap();

    second.commit(current).unwrap();
    let current_progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(current_progress.watermark(), 2);
    assert_eq!(current_progress.feedback_applied(), 1);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(202));
    })
    .unwrap();
}

#[test]
fn invalid_earlier_completion_does_not_block_a_later_valid_epoch() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let duplicate_a = request_for(&first, texture_id)
        .uploaded(imgui::TextureId::new(301))
        .unwrap();
    let duplicate_b = request_for(&first, texture_id)
        .uploaded(imgui::TextureId::new(302))
        .unwrap();
    let second = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let valid = upload_user(&second, texture_id, imgui::TextureId::new(303));
    let mut invalid = retry_all(&first);
    invalid.push(duplicate_a);
    invalid.push(duplicate_b);
    assert!(matches!(
        first.commit(invalid),
        Err(imgui::render::SnapshotCommitError::InvalidFeedback(
            imgui::render::RendererConsumerError::DuplicateFeedback { .. }
        ))
    ));
    second.commit(valid).unwrap();

    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.abandoned(), 1);
    ctx.with_texture(texture_id, |texture| {
        assert_eq!(texture.status(), imgui::TextureStatus::OK);
        assert_eq!(texture.texture_id(), imgui::TextureId::new(303));
    })
    .unwrap();
}

#[test]
fn out_of_order_dynamic_font_resize_keeps_every_atlas_allocation_reconcilable() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

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
    let second_feedback_count = second_feedback.len();

    second.commit(second_feedback).unwrap();
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 0);

    first.commit(first_feedback).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.committed(), 2);
    assert_eq!(progress.feedback_applied(), second_feedback_count);

    let third = ctx.begin_frame();
    third.ui().text("The replacement atlas remains renderable.");
    let third = third.render_snapshot(&consumer).unwrap();
    let feedback = retry_all(&third);
    third.commit(feedback).unwrap();
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 3);
}

#[test]
fn repeated_atlas_destroy_ack_survives_native_garbage_collection() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    ctx.prepare_frame(
        imgui::FramePrepareOptions::new([800.0, 600.0], 1.0 / 60.0)
            .framebuffer_scale([2.0, 2.0])
            .renderer_has_textures(),
    );
    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let initial = ctx.begin_frame();
    initial
        .ui()
        .text("Initial atlas allocation: the quick brown fox jumps over the lazy dog.");
    let initial = initial.render_snapshot(&consumer).unwrap();
    let initial_feedback = acknowledge_all(&initial, 5_000);
    initial.commit(initial_feedback).unwrap();
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 1);
    unsafe {
        let atlas = (*imgui::sys::igGetIO_ContextPtr(ctx.as_raw())).Fonts;
        let texture = (*atlas).TexData;
        assert_eq!((*texture).Status, imgui::sys::ImTextureStatus_OK);
        assert_ne!((*texture).TexID, 0 as imgui::sys::ImTextureID);
    }

    ctx.style_mut().set_font_size_base(96.0);
    let growth = ctx.begin_frame();
    growth
        .ui()
        .text("Resized atlas allocation: THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789.");
    drop(growth.render_snapshot(&consumer).unwrap());
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 2);

    let first = ctx.begin_frame();
    first
        .ui()
        .text("Observe the old atlas allocation's destroy request.");
    let first = first.render_snapshot(&consumer).unwrap();
    let retiring = first
        .texture_requests()
        .iter()
        .find(|request| matches!(request.operation(), imgui::render::TextureOp::Destroy))
        .unwrap_or_else(|| {
            let requests = first
                .texture_requests()
                .iter()
                .map(|request| (request.texture(), request.kind()))
                .collect::<Vec<_>>();
            panic!("font growth should retire the old atlas allocation; requests: {requests:?}")
        });
    let retiring_id = retiring.texture();
    let first_destroyed = retiring.destroyed().unwrap();

    let second = ctx.begin_frame();
    second
        .ui()
        .text("Retry the same destroy before acknowledging it.");
    let second = second.render_snapshot(&consumer).unwrap();
    let second_destroyed = second
        .texture_requests()
        .iter()
        .find(|request| {
            request.texture() == retiring_id
                && matches!(request.operation(), imgui::render::TextureOp::Destroy)
        })
        .expect("the unchanged destroy request should be retried")
        .destroyed()
        .unwrap();

    let mut first_feedback = first
        .texture_requests()
        .iter()
        .filter(|request| request.texture() != retiring_id)
        .map(imgui::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    first_feedback.push(first_destroyed);
    first.commit(first_feedback).unwrap();
    assert_eq!(ctx.poll_snapshot_completions().unwrap().watermark(), 3);

    let gc = ctx.begin_frame();
    gc.ui().text("Advance native atlas garbage collection.");
    let gc = gc.render_snapshot(&consumer).unwrap();
    assert!(
        gc.texture_requests()
            .iter()
            .all(|request| request.texture() != retiring_id),
        "the native atlas list should no longer expose the retired allocation"
    );
    drop(gc);

    let mut second_feedback = second
        .texture_requests()
        .iter()
        .filter(|request| request.texture() != retiring_id)
        .map(imgui::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    second_feedback.push(second_destroyed);
    second.commit(second_feedback).unwrap();
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 5);
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.abandoned(), 1);
}

#[test]
fn duplicate_feedback_abandons_the_epoch_without_partial_registry_mutation() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let texture_id = ctx.register_texture(owned_texture());
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let snapshot = frame.render_snapshot(&consumer).unwrap();
    let request = request_for(&snapshot, texture_id);
    let first = request.uploaded(imgui::TextureId::new(51)).unwrap();
    let duplicate = request.uploaded(imgui::TextureId::new(52)).unwrap();
    let mut invalid = retry_all(&snapshot);
    invalid.push(first);
    invalid.push(duplicate);

    assert!(matches!(
        snapshot.commit(invalid),
        Err(imgui::render::SnapshotCommitError::InvalidFeedback(
            imgui::render::RendererConsumerError::DuplicateFeedback { .. }
        ))
    ));
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 1);
    assert_eq!(progress.abandoned(), 1);
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
    let first_consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first.ui().image(texture_id, [16.0, 16.0]);
    let first = first.render_snapshot(&first_consumer).unwrap();
    let stale_feedback = request_for(&first, texture_id)
        .uploaded(imgui::TextureId::new(61))
        .unwrap();
    let first_feedback = retry_all(&first);
    first.commit(first_feedback).unwrap();
    ctx.poll_snapshot_completions().unwrap();
    drop(first_consumer);
    ctx.poll_snapshot_completions().unwrap();

    let second_consumer = ctx.create_detached_renderer_consumer().unwrap();
    let second = ctx.begin_frame();
    second.ui().image(texture_id, [16.0, 16.0]);
    let second = second.render_snapshot(&second_consumer).unwrap();
    let mut invalid = vec![stale_feedback];
    invalid.extend(retry_all(&second));
    assert!(matches!(
        second.commit(invalid),
        Err(imgui::render::SnapshotCommitError::InvalidFeedback(
            imgui::render::RendererConsumerError::StaleConsumerGeneration { .. }
        ))
    ));
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.abandoned(), 1);
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
    let consumer_a = context_a.create_detached_renderer_consumer().unwrap();
    let frame_a = context_a.begin_frame();
    frame_a.ui().image(texture_a, [16.0, 16.0]);
    let snapshot_a = frame_a.render_snapshot(&consumer_a).unwrap();
    let foreign_feedback = request_for(&snapshot_a, texture_a)
        .uploaded(imgui::TextureId::new(71))
        .unwrap();
    let feedback_a = retry_all(&snapshot_a);
    snapshot_a.commit(feedback_a).unwrap();
    context_a.poll_snapshot_completions().unwrap();
    drop(consumer_a);
    context_a.poll_snapshot_completions().unwrap();
    let suspended_a = context_a.suspend();

    let mut context_b = imgui::Context::create();
    prepare_context(&mut context_b);
    let texture_b = context_b.register_texture(owned_texture());
    let consumer_b = context_b.create_detached_renderer_consumer().unwrap();
    let frame_b = context_b.begin_frame();
    frame_b.ui().image(texture_b, [16.0, 16.0]);
    let snapshot_b = frame_b.render_snapshot(&consumer_b).unwrap();
    let mut invalid = vec![foreign_feedback];
    invalid.extend(retry_all(&snapshot_b));
    assert!(matches!(
        snapshot_b.commit(invalid),
        Err(imgui::render::SnapshotCommitError::InvalidFeedback(
            imgui::render::RendererConsumerError::ForeignContext { .. }
        ))
    ));
    let progress = context_b.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 1);
    assert_eq!(progress.abandoned(), 1);
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
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame();
    first.ui().image(first_id, [16.0, 16.0]);
    let first = first.render_snapshot(&consumer).unwrap();
    let request = request_for(&first, first_id);
    let stale_create = request.uploaded(imgui::TextureId::new(82)).unwrap();
    let create = upload_user(&first, first_id, imgui::TextureId::new(81));
    first.commit(create).unwrap();
    ctx.poll_snapshot_completions().unwrap();

    ctx.remove_texture(first_id).unwrap();
    let destroy = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let destroyed = destroy_user(&destroy, first_id);
    destroy.commit(destroyed).unwrap();
    ctx.poll_snapshot_completions().unwrap();

    let replacement_id = ctx.register_texture(owned_texture());
    assert_ne!(replacement_id, first_id);
    let replacement = ctx.begin_frame();
    replacement.ui().image(replacement_id, [16.0, 16.0]);
    let replacement = replacement.render_snapshot(&consumer).unwrap();
    let mut invalid = vec![stale_create];
    invalid.extend(retry_all(&replacement));
    assert!(matches!(
        replacement.commit(invalid),
        Err(imgui::render::SnapshotCommitError::InvalidFeedback(
            imgui::render::RendererConsumerError::FeedbackNotRequested { .. }
        ))
    ));
    let progress = ctx.poll_snapshot_completions().unwrap();
    assert_eq!(progress.abandoned(), 1);
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
    let consumer = ctx.create_detached_renderer_consumer().unwrap();

    let first = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let created = upload_user(&first, texture_id, imgui::TextureId::new(71));
    ctx.remove_texture(texture_id).unwrap();
    first.commit(created).unwrap();
    ctx.poll_snapshot_completions().unwrap();
    assert_eq!(
        ctx.with_texture(texture_id, |_| ()),
        Err(imgui::ManagedTextureError::Retiring(texture_id))
    );

    let destroy = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let destroyed = destroy_user(&destroy, texture_id);
    destroy.commit(destroyed).unwrap();
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
    let consumer_a = context_a.create_detached_renderer_consumer().unwrap();
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
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
    let snapshot = ctx.begin_frame().render_snapshot(&consumer).unwrap();
    let feedback = retry_all(&snapshot);
    drop(consumer);
    drop(ctx);
    assert_eq!(
        snapshot.commit(feedback),
        Err(imgui::render::SnapshotCommitError::ContextDropped)
    );
}

#[test]
fn renderer_reset_rejects_an_outstanding_detached_epoch() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
    let snapshot = ctx.begin_frame().render_snapshot(&consumer).unwrap();

    assert!(matches!(
        ctx.prepare_renderer_texture_reset(&consumer),
        Err(imgui::render::RendererConsumerError::OutstandingEpochs { count: 1 })
    ));
    drop(snapshot);
    ctx.poll_snapshot_completions().unwrap();
    let reset = ctx.prepare_renderer_texture_reset(&consumer).unwrap();
    reset.commit();
}
