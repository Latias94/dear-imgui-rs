use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn tab_and_color_options_helpers_no_panic() {
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

    ui.set_color_edit_options(
        imgui::ColorEditFlags::UINT8
            | imgui::ColorEditFlags::DISPLAY_HSV
            | imgui::ColorEditFlags::PICKER_HUE_BAR,
    );

    let _ = ui.tab_bar("Tabs").map(|_tab| {
        let _ = ui.tab_item("A");
        let _ = ui.tab_item_button("+");
        let _ = ui.tab_item_button_with_flags("+##fit", imgui::TabItemFlags::TRAILING);
        ui.set_tab_item_closed("A");
    });
}
