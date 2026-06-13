use super::*;

impl Ui {
    /// Returns the currently desired mouse cursor type
    ///
    /// Returns `None` if no cursor should be displayed
    #[doc(alias = "GetMouseCursor")]
    pub fn mouse_cursor(&self) -> Option<MouseCursor> {
        unsafe {
            match sys::igGetMouseCursor() {
                sys::ImGuiMouseCursor_Arrow => Some(MouseCursor::Arrow),
                sys::ImGuiMouseCursor_TextInput => Some(MouseCursor::TextInput),
                sys::ImGuiMouseCursor_ResizeAll => Some(MouseCursor::ResizeAll),
                sys::ImGuiMouseCursor_ResizeNS => Some(MouseCursor::ResizeNS),
                sys::ImGuiMouseCursor_ResizeEW => Some(MouseCursor::ResizeEW),
                sys::ImGuiMouseCursor_ResizeNESW => Some(MouseCursor::ResizeNESW),
                sys::ImGuiMouseCursor_ResizeNWSE => Some(MouseCursor::ResizeNWSE),
                sys::ImGuiMouseCursor_Hand => Some(MouseCursor::Hand),
                sys::ImGuiMouseCursor_NotAllowed => Some(MouseCursor::NotAllowed),
                _ => None,
            }
        }
    }

    /// Sets the desired mouse cursor type
    ///
    /// Passing `None` hides the mouse cursor
    #[doc(alias = "SetMouseCursor")]
    pub fn set_mouse_cursor(&self, cursor_type: Option<MouseCursor>) {
        unsafe {
            let val: sys::ImGuiMouseCursor = cursor_type
                .map(|x| x as sys::ImGuiMouseCursor)
                .unwrap_or(sys::ImGuiMouseCursor_None);
            sys::igSetMouseCursor(val);
        }
    }

    // ============================================================================
    // Focus and Navigation
    // ============================================================================

    /// Focuses keyboard on the next widget.
    ///
    /// This is the equivalent to [set_keyboard_focus_here_with_offset](Self::set_keyboard_focus_here_with_offset)
    /// with `offset` set to 0.
    #[doc(alias = "SetKeyboardFocusHere")]
    pub fn set_keyboard_focus_here(&self) {
        self.set_keyboard_focus_here_with_offset(0);
    }

    /// Focuses keyboard on a widget relative to current position.
    ///
    /// Use positive offset to focus on next widgets, negative offset to focus on previous widgets.
    #[doc(alias = "SetKeyboardFocusHere")]
    pub fn set_keyboard_focus_here_with_offset(&self, offset: i32) {
        unsafe {
            sys::igSetKeyboardFocusHere(offset);
        }
    }

    /// Shows or hides the navigation cursor (a small marker indicating nav focus).
    #[doc(alias = "SetNavCursorVisible")]
    pub fn set_nav_cursor_visible(&self, visible: bool) {
        self.run_with_bound_context(|| unsafe { sys::igSetNavCursorVisible(visible) });
    }
}
