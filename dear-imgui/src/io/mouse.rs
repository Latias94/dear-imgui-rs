use crate::io::{Io, assert_finite_f32, assert_finite_vec2};

impl Io {
    /// Mouse position, in pixels
    pub fn mouse_pos(&self) -> [f32; 2] {
        [self.inner().MousePos.x, self.inner().MousePos.y]
    }

    /// Set mouse position, in pixels
    pub fn set_mouse_pos(&mut self, pos: [f32; 2]) {
        assert_finite_vec2("Io::set_mouse_pos()", "pos", pos);
        self.inner_mut().MousePos.x = pos[0];
        self.inner_mut().MousePos.y = pos[1];
    }

    /// Mouse wheel vertical scrolling
    pub fn mouse_wheel(&self) -> f32 {
        self.inner().MouseWheel
    }

    /// Set mouse wheel vertical scrolling
    pub fn set_mouse_wheel(&mut self, wheel: f32) {
        assert_finite_f32("Io::set_mouse_wheel()", "wheel", wheel);
        self.inner_mut().MouseWheel = wheel;
    }

    /// Mouse wheel horizontal scrolling
    pub fn mouse_wheel_h(&self) -> f32 {
        self.inner().MouseWheelH
    }

    /// Set mouse wheel horizontal scrolling
    pub fn set_mouse_wheel_h(&mut self, wheel_h: f32) {
        assert_finite_f32("Io::set_mouse_wheel_h()", "wheel_h", wheel_h);
        self.inner_mut().MouseWheelH = wheel_h;
    }

    /// Check if a mouse button is down
    pub fn mouse_down(&self, button: usize) -> bool {
        if button < 5 {
            self.inner().MouseDown[button]
        } else {
            false
        }
    }

    /// Check if a mouse button is down (typed).
    pub fn mouse_down_button(&self, button: crate::input::MouseButton) -> bool {
        self.mouse_down(button as i32 as usize)
    }

    /// Set mouse button state
    pub fn set_mouse_down(&mut self, button: usize, down: bool) {
        if button < 5 {
            self.inner_mut().MouseDown[button] = down;
        }
    }

    /// Set mouse button state (typed).
    pub fn set_mouse_down_button(&mut self, button: crate::input::MouseButton, down: bool) {
        self.set_mouse_down(button as i32 as usize, down);
    }

    /// Check if imgui wants to capture mouse input
    pub fn want_capture_mouse(&self) -> bool {
        self.inner().WantCaptureMouse
    }

    /// Returns whether ImGui wants to capture mouse, unless a popup is closing.
    #[doc(alias = "WantCaptureMouseUnlessPopupClose")]
    pub fn want_capture_mouse_unless_popup_close(&self) -> bool {
        self.inner().WantCaptureMouseUnlessPopupClose
    }

    /// Check if imgui wants to capture keyboard input
    pub fn want_capture_keyboard(&self) -> bool {
        self.inner().WantCaptureKeyboard
    }

    /// Check if imgui wants to use text input
    pub fn want_text_input(&self) -> bool {
        self.inner().WantTextInput
    }

    /// Check if imgui wants to set mouse position
    pub fn want_set_mouse_pos(&self) -> bool {
        self.inner().WantSetMousePos
    }
    /// Whether ImGui requests software-drawn mouse cursor
    pub fn mouse_draw_cursor(&self) -> bool {
        self.inner().MouseDrawCursor
    }
    /// Enable or disable software-drawn mouse cursor
    pub fn set_mouse_draw_cursor(&mut self, draw: bool) {
        self.inner_mut().MouseDrawCursor = draw;
    }

    /// Check if imgui wants to save ini settings
    pub fn want_save_ini_settings(&self) -> bool {
        self.inner().WantSaveIniSettings
    }
}
