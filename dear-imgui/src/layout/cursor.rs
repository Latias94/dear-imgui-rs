use super::validation::{assert_finite_f32, assert_finite_vec2};
use crate::Ui;
use crate::sys;

impl Ui {
    /// Returns the cursor position (in window coordinates)
    #[doc(alias = "GetCursorPos")]
    pub fn cursor_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorPos() };
        [pos.x, pos.y]
    }

    /// Returns the cursor position (in absolute screen coordinates)
    #[doc(alias = "GetCursorScreenPos")]
    pub fn cursor_screen_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorScreenPos() };
        [pos.x, pos.y]
    }

    /// Sets the cursor position (in window coordinates)
    #[doc(alias = "SetCursorPos")]
    pub fn set_cursor_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        assert_finite_vec2("Ui::set_cursor_pos()", "position", pos_array);
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        unsafe { sys::igSetCursorPos(pos_vec) };
    }

    /// Sets the cursor position (in absolute screen coordinates)
    #[doc(alias = "SetCursorScreenPos")]
    pub fn set_cursor_screen_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        assert_finite_vec2("Ui::set_cursor_screen_pos()", "position", pos_array);
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        unsafe { sys::igSetCursorScreenPos(pos_vec) };
    }

    /// Returns the X cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosX")]
    pub fn cursor_pos_x(&self) -> f32 {
        unsafe { sys::igGetCursorPosX() }
    }

    /// Returns the Y cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosY")]
    pub fn cursor_pos_y(&self) -> f32 {
        unsafe { sys::igGetCursorPosY() }
    }

    /// Sets the X cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosX")]
    pub fn set_cursor_pos_x(&self, x: f32) {
        assert_finite_f32("Ui::set_cursor_pos_x()", "x", x);
        unsafe { sys::igSetCursorPosX(x) };
    }

    /// Sets the Y cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosY")]
    pub fn set_cursor_pos_y(&self, y: f32) {
        assert_finite_f32("Ui::set_cursor_pos_y()", "y", y);
        unsafe { sys::igSetCursorPosY(y) };
    }

    /// Returns the initial cursor position (in window coordinates)
    #[doc(alias = "GetCursorStartPos")]
    pub fn cursor_start_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetCursorStartPos() };
        [pos.x, pos.y]
    }
}
