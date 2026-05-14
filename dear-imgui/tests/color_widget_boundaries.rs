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
fn color_widgets_reject_non_finite_colors_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("color widget boundaries").build(|| {
        let mut edit3_color = [0.1, f32::NAN, 0.3];
        assert_panics!({
            let _ = ui.color_edit3("bad edit3", &mut edit3_color);
        });
        assert_eq!(edit3_color[1].to_bits(), f32::NAN.to_bits());

        let mut edit4_color = [0.1, 0.2, f32::INFINITY, 0.4];
        assert_panics!({
            let _ = ui.color_edit4("bad edit4", &mut edit4_color);
        });
        assert!(edit4_color[2].is_infinite());

        let mut picker3_color = [0.1, f32::NEG_INFINITY, 0.3];
        assert_panics!({
            let _ = ui.color_picker3("bad picker3", &mut picker3_color);
        });
        assert!(picker3_color[1].is_infinite());

        let mut picker4_color = [0.1, 0.2, 0.3, 0.4];
        assert_panics!({
            let _ = ui
                .color_picker4_config("bad reference", &mut picker4_color)
                .reference_color([0.1, 0.2, f32::NAN, 1.0])
                .build();
        });
        assert_eq!(picker4_color, [0.1, 0.2, 0.3, 0.4]);

        assert_panics!({
            let _ = ui.color_button("bad button color", [0.1, 0.2, f32::NAN, 1.0]);
        });
    });
}

#[test]
fn color_button_rejects_non_finite_or_negative_size_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("color button size boundaries").build(|| {
        let _ = ui
            .color_button_config("auto size", [0.1, 0.2, 0.3, 1.0])
            .size([0.0, 0.0])
            .build();

        assert_panics!({
            let _ = ui
                .color_button_config("negative size", [0.1, 0.2, 0.3, 1.0])
                .size([-1.0, 1.0])
                .build();
        });

        assert_panics!({
            let _ = ui
                .color_button_config("nan size", [0.1, 0.2, 0.3, 1.0])
                .size([1.0, f32::NAN])
                .build();
        });
    });
}
