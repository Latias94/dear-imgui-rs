use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);

    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

macro_rules! assert_panics {
    ($body:block) => {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err());
    };
}

#[test]
fn button_and_invisible_button_validate_geometry_and_flags_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("button boundaries").build(|| {
        let _ = ui.button_with_size("right aligned button", [-1.0, 0.0]);
        let _ = ui.invisible_button_flags(
            "nav invisible button",
            [0.0, 0.0],
            imgui::ButtonFlags::ENABLE_NAV | imgui::ButtonFlags::ALLOW_OVERLAP,
        );
        let _ = ui.invisible_button_options(
            "multi-button invisible button",
            [0.0, 0.0],
            imgui::InvisibleButtonOptions::new()
                .flags(imgui::ButtonFlags::ENABLE_NAV)
                .mouse_buttons(
                    imgui::InvisibleButtonMouseButtons::LEFT
                        | imgui::InvisibleButtonMouseButtons::RIGHT,
                ),
        );

        assert_panics!({
            let _ = ui.button_with_size("nan button", [f32::NAN, 1.0]);
        });

        assert_panics!({
            let _ = ui.invisible_button("bad invisible size", [1.0, f32::INFINITY]);
        });

        assert_panics!({
            let private_button_flag =
                imgui::ButtonFlags::from_bits_retain(imgui::sys::ImGuiButtonFlags_PressedOnClick);
            let _ = ui.invisible_button_flags(
                "private invisible flag",
                [1.0, 1.0],
                private_button_flag,
            );
        });

        assert_panics!({
            let mouse_button_in_flags =
                imgui::ButtonFlags::from_bits_retain(imgui::sys::ImGuiButtonFlags_MouseButtonLeft);
            let _ = ui.invisible_button_flags(
                "mouse button in independent flags",
                [1.0, 1.0],
                mouse_button_in_flags,
            );
        });

        assert_panics!({
            let independent_flag_in_mouse_buttons =
                imgui::InvisibleButtonMouseButtons::from_bits_retain(
                    imgui::sys::ImGuiButtonFlags_AllowOverlap,
                );
            let _ = ui.invisible_button_options(
                "independent flag in mouse buttons",
                [1.0, 1.0],
                imgui::InvisibleButtonOptions::new()
                    .mouse_buttons(independent_flag_in_mouse_buttons),
            );
        });

        assert_panics!({
            let _ = ui.invisible_button_options(
                "no invisible mouse button",
                [1.0, 1.0],
                imgui::InvisibleButtonOptions::new()
                    .mouse_buttons(imgui::InvisibleButtonMouseButtons::empty()),
            );
        });
    });
}

#[test]
fn arrow_button_rejects_none_direction_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("arrow boundaries").build(|| {
        let _ = ui.arrow_button("left arrow", imgui::Direction::Left);

        assert_panics!({
            let _ = ui.arrow_button("none arrow", imgui::Direction::None);
        });
    });
}

#[test]
fn progress_list_box_and_selectable_validate_numeric_inputs_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("basic widget numeric boundaries").build(|| {
        ui.progress_bar(-1.0).size([-1.0, 0.0]).build();
        let _ = ui
            .list_box_config("negative aligned list")
            .size([-1.0, 0.0])
            .begin(ui);
        let _ = ui
            .selectable_config("highlighted selectable")
            .flags(imgui::SelectableFlags::HIGHLIGHT | imgui::SelectableFlags::SELECT_ON_NAV)
            .size([0.0, 0.0])
            .build();

        assert_panics!({
            ui.progress_bar(f32::NAN).build();
        });
        assert_panics!({
            ui.progress_bar(0.5).size([1.0, f32::INFINITY]).build();
        });
        assert_panics!({
            let _ = ui
                .list_box_config("nan list")
                .size([f32::NAN, 0.0])
                .begin(ui);
        });
        assert_panics!({
            let _ = ui
                .selectable_config("negative selectable")
                .size([-1.0, 0.0])
                .build();
        });
        assert_panics!({
            let private_selectable_flag = imgui::SelectableFlags::from_bits_retain(
                imgui::sys::ImGuiSelectableFlags_SelectOnClick,
            );
            let _ = ui
                .selectable_config("private selectable flag")
                .flags(private_selectable_flag)
                .build();
        });
    });
}
