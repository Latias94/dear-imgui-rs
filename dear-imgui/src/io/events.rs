use crate::io::{Io, assert_finite_vec2};
use crate::sys;

impl Io {
    /// Add a key event to the input queue
    pub fn add_key_event(&mut self, key: crate::Key, down: bool) {
        unsafe {
            sys::ImGuiIO_AddKeyEvent(self.inner_mut() as *mut _, key.into(), down);
        }
    }

    /// Add a character input event to the input queue
    pub fn add_input_character(&mut self, character: char) {
        unsafe {
            sys::ImGuiIO_AddInputCharacter(self.inner_mut() as *mut _, character as u32);
        }
    }

    /// Add a mouse position event to the input queue
    pub fn add_mouse_pos_event(&mut self, pos: [f32; 2]) {
        assert_finite_vec2("Io::add_mouse_pos_event()", "pos", pos);
        unsafe {
            sys::ImGuiIO_AddMousePosEvent(self.inner_mut() as *mut _, pos[0], pos[1]);
        }
    }

    /// Add a mouse button event to the input queue
    pub fn add_mouse_button_event(&mut self, button: crate::input::MouseButton, down: bool) {
        unsafe {
            sys::ImGuiIO_AddMouseButtonEvent(self.inner_mut() as *mut _, button.into(), down);
        }
    }

    /// Add a mouse wheel event to the input queue
    pub fn add_mouse_wheel_event(&mut self, wheel: [f32; 2]) {
        assert_finite_vec2("Io::add_mouse_wheel_event()", "wheel", wheel);
        unsafe {
            sys::ImGuiIO_AddMouseWheelEvent(self.inner_mut() as *mut _, wheel[0], wheel[1]);
        }
    }

    /// Add a mouse source event to the input queue.
    ///
    /// When the input source switches between mouse / touch screen / pen,
    /// backends should call this before submitting other mouse events for
    /// the frame.
    pub fn add_mouse_source_event(&mut self, source: crate::input::MouseSource) {
        unsafe {
            sys::ImGuiIO_AddMouseSourceEvent(self.inner_mut() as *mut _, source.into());
        }
    }

    /// Queue the hovered viewport id for the current frame.
    ///
    /// When multi-viewport is enabled and the backend can reliably obtain
    /// the ImGui viewport hovered by the OS mouse, it should set
    /// `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT` and call this once per
    /// frame.
    pub fn add_mouse_viewport_event(&mut self, viewport_id: crate::Id) {
        unsafe {
            sys::ImGuiIO_AddMouseViewportEvent(self.inner_mut() as *mut _, viewport_id.raw());
        }
    }

    /// Notify Dear ImGui that the application window gained or lost focus
    /// This mirrors `io.AddFocusEvent()` in Dear ImGui and is used by platform backends.
    pub fn add_focus_event(&mut self, focused: bool) {
        unsafe {
            sys::ImGuiIO_AddFocusEvent(self.inner_mut() as *mut _, focused);
        }
    }
}
