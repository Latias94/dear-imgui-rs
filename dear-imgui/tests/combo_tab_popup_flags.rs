use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn combo_options_reject_non_independent_flag_bits_before_ffi() {
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
    let height_in_flags =
        imgui::ComboBoxFlags::from_bits_retain(imgui::sys::ImGuiComboFlags_HeightSmall);
    let no_preview_in_flags =
        imgui::ComboBoxFlags::from_bits_retain(imgui::sys::ImGuiComboFlags_NoPreview);
    let custom_preview_in_flags =
        imgui::ComboBoxFlags::from_bits_retain(imgui::sys::ImGuiComboFlags_CustomPreview);

    let _ = ui.window("Combo flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_combo_with_flags("height in flags", "", height_in_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_combo_with_flags("preview in flags", "", no_preview_in_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_combo_with_flags("private in flags", "", custom_preview_in_flags);
            }))
            .is_err()
        );

        let _ = ui.begin_combo_with_flags(
            "typed options",
            "",
            imgui::ComboBoxOptions::new()
                .height(imgui::ComboBoxHeight::Small)
                .preview_mode(imgui::ComboBoxPreviewMode::PreviewNoArrowButton),
        );
    });
}

#[test]
fn tab_options_reject_non_independent_flag_bits_before_ffi() {
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
    let fitting_in_flags =
        imgui::TabBarFlags::from_bits_retain(imgui::sys::ImGuiTabBarFlags_FittingPolicyShrink);
    let private_tab_bar =
        imgui::TabBarFlags::from_bits_retain(imgui::sys::ImGuiTabBarFlags_DockNode);
    let placement_in_flags =
        imgui::TabItemFlags::from_bits_retain(imgui::sys::ImGuiTabItemFlags_Leading);
    let private_button =
        imgui::TabItemFlags::from_bits_retain(imgui::sys::ImGuiTabItemFlags_Button);

    let _ = ui.window("Tab flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.tab_bar_with_flags("fitting in flags", fitting_in_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.tab_bar_with_flags("private tab bar", private_tab_bar);
            }))
            .is_err()
        );

        let _ = ui.tab_bar_with_flags(
            "typed tabs",
            imgui::TabBarOptions::new().fitting_policy(imgui::TabBarFittingPolicy::Shrink),
        );
        let _ = ui.tab_bar("Invalid tab items").map(|_tab| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.tab_item_with_flags("placement in flags", None, placement_in_flags);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.tab_item_with_flags("button private", None, private_button);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.tab_item_button_with_flags("button private", private_button);
                }))
                .is_err()
            );

            let _ = ui.tab_item_button_with_flags(
                "typed trailing",
                imgui::TabItemOptions::new().placement(imgui::TabItemPlacement::Trailing),
            );
        });
    });
}

#[test]
fn popup_options_reject_mouse_button_bits_in_flags_before_ffi() {
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
    let mouse_button_in_flags =
        imgui::PopupFlags::from_bits_retain(imgui::sys::ImGuiPopupFlags_MouseButtonLeft);
    let legacy_popup_flags =
        imgui::PopupFlags::from_bits_retain(imgui::sys::ImGuiPopupFlags_InvalidMask_);

    let _ = ui.window("Popup flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.open_popup_with_flags("legacy popup", legacy_popup_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.open_popup_on_item_click_with_flags(None, mouse_button_in_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_popup_context_item_with_flags(None, mouse_button_in_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_popup_open_with_flags(
                    "popup",
                    imgui::PopupFlags::from_bits_retain(imgui::sys::ImGuiPopupFlags_AnyPopupLevel),
                );
            }))
            .is_err()
        );

        ui.open_popup_on_item_click_with_flags(
            Some("typed popup"),
            imgui::PopupContextOptions::new().mouse_button(imgui::PopupContextMouseButton::Left),
        );
        let _ = ui.is_popup_open_with_flags("popup", imgui::PopupFlags::ANY_POPUP);
    });
}
