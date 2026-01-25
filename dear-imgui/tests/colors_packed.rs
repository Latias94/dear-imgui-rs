use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn color_packing_helpers_no_panic() {
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

    let _ = ui.style_color(imgui::StyleColor::Text);

    let c0 = ui.get_color_u32(imgui::StyleColor::Text);
    let _ = ui.get_color_u32_with_alpha(imgui::StyleColor::Text, 0.5);
    let _ = ui.get_color_u32_from_rgba([1.0, 0.0, 0.0, 1.0]);
    let _ = ui.get_color_u32_from_packed(c0, 0.25);

    let color = imgui::Color::rgb(0.2, 0.4, 0.6).with_alpha(0.8);
    let packed = color.to_imgui_u32();
    let roundtrip = imgui::Color::from_imgui_u32(packed);
    let _ = roundtrip.to_imgui_u32();
}
