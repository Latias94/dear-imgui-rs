use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn window_runtime_control_helpers_no_panic() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();

    let _ = ui.window("A").build(|| {
        ui.set_window_pos_with_cond([123.0, 45.0], imgui::Condition::Always);
        ui.set_window_size_with_cond([320.0, 240.0], imgui::Condition::Always);
        ui.set_window_collapsed_with_cond(false, imgui::Condition::Always);

        let _ = ui.window_pos();
        let _ = ui.window_size();
        let _ = ui.is_window_collapsed();
    });

    ui.set_window_pos_by_name_with_cond("A", [200.0, 80.0], imgui::Condition::Always);
    ui.set_window_size_by_name_with_cond("A", [400.0, 300.0], imgui::Condition::Always);
    ui.set_window_collapsed_by_name_with_cond("A", true, imgui::Condition::Always);
}

#[test]
fn window_runtime_control_rejects_non_finite_geometry_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();

    let _ = ui.window("A").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.set_window_pos_with_cond([f32::NAN, 45.0], imgui::Condition::Always);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.set_window_size_with_cond([320.0, f32::INFINITY], imgui::Condition::Always);
            }))
            .is_err()
        );
    });

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_window_pos_by_name_with_cond("A", [f32::NAN, 80.0], imgui::Condition::Always);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_window_size_by_name_with_cond(
                "A",
                [400.0, f32::INFINITY],
                imgui::Condition::Always,
            );
        }))
        .is_err()
    );
}
