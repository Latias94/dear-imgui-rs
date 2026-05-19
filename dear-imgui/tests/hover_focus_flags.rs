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

#[test]
fn hover_and_focus_flag_types_include_current_public_upstream_aliases() {
    assert_eq!(
        imgui::ItemHoveredFlags::ALLOW_WHEN_OVERLAPPED_BY_ITEM.bits(),
        imgui::sys::ImGuiHoveredFlags_AllowWhenOverlappedByItem
    );
    assert_eq!(
        imgui::ItemHoveredFlags::ALLOW_WHEN_OVERLAPPED_BY_WINDOW.bits(),
        imgui::sys::ImGuiHoveredFlags_AllowWhenOverlappedByWindow
    );
    assert_eq!(
        imgui::ItemHoveredFlags::RECT_ONLY.bits(),
        imgui::sys::ImGuiHoveredFlags_RectOnly
    );
    assert_eq!(
        imgui::WindowHoveredFlags::ROOT_AND_CHILD_WINDOWS.bits(),
        imgui::sys::ImGuiHoveredFlags_RootAndChildWindows
    );
    assert_eq!(
        imgui::TooltipHoveredFlags::DELAY_SHORT.bits(),
        imgui::sys::ImGuiHoveredFlags_DelayShort
    );
    assert_eq!(
        imgui::FocusedFlags::ROOT_AND_CHILD_WINDOWS.bits(),
        imgui::sys::ImGuiFocusedFlags_RootAndChildWindows
    );
}

#[test]
fn hover_query_methods_reject_flags_invalid_for_their_call_site_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("hover flag boundaries").build(|| {
        ui.text("hover target");

        let legal_item_flags = imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED
            | imgui::ItemHoveredFlags::ALLOW_WHEN_OVERLAPPED_BY_ITEM
            | imgui::ItemHoveredFlags::DELAY_SHORT;
        let _ = ui.is_item_hovered_with_flags(legal_item_flags);

        let legal_window_flags = imgui::WindowHoveredFlags::ROOT_AND_CHILD_WINDOWS
            | imgui::WindowHoveredFlags::ALLOW_WHEN_BLOCKED_BY_POPUP
            | imgui::WindowHoveredFlags::STATIONARY;
        let _ = ui.is_window_hovered_with_flags(legal_window_flags);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let flags = imgui::ItemHoveredFlags::from_bits_retain(
                    imgui::WindowHoveredFlags::CHILD_WINDOWS.bits(),
                );
                let _ = ui.is_item_hovered_with_flags(flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let flags = imgui::WindowHoveredFlags::from_bits_retain(
                    imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED.bits(),
                );
                let _ = ui.is_window_hovered_with_flags(flags);
            }))
            .is_err()
        );

        let raw_item_unused_bit = imgui::ItemHoveredFlags::from_bits_retain(1 << 6);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_item_hovered_with_flags(raw_item_unused_bit);
            }))
            .is_err()
        );
        let raw_window_unused_bit = imgui::WindowHoveredFlags::from_bits_retain(1 << 6);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_window_hovered_with_flags(raw_window_unused_bit);
            }))
            .is_err()
        );
    });
}

#[test]
fn focused_query_rejects_raw_unknown_bits_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let _ = ui.window("focus flag boundaries").build(|| {
        let legal_flags =
            imgui::FocusedFlags::ROOT_AND_CHILD_WINDOWS | imgui::FocusedFlags::DOCK_HIERARCHY;
        let _ = ui.is_window_focused_with_flags(legal_flags);

        let raw_unknown = imgui::FocusedFlags::from_bits_retain(1 << 5);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.is_window_focused_with_flags(raw_unknown);
            }))
            .is_err()
        );
    });
}

#[test]
fn tooltip_hover_style_flags_reject_raw_window_or_recursive_flags_before_storage() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let style = ctx.style_mut();

    let legal_mouse_flags = imgui::TooltipHoveredFlags::ALLOW_WHEN_DISABLED
        | imgui::TooltipHoveredFlags::STATIONARY
        | imgui::TooltipHoveredFlags::DELAY_SHORT;
    style.set_hover_flags_for_tooltip_mouse(legal_mouse_flags);
    assert_eq!(style.hover_flags_for_tooltip_mouse(), legal_mouse_flags);

    for invalid in [
        imgui::TooltipHoveredFlags::from_bits_retain(
            imgui::WindowHoveredFlags::CHILD_WINDOWS.bits(),
        ),
        imgui::TooltipHoveredFlags::from_bits_retain(imgui::ItemHoveredFlags::FOR_TOOLTIP.bits()),
        imgui::TooltipHoveredFlags::from_bits_retain(1 << 6),
    ] {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                style.set_hover_flags_for_tooltip_mouse(invalid);
            }))
            .is_err()
        );
        assert_eq!(style.hover_flags_for_tooltip_mouse(), legal_mouse_flags);
    }

    let legal_nav_flags =
        imgui::TooltipHoveredFlags::ALLOW_WHEN_DISABLED | imgui::TooltipHoveredFlags::DELAY_NORMAL;
    style.set_hover_flags_for_tooltip_nav(legal_nav_flags);
    assert_eq!(style.hover_flags_for_tooltip_nav(), legal_nav_flags);
}
