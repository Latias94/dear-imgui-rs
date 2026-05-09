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
    let _ = ui.shortcut_with_flags(
        chord,
        imgui::ShortcutRoute::Global(imgui::ShortcutGlobalRouteFlags::NONE),
    );

    ui.set_next_item_shortcut(chord);
    ui.set_next_item_shortcut_with_flags(
        chord,
        imgui::NextItemShortcutOptions::new()
            .route(imgui::ShortcutRoute::Global(
                imgui::ShortcutGlobalRouteFlags::OVER_ACTIVE,
            ))
            .tooltip(true),
    );
    let _ = ui.button("Save");
}

#[test]
fn input_flag_types_match_supported_imgui_subsets() {
    assert_eq!(
        imgui::ShortcutFlags::REPEAT.bits(),
        imgui::sys::ImGuiInputFlags_Repeat
    );
    assert_eq!(
        imgui::ShortcutRoute::Global(imgui::ShortcutGlobalRouteFlags::NONE).bits(),
        imgui::sys::ImGuiInputFlags_RouteGlobal
    );
    assert_eq!(
        imgui::ShortcutRoute::FocusedOverActive.bits(),
        imgui::sys::ImGuiInputFlags_RouteFocused | imgui::sys::ImGuiInputFlags_RouteOverActive
    );
    assert_eq!(
        imgui::ShortcutOptions::new()
            .flags(imgui::ShortcutFlags::REPEAT | imgui::ShortcutFlags::ROUTE_FROM_ROOT_WINDOW)
            .route(imgui::ShortcutRoute::Global(
                imgui::ShortcutGlobalRouteFlags::OVER_FOCUSED
                    | imgui::ShortcutGlobalRouteFlags::UNLESS_BG_FOCUSED,
            ))
            .bits(),
        imgui::sys::ImGuiInputFlags_Repeat
            | imgui::sys::ImGuiInputFlags_RouteFromRootWindow
            | imgui::sys::ImGuiInputFlags_RouteGlobal
            | imgui::sys::ImGuiInputFlags_RouteOverFocused
            | imgui::sys::ImGuiInputFlags_RouteUnlessBgFocused
    );

    assert_eq!(
        imgui::NextItemShortcutFlags::TOOLTIP.bits(),
        imgui::sys::ImGuiInputFlags_Tooltip
    );
    assert_eq!(
        imgui::NextItemShortcutOptions::new()
            .route(imgui::ShortcutRoute::Always)
            .tooltip(true)
            .bits(),
        imgui::sys::ImGuiInputFlags_RouteAlways | imgui::sys::ImGuiInputFlags_Tooltip
    );

    assert_eq!(
        imgui::ItemKeyOwnerFlags::LOCK_THIS_FRAME.bits(),
        imgui::sys::ImGuiInputFlags_LockThisFrame
    );
    assert_eq!(
        imgui::ItemKeyOwnerFlags::LOCK_UNTIL_RELEASE.bits(),
        imgui::sys::ImGuiInputFlags_LockUntilRelease
    );
    assert_eq!(
        imgui::ItemKeyOwnerFlags::COND_HOVERED.bits(),
        imgui::sys::ImGuiInputFlags_CondHovered
    );
    assert_eq!(
        imgui::ItemKeyOwnerFlags::COND_ACTIVE.bits(),
        imgui::sys::ImGuiInputFlags_CondActive
    );
}
