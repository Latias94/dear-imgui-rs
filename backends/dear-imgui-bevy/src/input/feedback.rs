//! Translation of Dear ImGui platform feedback into Bevy window feedback.

use bevy_window::{CursorIcon, SystemCursorIcon};
use dear_imgui_rs as imgui;

/// Convert a Dear ImGui mouse cursor into a Bevy window cursor icon.
#[must_use]
pub(crate) fn map_imgui_mouse_cursor(cursor: imgui::MouseCursor) -> Option<CursorIcon> {
    use imgui::MouseCursor as ImguiMouseCursor;

    let system_cursor = match cursor {
        ImguiMouseCursor::None => return None,
        ImguiMouseCursor::Arrow => SystemCursorIcon::Default,
        ImguiMouseCursor::TextInput => SystemCursorIcon::Text,
        ImguiMouseCursor::ResizeAll => SystemCursorIcon::Move,
        ImguiMouseCursor::ResizeNS => SystemCursorIcon::NsResize,
        ImguiMouseCursor::ResizeEW => SystemCursorIcon::EwResize,
        ImguiMouseCursor::ResizeNESW => SystemCursorIcon::NeswResize,
        ImguiMouseCursor::ResizeNWSE => SystemCursorIcon::NwseResize,
        ImguiMouseCursor::Hand => SystemCursorIcon::Pointer,
        ImguiMouseCursor::NotAllowed => SystemCursorIcon::NotAllowed,
    };

    Some(CursorIcon::from(system_cursor))
}
