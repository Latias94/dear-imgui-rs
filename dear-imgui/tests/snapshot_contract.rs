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
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn snapshot_preserves_draw_metadata_and_legacy_texture_binding() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let frame = ctx.begin_frame();
    {
        let ui = frame.ui();
        let draw_list = ui.get_foreground_draw_list();
        draw_list.add_image(
            imgui::TextureId::new(77),
            [0.0, 0.0],
            [32.0, 16.0],
            [0.0, 0.0],
            [1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect("snapshot should capture draw commands");

    assert_eq!(snapshot.draw.display_size, [640.0, 480.0]);
    assert_eq!(snapshot.draw.framebuffer_scale, [1.25, 1.5]);
    assert!(snapshot.draw.draw_lists.iter().any(|list| {
        !list.vtx.is_empty()
            && !list.idx.is_empty()
            && list.commands.iter().any(|cmd| {
                matches!(
                    cmd,
                    imgui::render::snapshot::DrawCmdSnapshot::Elements {
                        texture: imgui::render::snapshot::TextureBinding::Legacy(id),
                        count,
                        ..
                    } if *id == imgui::TextureId::new(77) && *count > 0
                )
            })
    }));
}

#[test]
fn snapshot_preserves_managed_texture_bindings_and_requests() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let mut texture = imgui::texture::TextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 2, 2);
    texture.set_data(&[
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ]);
    texture.set_status(imgui::texture::TextureStatus::WantCreate);
    let texture_id = texture.unique_id();
    ctx.register_user_texture(&mut texture);

    let frame = ctx.begin_frame();
    frame.ui().image(&mut *texture, [24.0, 24.0]);
    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect("snapshot should include managed texture binding and request");

    assert!(snapshot.draw.draw_lists.iter().any(|list| {
        list.commands.iter().any(|cmd| {
            matches!(
                cmd,
                imgui::render::snapshot::DrawCmdSnapshot::Elements {
                    texture: imgui::render::snapshot::TextureBinding::Managed(id),
                    ..
                } if *id == texture_id
            )
        })
    }));

    let request = snapshot
        .texture_requests
        .iter()
        .find(|request| request.id == texture_id)
        .expect("managed texture request should be captured");
    match &request.op {
        imgui::render::snapshot::TextureOp::Create {
            format,
            width,
            height,
            row_pitch,
            pixels,
        } => {
            assert_eq!(*format, imgui::texture::TextureFormat::RGBA32);
            assert_eq!((*width, *height), (2, 2));
            assert_eq!(*row_pitch, 8);
            assert_eq!(pixels.len(), 16);
        }
        other => panic!("expected create request, got {other:?}"),
    }
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
    {
        let platform_io = ctx.platform_io_mut();
        platform_io.set_draw_callback_set_sampler_linear_raw(Some(linear));
        platform_io.set_draw_callback_set_sampler_nearest_raw(Some(nearest));
    }

    let frame = ctx.begin_frame();
    {
        let draw_list = frame.ui().get_foreground_draw_list();
        unsafe {
            draw_list.add_callback(Some(linear), std::ptr::null_mut(), 0);
            draw_list.add_callback(Some(nearest), std::ptr::null_mut(), 0);
        }
    }

    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect("snapshot should preserve standard sampler callbacks");

    assert!(snapshot.draw.draw_lists.iter().any(|list| {
        list.commands.iter().any(|cmd| {
            matches!(
                cmd,
                imgui::render::snapshot::DrawCmdSnapshot::SetSamplerLinear
            )
        })
    }));
    assert!(snapshot.draw.draw_lists.iter().any(|list| {
        list.commands.iter().any(|cmd| {
            matches!(
                cmd,
                imgui::render::snapshot::DrawCmdSnapshot::SetSamplerNearest
            )
        })
    }));
}

#[test]
fn snapshot_can_disable_texture_request_capture_for_pure_draw_handoff() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let mut texture = imgui::texture::TextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    texture.set_status(imgui::texture::TextureStatus::WantCreate);
    ctx.register_user_texture(&mut texture);

    let frame = ctx.begin_frame();
    frame.ui().image(&mut *texture, [8.0, 8.0]);
    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions {
            capture_texture_requests: false,
            ..Default::default()
        })
        .expect("draw-only snapshot should still succeed");

    assert!(snapshot.texture_requests.is_empty());
    assert!(snapshot.draw.draw_lists.iter().any(|list| {
        list.commands.iter().any(|cmd| {
            matches!(
                cmd,
                imgui::render::snapshot::DrawCmdSnapshot::Elements {
                    texture: imgui::render::snapshot::TextureBinding::Managed(_),
                    ..
                }
            )
        })
    }));
}

#[test]
fn snapshot_captures_texture_update_rects_as_tight_uploads() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let mut texture = imgui::texture::TextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 4, 4);
    texture.set_data(&[
        0, 0, 0, 255, 1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 0, 1, 0, 255, 1, 1, 0, 255, 2, 1,
        0, 255, 3, 1, 0, 255, 0, 2, 0, 255, 1, 2, 0, 255, 2, 2, 0, 255, 3, 2, 0, 255, 0, 3, 0, 255,
        1, 3, 0, 255, 2, 3, 0, 255, 3, 3, 0, 255,
    ]);
    texture.set_status(imgui::texture::TextureStatus::WantUpdates);
    let texture_id = texture.unique_id();
    ctx.register_user_texture(&mut texture);

    let frame = ctx.begin_frame();
    frame.ui().image(&mut *texture, [16.0, 16.0]);
    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect("snapshot should include managed texture update request");

    let request = snapshot
        .texture_requests
        .iter()
        .find(|request| request.id == texture_id)
        .expect("managed texture update request should be captured");
    match &request.op {
        imgui::render::snapshot::TextureOp::Update {
            format,
            width,
            height,
            rects,
        } => {
            assert_eq!(*format, imgui::texture::TextureFormat::RGBA32);
            assert_eq!((*width, *height), (4, 4));
            assert_eq!(rects.len(), 1);
            assert_eq!(
                rects[0].rect,
                imgui::texture::TextureRect {
                    x: 0,
                    y: 0,
                    w: 4,
                    h: 4,
                }
            );
            assert_eq!(rects[0].row_pitch, 16);
            assert_eq!(rects[0].data.len(), 64);
        }
        other => panic!("expected update request, got {other:?}"),
    }
}
