//! Input handling for Dear ImGui winit backend
//!
//! This module provides keyboard and mouse input mapping between winit events
//! and Dear ImGui input system.

use dear_imgui::{Key, input::MouseButton as ImGuiMouseButton};
use winit::event::MouseButton as WinitMouseButton;
use winit::keyboard::{Key as WinitKey, KeyLocation, NamedKey};

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

/// Convert winit key to Dear ImGui key with location awareness
pub fn winit_key_to_imgui_key(key: &WinitKey, location: KeyLocation) -> Option<Key> {
    match key {
        WinitKey::Character(s) => {
            let ch = s.chars().next()?;
            match (ch, location) {
                // Numbers (0-9)
                ('0', _) => Some(Key::Key0),
                ('1', _) => Some(Key::Key1),
                ('2', _) => Some(Key::Key2),
                ('3', _) => Some(Key::Key3),
                ('4', _) => Some(Key::Key4),
                ('5', _) => Some(Key::Key5),
                ('6', _) => Some(Key::Key6),
                ('7', _) => Some(Key::Key7),
                ('8', _) => Some(Key::Key8),
                ('9', _) => Some(Key::Key9),

                // Letters (A-Z)
                ('a' | 'A', _) => Some(Key::A),
                ('b' | 'B', _) => Some(Key::B),
                ('c' | 'C', _) => Some(Key::C),
                ('d' | 'D', _) => Some(Key::D),
                ('e' | 'E', _) => Some(Key::E),
                ('f' | 'F', _) => Some(Key::F),
                ('g' | 'G', _) => Some(Key::G),
                ('h' | 'H', _) => Some(Key::H),
                ('i' | 'I', _) => Some(Key::I),
                ('j' | 'J', _) => Some(Key::J),
                ('k' | 'K', _) => Some(Key::K),
                ('l' | 'L', _) => Some(Key::L),
                ('m' | 'M', _) => Some(Key::M),
                ('n' | 'N', _) => Some(Key::N),
                ('o' | 'O', _) => Some(Key::O),
                ('p' | 'P', _) => Some(Key::P),
                ('q' | 'Q', _) => Some(Key::Q),
                ('r' | 'R', _) => Some(Key::R),
                ('s' | 'S', _) => Some(Key::S),
                ('t' | 'T', _) => Some(Key::T),
                ('u' | 'U', _) => Some(Key::U),
                ('v' | 'V', _) => Some(Key::V),
                ('w' | 'W', _) => Some(Key::W),
                ('x' | 'X', _) => Some(Key::X),
                ('y' | 'Y', _) => Some(Key::Y),
                ('z' | 'Z', _) => Some(Key::Z),

                // Punctuation
                ('\'', _) => Some(Key::Apostrophe),
                (',', _) => Some(Key::Comma),
                ('-', KeyLocation::Standard) => Some(Key::Minus),
                ('-', KeyLocation::Numpad) => Some(Key::KeypadSubtract),
                ('.', KeyLocation::Standard) => Some(Key::Period),
                ('.', KeyLocation::Numpad) => Some(Key::KeypadDecimal),
                ('/', KeyLocation::Standard) => Some(Key::Slash),
                ('/', KeyLocation::Numpad) => Some(Key::KeypadDivide),
                ('*', KeyLocation::Numpad) => Some(Key::KeypadMultiply),
                ('+', KeyLocation::Numpad) => Some(Key::KeypadAdd),
                (';', _) => Some(Key::Semicolon),
                ('=', KeyLocation::Standard) => Some(Key::Equal),
                ('=', KeyLocation::Numpad) => Some(Key::KeypadEqual),
                ('[', _) => Some(Key::LeftBracket),
                ('\\', _) => Some(Key::Backslash),
                (']', _) => Some(Key::RightBracket),
                ('`', _) => Some(Key::GraveAccent),

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
            NamedKey::Enter => match location {
                KeyLocation::Numpad => Some(Key::KeypadEnter),
                _ => Some(Key::Enter),
            },
            NamedKey::Escape => Some(Key::Escape),

            // Modifier keys - distinguish left/right
            NamedKey::Shift => match location {
                KeyLocation::Left => Some(Key::LeftShift),
                KeyLocation::Right => Some(Key::RightShift),
                _ => Some(Key::LeftShift), // Default to left
            },
            NamedKey::Control => match location {
                KeyLocation::Left => Some(Key::LeftCtrl),
                KeyLocation::Right => Some(Key::RightCtrl),
                _ => Some(Key::LeftCtrl), // Default to left
            },
            NamedKey::Alt => match location {
                KeyLocation::Left => Some(Key::LeftAlt),
                KeyLocation::Right => Some(Key::RightAlt),
                _ => Some(Key::LeftAlt), // Default to left
            },
            NamedKey::Super => match location {
                KeyLocation::Left => Some(Key::LeftSuper),
                KeyLocation::Right => Some(Key::RightSuper),
                _ => Some(Key::LeftSuper), // Default to left
            },

            // Function keys
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
            // Lock keys
            NamedKey::CapsLock => Some(Key::CapsLock),
            NamedKey::ScrollLock => Some(Key::ScrollLock),
            NamedKey::NumLock => Some(Key::NumLock),

            // Special keys
            NamedKey::PrintScreen => Some(Key::PrintScreen),
            NamedKey::Pause => Some(Key::Pause),
            NamedKey::ContextMenu => Some(Key::Menu),

            _ => None,
        },
        _ => None,
    }
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
            None // Not supported in our MouseButton enum
        );
        assert_eq!(
            to_imgui_mouse_button(WinitMouseButton::Forward),
            None // Not supported in our MouseButton enum
        );
        assert_eq!(to_imgui_mouse_button(WinitMouseButton::Other(10)), None);
    }

    #[test]
    fn test_key_mapping() {
        // Test character keys
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Character("a".into()), KeyLocation::Standard),
            Some(Key::A)
        );
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Character("A".into()), KeyLocation::Standard),
            Some(Key::A)
        );
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Character("1".into()), KeyLocation::Standard),
            Some(Key::Key1)
        );

        // Test named keys
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Named(NamedKey::Enter), KeyLocation::Standard),
            Some(Key::Enter)
        );
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Named(NamedKey::Escape), KeyLocation::Standard),
            Some(Key::Escape)
        );
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Named(NamedKey::F1), KeyLocation::Standard),
            Some(Key::F1)
        );

        // Test modifier keys with location
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Named(NamedKey::Shift), KeyLocation::Left),
            Some(Key::LeftShift)
        );
        assert_eq!(
            winit_key_to_imgui_key(&WinitKey::Named(NamedKey::Shift), KeyLocation::Right),
            Some(Key::RightShift)
        );
    }
}
