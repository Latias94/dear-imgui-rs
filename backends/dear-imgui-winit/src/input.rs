//! Input handling for Dear ImGui winit backend
//!
//! This module provides keyboard and mouse input mapping between winit events
//! and Dear ImGui input system.

use dear_imgui_rs::{Key, input::MouseButton as ImGuiMouseButton};
use winit::event::MouseButton as WinitMouseButton;
use winit::keyboard::{Key as WinitKey, KeyCode, NamedKey, PhysicalKey};

/// Convert winit mouse button to Dear ImGui mouse button
pub fn to_imgui_mouse_button(button: WinitMouseButton) -> Option<ImGuiMouseButton> {
    match button {
        WinitMouseButton::Left => Some(ImGuiMouseButton::Left),
        WinitMouseButton::Right => Some(ImGuiMouseButton::Right),
        WinitMouseButton::Middle => Some(ImGuiMouseButton::Middle),
        WinitMouseButton::Back => Some(ImGuiMouseButton::Extra1),
        WinitMouseButton::Forward => Some(ImGuiMouseButton::Extra2),
        // Map common OS extra buttons if delivered as Other indices
        WinitMouseButton::Other(3) => Some(ImGuiMouseButton::Extra1),
        WinitMouseButton::Other(4) => Some(ImGuiMouseButton::Extra2),
        WinitMouseButton::Other(_) => None,
    }
}

fn logical_key_to_imgui_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Character(s) => {
            let ch = s.chars().next()?;
            match ch {
                '0' => Some(Key::Key0),
                '1' => Some(Key::Key1),
                '2' => Some(Key::Key2),
                '3' => Some(Key::Key3),
                '4' => Some(Key::Key4),
                '5' => Some(Key::Key5),
                '6' => Some(Key::Key6),
                '7' => Some(Key::Key7),
                '8' => Some(Key::Key8),
                '9' => Some(Key::Key9),
                'a' | 'A' => Some(Key::A),
                'b' | 'B' => Some(Key::B),
                'c' | 'C' => Some(Key::C),
                'd' | 'D' => Some(Key::D),
                'e' | 'E' => Some(Key::E),
                'f' | 'F' => Some(Key::F),
                'g' | 'G' => Some(Key::G),
                'h' | 'H' => Some(Key::H),
                'i' | 'I' => Some(Key::I),
                'j' | 'J' => Some(Key::J),
                'k' | 'K' => Some(Key::K),
                'l' | 'L' => Some(Key::L),
                'm' | 'M' => Some(Key::M),
                'n' | 'N' => Some(Key::N),
                'o' | 'O' => Some(Key::O),
                'p' | 'P' => Some(Key::P),
                'q' | 'Q' => Some(Key::Q),
                'r' | 'R' => Some(Key::R),
                's' | 'S' => Some(Key::S),
                't' | 'T' => Some(Key::T),
                'u' | 'U' => Some(Key::U),
                'v' | 'V' => Some(Key::V),
                'w' | 'W' => Some(Key::W),
                'x' | 'X' => Some(Key::X),
                'y' | 'Y' => Some(Key::Y),
                'z' | 'Z' => Some(Key::Z),
                _ => None,
            }
        }
        WinitKey::Named(named_key) => match named_key {
            // Navigation keys
            NamedKey::ArrowDown => Some(Key::DownArrow),
            NamedKey::ArrowLeft => Some(Key::LeftArrow),
            NamedKey::ArrowRight => Some(Key::RightArrow),
            NamedKey::ArrowUp => Some(Key::UpArrow),
            NamedKey::End => Some(Key::End),
            NamedKey::Home => Some(Key::Home),
            NamedKey::PageDown => Some(Key::PageDown),
            NamedKey::PageUp => Some(Key::PageUp),

            // Editing keys
            NamedKey::Backspace => Some(Key::Backspace),
            NamedKey::Delete => Some(Key::Delete),
            NamedKey::Insert => Some(Key::Insert),

            // Whitespace keys
            NamedKey::Tab => Some(Key::Tab),
            NamedKey::Space => Some(Key::Space),
            NamedKey::Enter => Some(Key::Enter),
            NamedKey::Escape => Some(Key::Escape),
            NamedKey::F1 => Some(Key::F1),
            NamedKey::F2 => Some(Key::F2),
            NamedKey::F3 => Some(Key::F3),
            NamedKey::F4 => Some(Key::F4),
            NamedKey::F5 => Some(Key::F5),
            NamedKey::F6 => Some(Key::F6),
            NamedKey::F7 => Some(Key::F7),
            NamedKey::F8 => Some(Key::F8),
            NamedKey::F9 => Some(Key::F9),
            NamedKey::F10 => Some(Key::F10),
            NamedKey::F11 => Some(Key::F11),
            NamedKey::F12 => Some(Key::F12),
            NamedKey::F13 => Some(Key::F13),
            NamedKey::F14 => Some(Key::F14),
            NamedKey::F15 => Some(Key::F15),
            NamedKey::F16 => Some(Key::F16),
            NamedKey::F17 => Some(Key::F17),
            NamedKey::F18 => Some(Key::F18),
            NamedKey::F19 => Some(Key::F19),
            NamedKey::F20 => Some(Key::F20),
            NamedKey::F21 => Some(Key::F21),
            NamedKey::F22 => Some(Key::F22),
            NamedKey::F23 => Some(Key::F23),
            NamedKey::F24 => Some(Key::F24),
            NamedKey::CapsLock => Some(Key::CapsLock),
            NamedKey::ScrollLock => Some(Key::ScrollLock),
            NamedKey::NumLock => Some(Key::NumLock),
            NamedKey::PrintScreen => Some(Key::PrintScreen),
            NamedKey::Pause => Some(Key::Pause),
            NamedKey::ContextMenu => Some(Key::Menu),

            _ => None,
        },
        _ => None,
    }
}

fn physical_key_to_imgui_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Backquote => Key::GraveAccent,
        KeyCode::Backslash => Key::Backslash,
        KeyCode::BracketLeft => Key::LeftBracket,
        KeyCode::BracketRight => Key::RightBracket,
        KeyCode::Comma => Key::Comma,
        KeyCode::Digit0 => Key::Key0,
        KeyCode::Digit1 => Key::Key1,
        KeyCode::Digit2 => Key::Key2,
        KeyCode::Digit3 => Key::Key3,
        KeyCode::Digit4 => Key::Key4,
        KeyCode::Digit5 => Key::Key5,
        KeyCode::Digit6 => Key::Key6,
        KeyCode::Digit7 => Key::Key7,
        KeyCode::Digit8 => Key::Key8,
        KeyCode::Digit9 => Key::Key9,
        KeyCode::Equal => Key::Equal,
        KeyCode::IntlBackslash => Key::Oem102,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::Minus => Key::Minus,
        KeyCode::Period => Key::Period,
        KeyCode::Quote => Key::Apostrophe,
        KeyCode::Semicolon => Key::Semicolon,
        KeyCode::Slash => Key::Slash,
        KeyCode::AltLeft => Key::LeftAlt,
        KeyCode::AltRight => Key::RightAlt,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::CapsLock => Key::CapsLock,
        KeyCode::ContextMenu => Key::Menu,
        KeyCode::ControlLeft => Key::LeftCtrl,
        KeyCode::ControlRight => Key::RightCtrl,
        KeyCode::Enter => Key::Enter,
        KeyCode::SuperLeft => Key::LeftSuper,
        KeyCode::SuperRight => Key::RightSuper,
        KeyCode::ShiftLeft => Key::LeftShift,
        KeyCode::ShiftRight => Key::RightShift,
        KeyCode::Space => Key::Space,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::End => Key::End,
        KeyCode::Home => Key::Home,
        KeyCode::Insert => Key::Insert,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::ArrowDown => Key::DownArrow,
        KeyCode::ArrowLeft => Key::LeftArrow,
        KeyCode::ArrowRight => Key::RightArrow,
        KeyCode::ArrowUp => Key::UpArrow,
        KeyCode::NumLock => Key::NumLock,
        KeyCode::Numpad0 => Key::Keypad0,
        KeyCode::Numpad1 => Key::Keypad1,
        KeyCode::Numpad2 => Key::Keypad2,
        KeyCode::Numpad3 => Key::Keypad3,
        KeyCode::Numpad4 => Key::Keypad4,
        KeyCode::Numpad5 => Key::Keypad5,
        KeyCode::Numpad6 => Key::Keypad6,
        KeyCode::Numpad7 => Key::Keypad7,
        KeyCode::Numpad8 => Key::Keypad8,
        KeyCode::Numpad9 => Key::Keypad9,
        KeyCode::NumpadAdd => Key::KeypadAdd,
        KeyCode::NumpadDecimal | KeyCode::NumpadComma => Key::KeypadDecimal,
        KeyCode::NumpadDivide => Key::KeypadDivide,
        KeyCode::NumpadEnter => Key::KeypadEnter,
        KeyCode::NumpadEqual => Key::KeypadEqual,
        KeyCode::NumpadMultiply => Key::KeypadMultiply,
        KeyCode::NumpadSubtract => Key::KeypadSubtract,
        KeyCode::Escape => Key::Escape,
        KeyCode::PrintScreen => Key::PrintScreen,
        KeyCode::ScrollLock => Key::ScrollLock,
        KeyCode::Pause => Key::Pause,
        KeyCode::BrowserBack => Key::AppBack,
        KeyCode::BrowserForward => Key::AppForward,
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,
        KeyCode::F21 => Key::F21,
        KeyCode::F22 => Key::F22,
        KeyCode::F23 => Key::F23,
        KeyCode::F24 => Key::F24,
        _ => return None,
    })
}

fn physical_key_takes_priority(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Backquote
            | KeyCode::Backslash
            | KeyCode::BracketLeft
            | KeyCode::BracketRight
            | KeyCode::Comma
            | KeyCode::Equal
            | KeyCode::IntlBackslash
            | KeyCode::Minus
            | KeyCode::Period
            | KeyCode::Quote
            | KeyCode::Semicolon
            | KeyCode::Slash
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::Numpad0
            | KeyCode::Numpad1
            | KeyCode::Numpad2
            | KeyCode::Numpad3
            | KeyCode::Numpad4
            | KeyCode::Numpad5
            | KeyCode::Numpad6
            | KeyCode::Numpad7
            | KeyCode::Numpad8
            | KeyCode::Numpad9
            | KeyCode::NumpadAdd
            | KeyCode::NumpadComma
            | KeyCode::NumpadDecimal
            | KeyCode::NumpadDivide
            | KeyCode::NumpadEnter
            | KeyCode::NumpadEqual
            | KeyCode::NumpadMultiply
            | KeyCode::NumpadSubtract
    )
}

/// Converts a Winit logical/physical key pair to Dear ImGui's hybrid key model.
///
/// Layout-aware letters and digits preserve user-facing shortcuts. Physical punctuation,
/// modifiers, and keypad keys remain stable across Shift and layout transformations. A physical
/// fallback keeps shortcuts usable on layouts without Latin logical keys.
pub fn winit_key_to_imgui_key(logical: &WinitKey, physical: PhysicalKey) -> Option<Key> {
    let physical = match physical {
        PhysicalKey::Code(code) => Some(code),
        PhysicalKey::Unidentified(_) => None,
    };
    if let Some(code) = physical.filter(|code| physical_key_takes_priority(*code)) {
        return physical_key_to_imgui_key(code);
    }
    logical_key_to_imgui_key(logical).or_else(|| physical.and_then(physical_key_to_imgui_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_button_mapping() {
        assert_eq!(
            to_imgui_mouse_button(WinitMouseButton::Left),
            Some(ImGuiMouseButton::Left)
        );
        assert_eq!(
            to_imgui_mouse_button(WinitMouseButton::Right),
            Some(ImGuiMouseButton::Right)
        );
        assert_eq!(
            to_imgui_mouse_button(WinitMouseButton::Middle),
            Some(ImGuiMouseButton::Middle)
        );
        assert_eq!(
            to_imgui_mouse_button(WinitMouseButton::Back),
            Some(ImGuiMouseButton::Extra1)
        );
        assert_eq!(
            to_imgui_mouse_button(WinitMouseButton::Forward),
            Some(ImGuiMouseButton::Extra2)
        );
        assert_eq!(to_imgui_mouse_button(WinitMouseButton::Other(10)), None);
    }

    #[test]
    fn test_key_mapping() {
        assert_eq!(
            winit_key_to_imgui_key(
                &WinitKey::Character("a".into()),
                PhysicalKey::Code(KeyCode::KeyQ),
            ),
            Some(Key::A)
        );
        assert_eq!(
            winit_key_to_imgui_key(
                &WinitKey::Character(":".into()),
                PhysicalKey::Code(KeyCode::Semicolon),
            ),
            Some(Key::Semicolon)
        );
        assert_eq!(
            winit_key_to_imgui_key(
                &WinitKey::Character("1".into()),
                PhysicalKey::Code(KeyCode::Numpad1),
            ),
            Some(Key::Keypad1)
        );
        assert_eq!(
            winit_key_to_imgui_key(
                &WinitKey::Character("ф".into()),
                PhysicalKey::Code(KeyCode::KeyA),
            ),
            Some(Key::A)
        );
        assert_eq!(
            winit_key_to_imgui_key(
                &WinitKey::Named(NamedKey::Escape),
                PhysicalKey::Code(KeyCode::Escape),
            ),
            Some(Key::Escape)
        );
        assert_eq!(
            winit_key_to_imgui_key(
                &WinitKey::Named(NamedKey::Shift),
                PhysicalKey::Code(KeyCode::ShiftRight),
            ),
            Some(Key::RightShift)
        );
    }
}
