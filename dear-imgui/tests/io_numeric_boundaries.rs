use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

macro_rules! assert_panics {
    ($body:block) => {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err());
    };
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);

    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn io_frame_inputs_reject_invalid_values_before_storing() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let io = ctx.io_mut();

    io.set_display_size([0.0, 0.0]);
    io.set_delta_time(0.0);
    io.set_delta_time(1.0 / 60.0);
    io.set_display_framebuffer_scale([0.0, 2.0]);

    assert_eq!(io.display_size(), [0.0, 0.0]);
    assert_eq!(io.delta_time(), 1.0 / 60.0);
    assert_eq!(io.display_framebuffer_scale(), [0.0, 2.0]);

    assert_panics!({
        io.set_display_size([-1.0, 1.0]);
    });
    assert_eq!(io.display_size(), [0.0, 0.0]);

    assert_panics!({
        io.set_display_size([f32::NAN, 1.0]);
    });
    assert_eq!(io.display_size(), [0.0, 0.0]);

    assert_panics!({
        io.set_delta_time(f32::INFINITY);
    });
    assert_eq!(io.delta_time(), 1.0 / 60.0);

    assert_panics!({
        io.set_display_framebuffer_scale([1.0, -0.1]);
    });
    assert_eq!(io.display_framebuffer_scale(), [0.0, 2.0]);

    assert_panics!({
        io.set_display_framebuffer_scale([1.0, f32::NAN]);
    });
    assert_eq!(io.display_framebuffer_scale(), [0.0, 2.0]);
}

#[test]
fn io_delta_time_must_be_positive_after_first_frame() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let _ = ctx.frame();
    ctx.render();

    let io = ctx.io_mut();
    let previous = io.delta_time();

    io.set_delta_time(0.001);
    assert_eq!(io.delta_time(), 0.001);

    assert_panics!({
        io.set_delta_time(0.0);
    });
    assert_eq!(io.delta_time(), 0.001);

    assert_panics!({
        io.set_delta_time(-0.1);
    });
    assert_eq!(io.delta_time(), 0.001);

    io.set_delta_time(previous);
}

#[test]
fn io_mouse_inputs_allow_finite_sentinel_values_but_reject_non_finite() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let io = ctx.io_mut();

    io.set_mouse_pos([-f32::MAX, -f32::MAX]);
    io.set_mouse_wheel(-1.25);
    io.set_mouse_wheel_h(0.5);
    io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
    io.add_mouse_wheel_event([0.25, -0.75]);

    assert_eq!(io.mouse_pos(), [-f32::MAX, -f32::MAX]);
    assert_eq!(io.mouse_wheel(), -1.25);
    assert_eq!(io.mouse_wheel_h(), 0.5);

    assert_panics!({
        io.set_mouse_pos([f32::NAN, 0.0]);
    });
    assert_eq!(io.mouse_pos(), [-f32::MAX, -f32::MAX]);

    assert_panics!({
        io.set_mouse_wheel(f32::INFINITY);
    });
    assert_eq!(io.mouse_wheel(), -1.25);

    assert_panics!({
        io.set_mouse_wheel_h(f32::NAN);
    });
    assert_eq!(io.mouse_wheel_h(), 0.5);

    assert_panics!({
        io.add_mouse_pos_event([0.0, f32::NAN]);
    });

    assert_panics!({
        io.add_mouse_wheel_event([f32::INFINITY, 0.0]);
    });
}

#[test]
fn io_mouse_button_state_uses_typed_buttons_and_preserves_raw_index_escape_hatch() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let io = ctx.io_mut();

    for button in [
        imgui::MouseButton::Left,
        imgui::MouseButton::Right,
        imgui::MouseButton::Middle,
        imgui::MouseButton::Extra1,
        imgui::MouseButton::Extra2,
    ] {
        assert!(!io.mouse_down(button));
        io.set_mouse_down(button, true);
        assert!(io.mouse_down(button));
        assert!(io.mouse_down_raw_index(button as i32 as usize));
        io.set_mouse_down(button, false);
        assert!(!io.mouse_down(button));
    }

    io.set_mouse_down_raw_index(3, true);
    assert!(io.mouse_down(imgui::MouseButton::Extra1));
    io.set_mouse_down_raw_index(3, false);
    assert!(!io.mouse_down(imgui::MouseButton::Extra1));

    assert!(!io.mouse_down_raw_index(5));
    io.set_mouse_down_raw_index(5, true);
    assert!(!io.mouse_down_raw_index(5));
}

#[test]
fn io_config_floats_reject_invalid_values_before_storing() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let io = ctx.io_mut();

    io.set_ini_saving_rate(0.0);
    io.set_config_memory_compact_timer(-1.0);
    io.set_mouse_double_click_time(0.0);
    io.set_mouse_double_click_max_dist(0.0);
    io.set_mouse_drag_threshold(0.0);
    io.set_key_repeat_delay(0.0);
    io.set_key_repeat_rate(0.0);

    assert_eq!(io.ini_saving_rate(), 0.0);
    assert_eq!(io.config_memory_compact_timer(), -1.0);
    assert_eq!(io.mouse_double_click_time(), 0.0);
    assert_eq!(io.mouse_double_click_max_dist(), 0.0);
    assert_eq!(io.mouse_drag_threshold(), 0.0);
    assert_eq!(io.key_repeat_delay(), 0.0);
    assert_eq!(io.key_repeat_rate(), 0.0);

    assert_panics!({
        io.set_ini_saving_rate(-0.1);
    });
    assert_eq!(io.ini_saving_rate(), 0.0);

    assert_panics!({
        io.set_config_memory_compact_timer(-2.0);
    });
    assert_eq!(io.config_memory_compact_timer(), -1.0);

    assert_panics!({
        io.set_mouse_double_click_time(f32::NAN);
    });
    assert_eq!(io.mouse_double_click_time(), 0.0);

    assert_panics!({
        io.set_mouse_double_click_max_dist(-0.1);
    });
    assert_eq!(io.mouse_double_click_max_dist(), 0.0);

    assert_panics!({
        io.set_mouse_drag_threshold(f32::INFINITY);
    });
    assert_eq!(io.mouse_drag_threshold(), 0.0);

    assert_panics!({
        io.set_key_repeat_delay(-0.1);
    });
    assert_eq!(io.key_repeat_delay(), 0.0);

    assert_panics!({
        io.set_key_repeat_rate(f32::NAN);
    });
    assert_eq!(io.key_repeat_rate(), 0.0);
}

#[test]
fn io_font_global_scale_rejects_non_positive_values_before_storing() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let io = ctx.io_mut();

    io.set_font_global_scale(1.5);
    assert_eq!(io.font_global_scale(), 1.5);

    assert_panics!({
        io.set_font_global_scale(0.0);
    });
    assert_eq!(io.font_global_scale(), 1.5);

    assert_panics!({
        io.set_font_global_scale(f32::NAN);
    });
    assert_eq!(io.font_global_scale(), 1.5);
}
