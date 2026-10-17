use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn unified_draw_flags_match_imgui() {
    assert_eq!(
        imgui::DrawFlags::NONE.bits(),
        imgui::sys::ImDrawFlags_None as u32
    );
    assert_eq!(
        imgui::DrawFlags::CLOSED.bits(),
        imgui::sys::ImDrawFlags_Closed as u32
    );

    assert_eq!(
        imgui::DrawFlags::ROUND_ALL.bits(),
        imgui::sys::ImDrawFlags_RoundAll as u32
    );
    assert_eq!(
        imgui::DrawFlags::ROUND_NONE.bits(),
        imgui::sys::ImDrawFlags_RoundNone as u32
    );
    assert_eq!(
        (imgui::DrawFlags::ROUND_TOP_LEFT | imgui::DrawFlags::ROUND_BOTTOM_RIGHT).bits(),
        (imgui::sys::ImDrawFlags_RoundTopLeft | imgui::sys::ImDrawFlags_RoundBottomRight) as u32
    );
}

#[test]
fn unified_draw_flags_cover_shapes_and_paths() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();
    let draw_list = ui.get_window_draw_list();

    draw_list
        .add_rect([8.0, 8.0], [48.0, 48.0], imgui::Color::WHITE)
        .rounding(4.0)
        .flags(imgui::DrawFlags::ROUND_TOP_LEFT | imgui::DrawFlags::ROUND_BOTTOM_RIGHT)
        .build();

    draw_list
        .add_polyline(
            vec![[60.0, 8.0], [90.0, 48.0], [30.0, 48.0]],
            imgui::Color::WHITE,
        )
        .flags(imgui::DrawFlags::CLOSED)
        .build();

    draw_list.path_clear();
    draw_list.path_line_to([110.0, 8.0]);
    draw_list.path_line_to([140.0, 48.0]);
    draw_list.path_line_to([80.0, 48.0]);
    draw_list.path_stroke(imgui::Color::WHITE, 1.0, imgui::DrawFlags::CLOSED);

    draw_list.path_rect(
        [150.0, 8.0],
        [190.0, 48.0],
        4.0,
        imgui::DrawFlags::ROUND_NONE,
    );
    draw_list.path_stroke(imgui::Color::WHITE, 1.0, imgui::DrawFlags::CLOSED);
}
