use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn checkbox_flags_api_exists_and_no_panic() {
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
    let mut flags_u32: u32 = 0b0101;
    let mut flags_i32: i32 = 0b0101;

    let _ = ui.window("Flags").build(|| {
        let _ = ui.checkbox_flags("u32 mask", &mut flags_u32, 0b0001);
        let _ = ui.checkbox_flags("i32 mask", &mut flags_i32, 0b0100);
    });
}
