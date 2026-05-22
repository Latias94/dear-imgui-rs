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
    let _ = ctx.font_atlas_mut().build();
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

    let frame = ctx.begin_frame();
    assert_eq!(frame.lifecycle_state(), imgui::FrameLifecycleState::InFrame);
    frame.ui().text("first system");
    frame.ui().text("second system");

    let draw_data = frame.render();
    assert!(draw_data.valid());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
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
fn frame_token_can_capture_thread_safe_snapshot_on_end() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let frame = ctx.begin_frame();
    frame.ui().text("snapshot me");

    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect("snapshot should be created from a rendered frame");

    assert!(snapshot.draw.display_size[0] > 0.0);
    assert!(snapshot.draw.display_size[1] > 0.0);
}

#[test]
fn frame_with_result_lets_engines_run_multiple_ui_steps_before_rendering() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let result = ctx.frame_with_result(|ui| {
        ui.text("system A");
        ui.text("system B");
        42usize
    });

    assert_eq!(result.value, 42);
    assert!(result.draw_data.valid());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn frame_token_snapshot_reports_managed_texture_requests() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let mut texture = imgui::texture::TextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    texture.set_status(imgui::texture::TextureStatus::WantCreate);
    let texture_id = texture.unique_id();
    ctx.register_user_texture(&mut texture);

    let frame = ctx.begin_frame();
    frame.ui().image(&mut *texture, [16.0, 16.0]);
    let snapshot = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect("snapshot should include managed texture requests");

    assert!(snapshot.texture_requests.iter().any(|request| {
        request.id == texture_id
            && matches!(
                request.op,
                imgui::render::snapshot::TextureOp::Create {
                    width: 1,
                    height: 1,
                    ..
                }
            )
    }));
}
