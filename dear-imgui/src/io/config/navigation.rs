use super::*;

impl Io {
    /// Returns whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn config_nav_swap_gamepad_buttons(&self) -> bool {
        self.inner().ConfigNavSwapGamepadButtons
    }

    /// Set whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn set_config_nav_swap_gamepad_buttons(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavSwapGamepadButtons = enabled;
    }

    /// Returns whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn config_nav_move_set_mouse_pos(&self) -> bool {
        self.inner().ConfigNavMoveSetMousePos
    }

    /// Set whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn set_config_nav_move_set_mouse_pos(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavMoveSetMousePos = enabled;
    }

    /// Returns whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn config_nav_capture_keyboard(&self) -> bool {
        self.inner().ConfigNavCaptureKeyboard
    }

    /// Set whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn set_config_nav_capture_keyboard(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCaptureKeyboard = enabled;
    }

    /// Returns whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn config_nav_escape_clear_focus_item(&self) -> bool {
        self.inner().ConfigNavEscapeClearFocusItem
    }

    /// Set whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn set_config_nav_escape_clear_focus_item(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavEscapeClearFocusItem = enabled;
    }

    /// Returns whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn config_nav_escape_clear_focus_window(&self) -> bool {
        self.inner().ConfigNavEscapeClearFocusWindow
    }

    /// Set whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn set_config_nav_escape_clear_focus_window(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavEscapeClearFocusWindow = enabled;
    }

    /// Returns whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn config_nav_cursor_visible_auto(&self) -> bool {
        self.inner().ConfigNavCursorVisibleAuto
    }

    /// Set whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn set_config_nav_cursor_visible_auto(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCursorVisibleAuto = enabled;
    }

    /// Returns whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn config_nav_cursor_visible_always(&self) -> bool {
        self.inner().ConfigNavCursorVisibleAlways
    }

    /// Set whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn set_config_nav_cursor_visible_always(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCursorVisibleAlways = enabled;
    }
}
