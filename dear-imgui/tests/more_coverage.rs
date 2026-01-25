use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn more_coverage_queries_no_panic() {
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
    let _ = ui.is_mouse_pos_valid();
    let _ = ui.is_mouse_pos_valid_at([0.0, 0.0]);
    let _ = ui.is_mouse_released_with_delay(imgui::MouseButton::Left, 0.0);
    let _ = ui.calc_item_width();

    let main_id = imgui::Id::from(ui.main_viewport().id());
    let _ = ui.find_viewport_by_id(main_id);
    let _ = ui.find_viewport_by_platform_handle(std::ptr::null_mut());
}
