use super::counts::non_negative_count_from_i32;
use super::validation::assert_finite_f32;
use crate::input::{Key, MouseButton};
use crate::sys;

impl crate::ui::Ui {
    /// Returns the number of times the key was pressed in the current frame
    #[doc(alias = "GetKeyPressedAmount")]
    pub fn get_key_pressed_amount(&self, key: Key, repeat_delay: f32, rate: f32) -> usize {
        assert_finite_f32("Ui::get_key_pressed_amount()", "repeat_delay", repeat_delay);
        assert_finite_f32("Ui::get_key_pressed_amount()", "rate", rate);
        non_negative_count_from_i32("Ui::get_key_pressed_amount()", unsafe {
            sys::igGetKeyPressedAmount(key.into(), repeat_delay, rate)
        })
    }

    /// Returns the name of a key
    #[doc(alias = "GetKeyName")]
    pub fn get_key_name(&self, key: Key) -> &str {
        unsafe {
            let name_ptr = sys::igGetKeyName(key.into());
            if name_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(name_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }

    /// Returns the number of times the mouse button was clicked in the current frame
    #[doc(alias = "GetMouseClickedCount")]
    pub fn get_mouse_clicked_count(&self, button: MouseButton) -> usize {
        non_negative_count_from_i32("Ui::get_mouse_clicked_count()", unsafe {
            sys::igGetMouseClickedCount(button.into())
        })
    }

    /// Returns the mouse position in screen coordinates
    #[doc(alias = "GetMousePos")]
    pub fn get_mouse_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetMousePos() };
        [pos.x, pos.y]
    }

    /// Returns the mouse position when the button was clicked
    #[doc(alias = "GetMousePosOnOpeningCurrentPopup")]
    pub fn get_mouse_pos_on_opening_current_popup(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetMousePosOnOpeningCurrentPopup() };
        [pos.x, pos.y]
    }

    /// Returns the mouse drag delta
    #[doc(alias = "GetMouseDragDelta")]
    pub fn get_mouse_drag_delta(&self, button: MouseButton, lock_threshold: f32) -> [f32; 2] {
        assert_finite_f32(
            "Ui::get_mouse_drag_delta()",
            "lock_threshold",
            lock_threshold,
        );
        let delta = unsafe { sys::igGetMouseDragDelta(button.into(), lock_threshold) };
        [delta.x, delta.y]
    }

    /// Returns the mouse wheel delta
    #[doc(alias = "GetIO")]
    pub fn get_mouse_wheel(&self) -> f32 {
        self.io().mouse_wheel()
    }

    /// Returns the horizontal mouse wheel delta
    #[doc(alias = "GetIO")]
    pub fn get_mouse_wheel_h(&self) -> f32 {
        self.io().mouse_wheel_h()
    }

    /// Returns `true` if any mouse button is down
    #[doc(alias = "IsAnyMouseDown")]
    pub fn is_any_mouse_down(&self) -> bool {
        unsafe { sys::igIsAnyMouseDown() }
    }
}
