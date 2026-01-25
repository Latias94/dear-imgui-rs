use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn log_and_popup_helpers_no_panic() {
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

    ui.log_to_tty(-1);
    ui.log_to_clipboard(-1);
    ui.log_buttons();
    ui.log_finish();

    let log_path =
        std::env::temp_dir().join(format!("dear-imgui-rs-test-log-{}.txt", std::process::id()));
    let _ = ui.log_to_file(-1, &log_path);
    ui.log_finish();
    let _ = std::fs::remove_file(log_path);

    let _ = ui.window("Popup").build(|| {
        let _ = ui.button("Item");
        ui.open_popup_on_item_click(None);
        let _ = ui.begin_popup_context_item();
    });
}
