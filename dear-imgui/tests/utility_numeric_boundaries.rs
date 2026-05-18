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

#[test]
fn utility_float_inputs_reject_non_finite_values_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();

    let _ = ui.get_color_u32_with_alpha(imgui::StyleColor::Text, 0.5);
    let _ = ui.get_color_u32_from_rgba([1.5, -1.0, 0.0, 1.0]);
    let _ = ui.get_color_u32_from_packed(0xFFFF_FFFF, 0.25);
    let _: usize = ui.get_key_pressed_amount(imgui::Key::A, 0.25, 0.05);
    let _: usize = ui.get_mouse_clicked_count(imgui::MouseButton::Left);
    let _: usize = ui.frame_count();
    let _ = ui.get_mouse_drag_delta(imgui::MouseButton::Left, -1.0);

    macro_rules! assert_panics {
        ($body:block) => {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err());
        };
    }

    assert_panics!({
        let _ = ui.get_color_u32_with_alpha(imgui::StyleColor::Text, f32::NAN);
    });
    assert_panics!({
        let _ = ui.get_color_u32_from_rgba([1.0, f32::NAN, 0.0, 1.0]);
    });
    assert_panics!({
        let _ = ui.get_color_u32_from_packed(0xFFFF_FFFF, f32::NAN);
    });
    assert_panics!({
        let _ = ui.get_key_pressed_amount(imgui::Key::A, f32::NAN, 0.05);
    });
    assert_panics!({
        let _ = ui.get_key_pressed_amount(imgui::Key::A, 0.25, f32::INFINITY);
    });
    assert_panics!({
        let _ = ui.get_mouse_drag_delta(imgui::MouseButton::Left, f32::NAN);
    });

    let _ = ui.window("utility numeric boundaries").build(|| {
        let _ = ui.is_rect_visible([10.0, 10.0]);
        let _ = ui.is_rect_visible_ex([0.0, 0.0], [10.0, 10.0]);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_rect_visible([f32::NAN, 10.0]);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_rect_visible_ex([0.0, 0.0], [f32::INFINITY, 10.0]);
            }))
            .is_err()
        );
    });
}
