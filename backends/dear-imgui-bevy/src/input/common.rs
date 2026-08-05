use std::collections::HashSet;

use bevy_input::keyboard::KeyCode;
use bevy_input::mouse::{MouseButton as BevyMouseButton, MouseScrollUnit};
use bevy_math::Vec2;
use bevy_window::Window;
#[cfg(not(feature = "render"))]
use bevy_window::WindowPosition;
use dear_imgui_rs as imgui;

use super::state::ImguiInputWindow;

pub(super) const INVALID_MOUSE_POS: [f32; 2] = [-f32::MAX, -f32::MAX];

pub(super) fn add_mouse_viewport_event(io: &mut imgui::Io, viewport_id: Option<imgui::Id>) {
    if !io
        .config_flags()
        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    {
        return;
    }
    if io
        .backend_flags()
        .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT)
    {
        io.add_mouse_viewport_event(viewport_id.unwrap_or_default());
    }
}

pub(super) fn mouse_pos_for_window(
    context: &imgui::Context,
    _window: ImguiInputWindow,
    local_pos: Vec2,
) -> [f32; 2] {
    let pos = [local_pos.x, local_pos.y];
    if !context
        .io()
        .config_flags()
        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    {
        return pos;
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        crate::viewport::window_client_logical_to_desktop(
            _window.entity,
            _window.scale_factor,
            _window.desktop_origin,
            pos,
        )
        .unwrap_or(pos)
    }

    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    {
        #[cfg(not(feature = "render"))]
        {
            let WindowPosition::At(window_pos) = _window.position else {
                return pos;
            };
            let scale_factor = positive_finite_or(_window.scale_factor, 1.0);
            return [
                pos[0] + window_pos.x as f32 / scale_factor,
                pos[1] + window_pos.y as f32 / scale_factor,
            ];
        }
        #[cfg(feature = "render")]
        pos
    }
}

pub(super) fn positive_finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

/// Convert a Bevy mouse button into Dear ImGui's button space.
#[must_use]
pub(crate) fn map_bevy_mouse_button(button: BevyMouseButton) -> Option<imgui::MouseButton> {
    match button {
        BevyMouseButton::Left => Some(imgui::MouseButton::Left),
        BevyMouseButton::Right => Some(imgui::MouseButton::Right),
        BevyMouseButton::Middle => Some(imgui::MouseButton::Middle),
        BevyMouseButton::Back => Some(imgui::MouseButton::Extra1),
        BevyMouseButton::Forward => Some(imgui::MouseButton::Extra2),
        BevyMouseButton::Other(_) => None,
    }
}

/// Convert a Bevy physical key code into Dear ImGui's key space.
#[must_use]
pub(crate) fn map_bevy_key_code(key_code: KeyCode) -> Option<imgui::Key> {
    use KeyCode as B;
    use imgui::Key as I;

    match key_code {
        B::Backquote => Some(I::GraveAccent),
        B::Backslash => Some(I::Backslash),
        B::BracketLeft => Some(I::LeftBracket),
        B::BracketRight => Some(I::RightBracket),
        B::Comma => Some(I::Comma),
        B::Digit0 => Some(I::Key0),
        B::Digit1 => Some(I::Key1),
        B::Digit2 => Some(I::Key2),
        B::Digit3 => Some(I::Key3),
        B::Digit4 => Some(I::Key4),
        B::Digit5 => Some(I::Key5),
        B::Digit6 => Some(I::Key6),
        B::Digit7 => Some(I::Key7),
        B::Digit8 => Some(I::Key8),
        B::Digit9 => Some(I::Key9),
        B::Equal => Some(I::Equal),
        B::IntlBackslash | B::IntlRo | B::IntlYen => Some(I::Oem102),
        B::KeyA => Some(I::A),
        B::KeyB => Some(I::B),
        B::KeyC => Some(I::C),
        B::KeyD => Some(I::D),
        B::KeyE => Some(I::E),
        B::KeyF => Some(I::F),
        B::KeyG => Some(I::G),
        B::KeyH => Some(I::H),
        B::KeyI => Some(I::I),
        B::KeyJ => Some(I::J),
        B::KeyK => Some(I::K),
        B::KeyL => Some(I::L),
        B::KeyM => Some(I::M),
        B::KeyN => Some(I::N),
        B::KeyO => Some(I::O),
        B::KeyP => Some(I::P),
        B::KeyQ => Some(I::Q),
        B::KeyR => Some(I::R),
        B::KeyS => Some(I::S),
        B::KeyT => Some(I::T),
        B::KeyU => Some(I::U),
        B::KeyV => Some(I::V),
        B::KeyW => Some(I::W),
        B::KeyX => Some(I::X),
        B::KeyY => Some(I::Y),
        B::KeyZ => Some(I::Z),
        B::Minus => Some(I::Minus),
        B::Period => Some(I::Period),
        B::Quote => Some(I::Apostrophe),
        B::Semicolon => Some(I::Semicolon),
        B::Slash => Some(I::Slash),
        B::AltLeft => Some(I::LeftAlt),
        B::AltRight => Some(I::RightAlt),
        B::Backspace | B::NumpadBackspace => Some(I::Backspace),
        B::CapsLock => Some(I::CapsLock),
        B::ContextMenu => Some(I::Menu),
        B::ControlLeft => Some(I::LeftCtrl),
        B::ControlRight => Some(I::RightCtrl),
        B::Enter => Some(I::Enter),
        B::SuperLeft | B::Meta => Some(I::LeftSuper),
        B::SuperRight => Some(I::RightSuper),
        B::ShiftLeft => Some(I::LeftShift),
        B::ShiftRight => Some(I::RightShift),
        B::Space => Some(I::Space),
        B::Tab => Some(I::Tab),
        B::Delete => Some(I::Delete),
        B::End => Some(I::End),
        B::Home => Some(I::Home),
        B::Insert => Some(I::Insert),
        B::PageDown => Some(I::PageDown),
        B::PageUp => Some(I::PageUp),
        B::ArrowDown => Some(I::DownArrow),
        B::ArrowLeft => Some(I::LeftArrow),
        B::ArrowRight => Some(I::RightArrow),
        B::ArrowUp => Some(I::UpArrow),
        B::NumLock => Some(I::NumLock),
        B::Numpad0 => Some(I::Keypad0),
        B::Numpad1 => Some(I::Keypad1),
        B::Numpad2 => Some(I::Keypad2),
        B::Numpad3 => Some(I::Keypad3),
        B::Numpad4 => Some(I::Keypad4),
        B::Numpad5 => Some(I::Keypad5),
        B::Numpad6 => Some(I::Keypad6),
        B::Numpad7 => Some(I::Keypad7),
        B::Numpad8 => Some(I::Keypad8),
        B::Numpad9 => Some(I::Keypad9),
        B::NumpadAdd => Some(I::KeypadAdd),
        B::NumpadDecimal | B::NumpadComma => Some(I::KeypadDecimal),
        B::NumpadDivide => Some(I::KeypadDivide),
        B::NumpadEnter => Some(I::KeypadEnter),
        B::NumpadEqual => Some(I::KeypadEqual),
        B::NumpadMultiply | B::NumpadStar => Some(I::KeypadMultiply),
        B::NumpadSubtract => Some(I::KeypadSubtract),
        B::Escape => Some(I::Escape),
        B::PrintScreen => Some(I::PrintScreen),
        B::ScrollLock => Some(I::ScrollLock),
        B::Pause => Some(I::Pause),
        B::F1 => Some(I::F1),
        B::F2 => Some(I::F2),
        B::F3 => Some(I::F3),
        B::F4 => Some(I::F4),
        B::F5 => Some(I::F5),
        B::F6 => Some(I::F6),
        B::F7 => Some(I::F7),
        B::F8 => Some(I::F8),
        B::F9 => Some(I::F9),
        B::F10 => Some(I::F10),
        B::F11 => Some(I::F11),
        B::F12 => Some(I::F12),
        _ => None,
    }
}

#[cfg(not(feature = "render"))]
pub(super) fn sync_window_metrics(context: &mut imgui::Context, window: &Window) {
    let io = context.io_mut();
    io.set_display_size(sanitized_window_display_size(window));
    io.set_display_framebuffer_scale(sanitized_window_framebuffer_scale(window));
}

#[cfg(not(feature = "render"))]
pub(super) fn set_framebuffer_scale(context: &mut imgui::Context, scale_factor: f32) {
    context
        .io_mut()
        .set_display_framebuffer_scale([scale_factor, scale_factor]);
}

pub(crate) fn sanitized_window_display_size(window: &Window) -> [f32; 2] {
    finite_non_negative_size([window.width(), window.height()])
}

pub(crate) fn sanitized_window_framebuffer_scale(window: &Window) -> [f32; 2] {
    let scale_factor = positive_finite_or(window.scale_factor(), 1.0);
    [scale_factor, scale_factor]
}

pub(super) fn finite_non_negative_size(size: [f32; 2]) -> [f32; 2] {
    [
        if size[0].is_finite() && size[0] >= 0.0 {
            size[0]
        } else {
            0.0
        },
        if size[1].is_finite() && size[1] >= 0.0 {
            size[1]
        } else {
            0.0
        },
    ]
}

pub(super) fn clear_mouse_hovered_viewport(io: &mut imgui::Io) {
    io.set_mouse_hovered_viewport(imgui::Id::from(0));
}

pub(super) fn normalize_wheel(unit: MouseScrollUnit, x: f32, y: f32) -> [f32; 2] {
    match unit {
        MouseScrollUnit::Line => [x, y],
        MouseScrollUnit::Pixel => [pixel_wheel_step(x), pixel_wheel_step(y)],
    }
}

fn pixel_wheel_step(value: f32) -> f32 {
    match value.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Greater) => 1.0,
        Some(std::cmp::Ordering::Less) => -1.0,
        _ => 0.0,
    }
}

pub(super) fn add_keyboard_text(io: &mut imgui::Io, text: &str) {
    for character in text.chars().filter(|character| *character != '\u{7f}') {
        io.add_input_character(character);
    }
}

pub(super) fn add_ime_text(io: &mut imgui::Io, text: &str) {
    for character in text.chars().filter(|character| !character.is_control()) {
        io.add_input_character(character);
    }
}

pub(super) fn modifier_state(keys: &HashSet<imgui::Key>) -> (bool, bool, bool, bool) {
    (
        keys.contains(&imgui::Key::LeftCtrl) || keys.contains(&imgui::Key::RightCtrl),
        keys.contains(&imgui::Key::LeftShift) || keys.contains(&imgui::Key::RightShift),
        keys.contains(&imgui::Key::LeftAlt) || keys.contains(&imgui::Key::RightAlt),
        keys.contains(&imgui::Key::LeftSuper) || keys.contains(&imgui::Key::RightSuper),
    )
}

pub(super) fn apply_modifier_events(io: &mut imgui::Io, modifiers: (bool, bool, bool, bool)) {
    io.add_key_event(imgui::Key::ModCtrl, modifiers.0);
    io.add_key_event(imgui::Key::ModShift, modifiers.1);
    io.add_key_event(imgui::Key::ModAlt, modifiers.2);
    io.add_key_event(imgui::Key::ModSuper, modifiers.3);
}

pub(super) fn release_sticky_keys_and_buttons(
    io: &mut imgui::Io,
    keys: &mut HashSet<imgui::Key>,
    mouse_buttons: &mut HashSet<imgui::MouseButton>,
) {
    for key in keys.drain() {
        io.add_key_event(key, false);
    }
    apply_modifier_events(io, (false, false, false, false));
    for button in mouse_buttons.drain() {
        io.add_mouse_button_event(button, false);
    }
}
