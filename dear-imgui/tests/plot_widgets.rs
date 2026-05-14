use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn plot_widgets_validate_value_offsets() {
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
    let values = [0.0f32, 1.0, 0.5];
    let empty: [f32; 0] = [];

    let _ = ui.window("Plots").build(|| {
        ui.plot_lines("empty_lines", &empty);
        ui.plot_histogram("empty_histogram", &empty);
        ui.plot_lines_config("offset_lines", &values)
            .values_offset(2)
            .build();
        ui.plot_histogram_config("offset_histogram", &values)
            .values_offset(2)
            .build();

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.plot_lines_config("negative_lines", &values)
                    .values_offset(-1)
                    .build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.plot_histogram_config("negative_histogram", &values)
                    .values_offset(-1)
                    .build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.plot_lines_config("too_large_lines", &values)
                    .values_offset(values.len() as i32)
                    .build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.plot_histogram_config("too_large_histogram", &values)
                    .values_offset(values.len() as i32)
                    .build();
            }))
            .is_err()
        );
    });
}
