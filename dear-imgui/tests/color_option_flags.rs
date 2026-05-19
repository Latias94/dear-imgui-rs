use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn color_options_reject_raw_option_bits_before_ffi() {
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
    let unsupported_edit_flags = imgui::ColorEditFlags::from_bits_retain(
        imgui::sys::ImGuiColorEditFlags_NoSidePreview as u32,
    );
    let unsupported_picker_flags =
        imgui::ColorPickerFlags::from_bits_retain(imgui::sys::ImGuiColorEditFlags_NoPicker as u32);
    let unsupported_button_flags = imgui::ColorButtonFlags::from_bits_retain(
        imgui::sys::ImGuiColorEditFlags_NoSidePreview as u32,
    );
    let unsupported_display_flags = imgui::ColorPickerDisplayFlags::from_bits_retain(
        imgui::sys::ImGuiColorEditFlags_InputRGB as u32,
    );

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_color_edit_options(imgui::ColorEditOptions::new().flags(unsupported_edit_flags));
        }))
        .is_err()
    );

    let _ = ui.window("Color flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut color = [0.1, 0.2, 0.3, 0.4];
                let _ = ui
                    .color_edit4_config("invalid color edit", &mut color)
                    .flags(unsupported_edit_flags)
                    .build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut color = [0.1, 0.2, 0.3, 0.4];
                let _ = ui
                    .color_picker4_config("invalid color picker", &mut color)
                    .flags(unsupported_picker_flags)
                    .build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut color = [0.1, 0.2, 0.3, 0.4];
                let _ = ui
                    .color_picker4_config("invalid display flags", &mut color)
                    .display_flags(unsupported_display_flags)
                    .build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .color_button_config("invalid color button", [0.1, 0.2, 0.3, 0.4])
                    .flags(unsupported_button_flags)
                    .build();
            }))
            .is_err()
        );

        ui.set_color_edit_options(
            imgui::ColorEditOptions::new()
                .flags(
                    imgui::ColorEditFlags::NO_COLOR_MARKERS | imgui::ColorEditFlags::ALPHA_OPAQUE,
                )
                .display_mode(imgui::ColorDisplayMode::Hsv)
                .data_type(imgui::ColorDataType::Float)
                .picker_mode(imgui::ColorPickerMode::HueWheel)
                .input_mode(imgui::ColorInputMode::Rgb),
        );

        let mut edit_color = [0.1, 0.2, 0.3, 0.4];
        let _ = ui
            .color_edit4_config("typed color edit", &mut edit_color)
            .flags(
                imgui::ColorEditOptions::new()
                    .flags(imgui::ColorEditFlags::ALPHA_NO_BG)
                    .display_mode(imgui::ColorDisplayMode::Rgb)
                    .data_type(imgui::ColorDataType::Uint8)
                    .picker_mode(imgui::ColorPickerMode::HueBar)
                    .input_mode(imgui::ColorInputMode::Rgb),
            )
            .build();

        let mut picker_color = [0.1, 0.2, 0.3, 0.4];
        let _ = ui
            .color_picker4_config("typed color picker", &mut picker_color)
            .flags(
                imgui::ColorPickerOptions::new()
                    .flags(
                        imgui::ColorPickerFlags::ALPHA_BAR
                            | imgui::ColorPickerFlags::ALPHA_PREVIEW_HALF,
                    )
                    .display_flags(
                        imgui::ColorPickerDisplayFlags::RGB | imgui::ColorPickerDisplayFlags::HEX,
                    )
                    .data_type(imgui::ColorDataType::Float)
                    .picker_mode(imgui::ColorPickerMode::HueBar)
                    .input_mode(imgui::ColorInputMode::Rgb),
            )
            .build();

        let _ = ui
            .color_button_config("typed color button", [0.1, 0.2, 0.3, 0.4])
            .flags(
                imgui::ColorButtonOptions::new()
                    .flags(
                        imgui::ColorButtonFlags::NO_TOOLTIP | imgui::ColorButtonFlags::ALPHA_OPAQUE,
                    )
                    .input_mode(imgui::ColorInputMode::Rgb),
            )
            .build();
    });
}
