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
    assert!(ctx.draw_data().is_none());

    prepare_context(&mut ctx);
    let frame = ctx.begin_frame();
    frame.ui().text("next frame still starts cleanly");
    let draw_data = frame.render();
    assert!(draw_data.valid());
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
fn frame_token_rejects_font_managed_draws_without_a_snapshot_consumer() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let frame = ctx.begin_frame();
    frame.ui().text("snapshot me");

    let error = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect_err("managed font draws require Context-owned snapshot capture");
    assert!(matches!(
        error,
        imgui::render::snapshot::SnapshotError::ManagedTextureRequiresContext
    ));
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
fn frame_token_snapshot_requires_a_managed_texture_consumer() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    let texture_id = ctx.register_texture(texture);

    let frame = ctx.begin_frame();
    frame.ui().image(texture_id, [16.0, 16.0]);
    let error = frame
        .render_snapshot(imgui::render::snapshot::SnapshotOptions::default())
        .expect_err("managed snapshot capture must use the U3 consumer contract");
    assert!(matches!(
        error,
        imgui::render::snapshot::SnapshotError::ManagedTextureRequiresContext
    ));
}
