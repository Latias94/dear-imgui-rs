use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn draw_flag_types_match_supported_imgui_subsets() {
    assert_eq!(
        imgui::PolylineFlags::NONE.bits(),
        imgui::sys::ImDrawFlags_None as u32
    );
    assert_eq!(
        imgui::PolylineFlags::CLOSED.bits(),
        imgui::sys::ImDrawFlags_Closed as u32
    );

    assert_eq!(
        imgui::DrawCornerFlags::DEFAULT.bits(),
        imgui::sys::ImDrawFlags_None as u32
    );
    assert_eq!(
        imgui::DrawCornerFlags::ALL.bits(),
        imgui::sys::ImDrawFlags_RoundCornersAll as u32
    );
    assert_eq!(
        imgui::DrawCornerFlags::NO_ROUNDING.bits(),
        imgui::sys::ImDrawFlags_RoundCornersNone as u32
    );
    assert_eq!(
        (imgui::DrawCornerFlags::TOP_LEFT | imgui::DrawCornerFlags::BOTTOM_RIGHT).bits(),
        (imgui::sys::ImDrawFlags_RoundCornersTopLeft
            | imgui::sys::ImDrawFlags_RoundCornersBottomRight) as u32
    );
}

#[test]
fn draw_flag_domains_can_be_used_without_crossing_streams() {
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
    let draw_list = ui.get_window_draw_list();

    draw_list
        .add_rect([8.0, 8.0], [48.0, 48.0], imgui::Color::WHITE)
        .rounding(4.0)
        .flags(imgui::DrawCornerFlags::TOP_LEFT | imgui::DrawCornerFlags::BOTTOM_RIGHT)
        .build();

    draw_list
        .add_polyline(
            vec![[60.0, 8.0], [90.0, 48.0], [30.0, 48.0]],
            imgui::Color::WHITE,
        )
        .flags(imgui::PolylineFlags::CLOSED)
        .build();

    draw_list.path_clear();
    draw_list.path_line_to([110.0, 8.0]);
    draw_list.path_line_to([140.0, 48.0]);
    draw_list.path_line_to([80.0, 48.0]);
    draw_list.path_stroke(imgui::Color::WHITE, imgui::PolylineFlags::CLOSED, 1.0);

    draw_list.path_rect(
        [150.0, 8.0],
        [190.0, 48.0],
        4.0,
        imgui::DrawCornerFlags::NO_ROUNDING,
    );
    draw_list.path_stroke(imgui::Color::WHITE, imgui::PolylineFlags::CLOSED, 1.0);
}
