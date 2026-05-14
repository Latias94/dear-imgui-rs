use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);

    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

macro_rules! assert_panics {
    ($body:block) => {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err());
    };
}

#[test]
fn image_widgets_reject_invalid_geometry_and_colors_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let tex = imgui::TextureId::new(42);

    let _ = ui.window("image widget boundaries").build(|| {
        ui.image_config(tex, [0.0, 0.0])
            .uv0([-1.0, 0.0])
            .uv1([2.0, 1.0])
            .tint_color([0.5, 0.5, 0.5, 1.0])
            .border_color([1.0, 0.0, 0.0, 1.0])
            .build();

        assert_panics!({
            ui.image(tex, [-1.0, 8.0]);
        });
        assert_panics!({
            ui.image_config(tex, [8.0, 8.0])
                .uv0([f32::NAN, 0.0])
                .build();
        });
        assert_panics!({
            ui.image_config(tex, [8.0, 8.0])
                .tint_color([1.0, f32::INFINITY, 1.0, 1.0])
                .build();
        });
        assert_panics!({
            ui.image_config(tex, [8.0, 8.0])
                .border_color([1.0, 0.0, 0.0, f32::NAN])
                .build();
        });

        assert_panics!({
            ui.image_config(tex, [8.0, 8.0])
                .build_with_bg([0.0, 0.0, 0.0, f32::NAN], [1.0, 1.0, 1.0, 1.0]);
        });
        assert_panics!({
            ui.image_config(tex, [8.0, 8.0])
                .build_with_bg([0.0, 0.0, 0.0, 0.0], [1.0, f32::NAN, 1.0, 1.0]);
        });
    });
}

#[test]
fn image_button_rejects_invalid_geometry_and_colors_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let tex = imgui::TextureId::new(43);

    let _ = ui.window("image button boundaries").build(|| {
        let _ = ui
            .image_button_config("valid image button", tex, [0.0, 0.0])
            .uv0([-1.0, 0.0])
            .uv1([2.0, 1.0])
            .bg_color([0.0, 0.0, 0.0, 0.25])
            .tint_color([1.0, 1.0, 1.0, 1.0])
            .build();

        assert_panics!({
            let _ = ui.image_button("negative image button size", tex, [8.0, -1.0]);
        });
        assert_panics!({
            let _ = ui
                .image_button_config("bad image button uv", tex, [8.0, 8.0])
                .uv1([1.0, f32::INFINITY])
                .build();
        });
        assert_panics!({
            let _ = ui
                .image_button_config("bad image button bg", tex, [8.0, 8.0])
                .bg_color([0.0, f32::NAN, 0.0, 1.0])
                .build();
        });
        assert_panics!({
            let _ = ui
                .image_button_config("bad image button tint", tex, [8.0, 8.0])
                .tint_color([1.0, 1.0, f32::INFINITY, 1.0])
                .build();
        });
    });
}
