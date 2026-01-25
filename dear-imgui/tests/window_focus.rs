use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn set_window_focus_accepts_none_and_name_no_panic() {
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
        ui.text("Hello");
        let _ = ui.is_window_focused();
        let _ = ui.is_window_focused_with_flags(imgui::FocusedFlags::ROOT_WINDOW);
        let _ = ui.is_item_focused();
        let _ = ui.is_any_item_focused();
    });

    let _ = ui
        .window("B")
        .size_constraints([100.0, 80.0], [400.0, 300.0])
        .scroll([0.0, 10.0])
        .build(|| ui.text("Constrained"));

    ui.set_window_focus(Some("A"));
    ui.set_window_focus(None);
}
