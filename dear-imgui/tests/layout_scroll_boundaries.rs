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
fn layout_and_scroll_helpers_reject_non_finite_or_invalid_ratios_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_item_width(f32::NAN);
        }))
        .is_err()
    );

    let _ = ui.window("layout boundaries").build(|| {
        ui.same_line_with_spacing(0.0, -1.0);
        ui.dummy([1.0, 1.0]);
        ui.indent_by(0.0);
        ui.unindent_by(0.0);
        ui.set_cursor_pos([0.0, 0.0]);
        ui.set_cursor_screen_pos([0.0, 0.0]);
        ui.set_cursor_pos_x(0.0);
        ui.set_cursor_pos_y(0.0);
        ui.push_clip_rect([0.0, 0.0], [1.0, 1.0], true);
        ui.pop_clip_rect();
        let _ = ui.is_rect_visible_min_max([0.0, 0.0], [1.0, 1.0]);
        let _ = ui.is_rect_visible_with_size([1.0, 1.0]);

        ui.set_scroll_x(0.0);
        ui.set_scroll_y(0.0);
        ui.set_scroll_from_pos_x(0.0, 0.5);
        ui.set_scroll_from_pos_y(0.0, 0.5);
        ui.set_scroll_here_x(0.5);
        ui.set_scroll_here_y(0.5);

        for panics in [
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.same_line_with_spacing(f32::NAN, -1.0);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.dummy([1.0, f32::INFINITY]);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.indent_by(f32::NAN);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.set_cursor_pos([f32::NAN, 0.0]);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.push_clip_rect([0.0, 0.0], [f32::INFINITY, 1.0], true);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_rect_visible_min_max([0.0, 0.0], [f32::NAN, 1.0]);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.set_scroll_x(f32::NAN);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.set_scroll_from_pos_x(0.0, 1.5);
            })),
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.set_scroll_here_y(f32::NAN);
            })),
        ] {
            assert!(panics.is_err());
        }
    });
}
