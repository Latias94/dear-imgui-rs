use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn shortcut_routing_apis_no_panic() {
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

    let chord = imgui::KeyChord::new(imgui::Key::S).with_mods(imgui::KeyMods::CTRL);
    let _ = ui.is_key_chord_pressed(chord);
    let _ = ui.shortcut(chord);
    let _ = ui.shortcut_with_flags(chord, imgui::InputFlags::ROUTE_GLOBAL);

    ui.set_next_item_shortcut(chord);
    let _ = ui.button("Save");
}
