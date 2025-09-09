//! Cursor management for Dear ImGui winit backend
//!
//! This module handles mouse cursor type mapping and caching to avoid
//! unnecessary system calls when changing cursor appearance.

use dear_imgui::MouseCursor;
use winit::window::{CursorIcon as WinitCursor, Window};

/// Cursor settings cache to avoid unnecessary cursor changes
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CursorSettings {
    pub cursor: Option<MouseCursor>,
    pub draw_cursor: bool,
}

impl CursorSettings {
    /// Apply cursor settings to the window
    pub fn apply(&self, window: &Window) {
        match self.cursor {
            Some(mouse_cursor) if !self.draw_cursor => {
                window.set_cursor_visible(true);
                window.set_cursor(to_winit_cursor(mouse_cursor));
            }
            _ => {
                window.set_cursor_visible(!self.draw_cursor);
            }
        }
    }
}

/// Convert Dear ImGui mouse cursor to winit cursor
pub fn to_winit_cursor(cursor: MouseCursor) -> WinitCursor {
    match cursor {
        MouseCursor::None => WinitCursor::Default, // Default cursor when none specified
        MouseCursor::Arrow => WinitCursor::Default,
        MouseCursor::TextInput => WinitCursor::Text,
        MouseCursor::ResizeAll => WinitCursor::Move,
        MouseCursor::ResizeNS => WinitCursor::NsResize,
        MouseCursor::ResizeEW => WinitCursor::EwResize,
        MouseCursor::ResizeNESW => WinitCursor::NeswResize,
        MouseCursor::ResizeNWSE => WinitCursor::NwseResize,
        MouseCursor::Hand => WinitCursor::Pointer,
        MouseCursor::NotAllowed => WinitCursor::NotAllowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_mapping() {
        // Test basic cursor mappings
        assert_eq!(to_winit_cursor(MouseCursor::Arrow), WinitCursor::Default);
        assert_eq!(to_winit_cursor(MouseCursor::TextInput), WinitCursor::Text);
        assert_eq!(to_winit_cursor(MouseCursor::Hand), WinitCursor::Pointer);
        assert_eq!(to_winit_cursor(MouseCursor::NotAllowed), WinitCursor::NotAllowed);
        
        // Test resize cursors
        assert_eq!(to_winit_cursor(MouseCursor::ResizeNS), WinitCursor::NsResize);
        assert_eq!(to_winit_cursor(MouseCursor::ResizeEW), WinitCursor::EwResize);
        assert_eq!(to_winit_cursor(MouseCursor::ResizeNESW), WinitCursor::NeswResize);
        assert_eq!(to_winit_cursor(MouseCursor::ResizeNWSE), WinitCursor::NwseResize);
    }

    #[test]
    fn test_cursor_settings_equality() {
        let settings1 = CursorSettings {
            cursor: Some(MouseCursor::Arrow),
            draw_cursor: false,
        };
        
        let settings2 = CursorSettings {
            cursor: Some(MouseCursor::Arrow),
            draw_cursor: false,
        };
        
        let settings3 = CursorSettings {
            cursor: Some(MouseCursor::Hand),
            draw_cursor: false,
        };
        
        assert_eq!(settings1, settings2);
        assert_ne!(settings1, settings3);
    }
}
