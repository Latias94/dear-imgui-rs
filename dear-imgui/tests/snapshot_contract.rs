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
fn arbitrary_draw_data_snapshot_rejects_context_owned_managed_textures() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 2, 2);
    texture.set_data(&[
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ]);
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [24.0, 24.0]);
    let error = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect_err("managed capture must enter through a Context-owned consumer");
    assert!(matches!(
        error,
        imgui::render::snapshot::SnapshotError::ManagedTextureRequiresContext
    ));
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
fn disabling_request_capture_cannot_bypass_managed_texture_ownership() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [8.0, 8.0]);
    let error = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions {
            capture_texture_requests: false,
            ..Default::default()
        })
        .expect_err("managed draw bindings cannot be detached without a consumer");
    assert!(matches!(
        error,
        imgui::render::snapshot::SnapshotError::ManagedTextureRequiresContext
    ));
}

#[test]
fn managed_mutation_remains_context_scoped_before_snapshot_capture() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 4, 4);
    texture.set_data(&[
        0, 0, 0, 255, 1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 0, 1, 0, 255, 1, 1, 0, 255, 2, 1,
        0, 255, 3, 1, 0, 255, 0, 2, 0, 255, 1, 2, 0, 255, 2, 2, 0, 255, 3, 2, 0, 255, 0, 3, 0, 255,
        1, 3, 0, 255, 2, 3, 0, 255, 3, 3, 0, 255,
    ]);
    let texture_id = ctx.register_texture(texture);
    ctx.with_texture_mut(texture_id, |texture| {
        texture.set_data(&[7; 64]);
    })
    .expect("owner Context should mutate an active texture");

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let error = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect_err("U3 Context-owned capture is required for managed requests");
    assert!(matches!(
        error,
        imgui::render::snapshot::SnapshotError::ManagedTextureRequiresContext
    ));
}
