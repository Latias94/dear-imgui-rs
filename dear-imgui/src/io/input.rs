use crate::input::MouseButton;
use crate::sys;
use crate::validation::{assert_finite_f32, assert_finite_vec2};

use super::Io;

const MOUSE_BUTTON_COUNT: usize = 5;

fn mouse_button_index(button: MouseButton) -> usize {
    button as i32 as usize
}

impl Io {
    /// Add a key event to the input queue.
    #[doc(alias = "AddKeyEvent")]
    pub fn add_key_event(&mut self, key: crate::Key, down: bool) {
        unsafe {
            sys::ImGuiIO_AddKeyEvent(self.inner_mut() as *mut _, key.into(), down);
        }
    }

    /// Add a key or gamepad event with an analog value in `0.0..=1.0`.
    #[doc(alias = "AddKeyAnalogEvent")]
    pub fn add_key_analog_event(&mut self, key: crate::Key, down: bool, value: f32) {
        assert!(
            value.is_finite() && (0.0..=1.0).contains(&value),
            "Io::add_key_analog_event() value must be finite and in 0.0..=1.0"
        );
        unsafe {
            sys::ImGuiIO_AddKeyAnalogEvent(self.inner_mut() as *mut _, key.into(), down, value);
        }
    }

    /// Add a character input event to the input queue.
    #[doc(alias = "AddInputCharacter")]
    pub fn add_input_character(&mut self, character: char) {
        unsafe {
            sys::ImGuiIO_AddInputCharacter(self.inner_mut() as *mut _, character as u32);
        }
    }

    /// Add one UTF-16 code unit, combining surrogate pairs across calls.
    ///
    /// Malformed surrogate sequences emit U+FFFD. When input events are paused,
    /// the code unit is ignored and any pending high surrogate is preserved.
    /// Pending state lives in the native IO object and follows its lifetime.
    #[doc(alias = "AddInputCharacterUTF16")]
    pub fn add_input_character_utf16(&mut self, code_unit: u16) {
        let io = self.inner_mut();
        if !io.AppAcceptingEvents {
            return;
        }

        let pending = std::mem::take(&mut io.InputQueueSurrogate);
        if (0xd800..=0xdbff).contains(&code_unit) {
            io.InputQueueSurrogate = code_unit;
            if pending != 0 {
                self.add_input_character(char::REPLACEMENT_CHARACTER);
            }
            return;
        }

        let units = [pending, code_unit];
        let start = usize::from(pending == 0);
        for decoded in char::decode_utf16(units[start..].iter().copied()) {
            let character = decoded.unwrap_or(char::REPLACEMENT_CHARACTER);
            if character != '\0' {
                self.add_input_character(character);
            }
        }
    }

    /// Add all Unicode scalar values from a UTF-8 Rust string.
    #[doc(alias = "AddInputCharactersUTF8")]
    pub fn add_input_characters_utf8(&mut self, text: impl AsRef<str>) {
        for character in text.as_ref().chars() {
            self.add_input_character(character);
        }
    }

    /// Add a mouse position event to the input queue.
    #[doc(alias = "AddMousePosEvent")]
    pub fn add_mouse_pos_event(&mut self, pos: [f32; 2]) {
        assert_finite_vec2("Io::add_mouse_pos_event()", "pos", pos);
        unsafe {
            sys::ImGuiIO_AddMousePosEvent(self.inner_mut() as *mut _, pos[0], pos[1]);
        }
    }

    /// Add a mouse button event to the input queue.
    #[doc(alias = "AddMouseButtonEvent")]
    pub fn add_mouse_button_event(&mut self, button: crate::input::MouseButton, down: bool) {
        unsafe {
            sys::ImGuiIO_AddMouseButtonEvent(self.inner_mut() as *mut _, button.into(), down);
        }
    }

    /// Add a mouse wheel event to the input queue.
    #[doc(alias = "AddMouseWheelEvent")]
    pub fn add_mouse_wheel_event(&mut self, wheel: [f32; 2]) {
        assert_finite_vec2("Io::add_mouse_wheel_event()", "wheel", wheel);
        unsafe {
            sys::ImGuiIO_AddMouseWheelEvent(self.inner_mut() as *mut _, wheel[0], wheel[1]);
        }
    }

    /// Add a mouse source event to the input queue.
    ///
    /// Backends should call this before other mouse events when switching
    /// between mouse, touch-screen, and pen input.
    #[doc(alias = "AddMouseSourceEvent")]
    pub fn add_mouse_source_event(&mut self, source: crate::input::MouseSource) {
        unsafe {
            sys::ImGuiIO_AddMouseSourceEvent(self.inner_mut() as *mut _, source.into());
        }
    }

    /// Queue the hovered viewport ID for the current frame.
    ///
    /// Backends should also set `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT`.
    #[doc(alias = "AddMouseViewportEvent")]
    pub fn add_mouse_viewport_event(&mut self, viewport_id: crate::Id) {
        unsafe {
            sys::ImGuiIO_AddMouseViewportEvent(self.inner_mut() as *mut _, viewport_id.raw());
        }
    }

    /// Notify Dear ImGui that the application gained or lost focus.
    #[doc(alias = "AddFocusEvent")]
    pub fn add_focus_event(&mut self, focused: bool) {
        unsafe {
            sys::ImGuiIO_AddFocusEvent(self.inner_mut() as *mut _, focused);
        }
    }

    /// Store native key metadata for legacy backend interoperability.
    ///
    /// `native_legacy_index`, when present, must be in `0..512`.
    #[doc(alias = "SetKeyEventNativeData")]
    pub fn set_key_event_native_data(
        &mut self,
        key: crate::Key,
        native_keycode: i32,
        native_scancode: i32,
        native_legacy_index: Option<i32>,
    ) {
        let key: sys::ImGuiKey = key.into();
        assert!(
            (sys::ImGuiKey_NamedKey_BEGIN..sys::ImGuiKey_NamedKey_END).contains(&key),
            "Io::set_key_event_native_data() key must be a named non-modifier key"
        );
        let native_legacy_index = native_legacy_index.unwrap_or(-1);
        assert!(
            native_legacy_index == -1
                || (0..sys::ImGuiKey_NamedKey_BEGIN).contains(&native_legacy_index),
            "Io::set_key_event_native_data() native_legacy_index must be in 0..512"
        );
        unsafe {
            sys::ImGuiIO_SetKeyEventNativeData(
                self.inner_mut() as *mut _,
                key,
                native_keycode,
                native_scancode,
                native_legacy_index,
            );
        }
    }

    /// Enable or pause acceptance of queued keyboard, mouse, and text events.
    #[doc(alias = "SetAppAcceptingEvents")]
    pub fn set_app_accepting_events(&mut self, accepting_events: bool) {
        unsafe {
            sys::ImGuiIO_SetAppAcceptingEvents(self.inner_mut() as *mut _, accepting_events);
        }
    }

    /// Clear all pending input events that have not been processed by a frame.
    #[doc(alias = "ClearEventsQueue")]
    pub fn clear_events_queue(&mut self) {
        unsafe { sys::ImGuiIO_ClearEventsQueue(self.inner_mut() as *mut _) }
    }

    /// Release all keyboard/gamepad state and clear queued text characters.
    #[doc(alias = "ClearInputKeys")]
    pub fn clear_input_keys(&mut self) {
        unsafe { sys::ImGuiIO_ClearInputKeys(self.inner_mut() as *mut _) }
    }

    /// Release held keyboard/gamepad keys and discard their pending key events.
    ///
    /// Physical and aggregate modifiers retain their state and pending events.
    /// Text, mouse, focus events, and current-frame text characters are preserved.
    /// Platform backends can use this when an IME takes ownership of editing keys.
    pub fn clear_non_modifier_keys(&mut self) {
        let io = self.inner_mut();
        for (index, data) in io.KeysData.iter_mut().enumerate() {
            let key = sys::ImGuiKey_NamedKey_BEGIN + index as sys::ImGuiKey;
            if is_non_modifier_key(key) {
                data.Down = false;
                data.DownDuration = -1.0;
                data.DownDurationPrev = -1.0;
                data.AnalogValue = 0.0;
            }
        }

        // SAFETY: Io belongs to a live Context. Its input queue is an ImVector of
        // Copy events, uniquely borrowed here; compaction keeps its allocation intact.
        let queue = unsafe {
            crate::internal::imvector_cast_mut::<sys::ImGuiInputEvent, _>(
                &mut (*io.Ctx).InputEventsQueue,
            )
        };
        let events = queue.as_slice_mut();
        let mut retained = 0;
        for index in 0..events.len() {
            let event = events[index];
            // SAFETY: the Key union member is initialized only for a Key event.
            if event.Type == sys::ImGuiInputEventType_Key
                && is_non_modifier_key(unsafe { event.__bindgen_anon_1.Key.Key })
            {
                continue;
            }
            events[retained] = event;
            retained += 1;
        }
        // The retained count cannot exceed the original ImVector's i32 size.
        queue.size = retained as i32;
    }

    /// Release all mouse buttons and reset mouse position and wheel state.
    #[doc(alias = "ClearInputMouse")]
    pub fn clear_input_mouse(&mut self) {
        unsafe { sys::ImGuiIO_ClearInputMouse(self.inner_mut() as *mut _) }
    }

    // Current input and capture state

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

    /// Check if a mouse button is down.
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        self.mouse_down_raw_index(mouse_button_index(button))
    }

    /// Check if a raw mouse button index is down.
    ///
    /// This is an explicit escape hatch for low-level adapters that need to
    /// inspect Dear ImGui's raw `MouseDown` array. Out-of-range indices return
    /// `false`, matching the previous `mouse_down(usize)` behavior.
    pub fn mouse_down_raw_index(&self, button: usize) -> bool {
        if button < MOUSE_BUTTON_COUNT {
            self.inner().MouseDown[button]
        } else {
            false
        }
    }

    /// Check if a mouse button is down.
    pub fn mouse_down_button(&self, button: MouseButton) -> bool {
        self.mouse_down(button)
    }

    /// Set mouse button state.
    pub fn set_mouse_down(&mut self, button: MouseButton, down: bool) {
        self.set_mouse_down_raw_index(mouse_button_index(button), down);
    }

    /// Set a raw mouse button index state.
    ///
    /// This is an explicit escape hatch for low-level adapters that need to
    /// write Dear ImGui's raw `MouseDown` array. Out-of-range indices are
    /// ignored, matching the previous `set_mouse_down(usize, bool)` behavior.
    pub fn set_mouse_down_raw_index(&mut self, button: usize, down: bool) {
        if button < MOUSE_BUTTON_COUNT {
            self.inner_mut().MouseDown[button] = down;
        }
    }

    /// Set mouse button state.
    pub fn set_mouse_down_button(&mut self, button: MouseButton, down: bool) {
        self.set_mouse_down(button, down);
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

    /// Returns the mouse delta from the previous frame, in pixels.
    #[doc(alias = "MouseDelta")]
    pub fn mouse_delta(&self) -> [f32; 2] {
        let delta = self.inner().MouseDelta;
        [delta.x, delta.y]
    }

    /// Returns whether Ctrl modifier is held.
    #[doc(alias = "KeyCtrl")]
    pub fn key_ctrl(&self) -> bool {
        self.inner().KeyCtrl
    }

    /// Returns whether Shift modifier is held.
    #[doc(alias = "KeyShift")]
    pub fn key_shift(&self) -> bool {
        self.inner().KeyShift
    }

    /// Returns whether Alt modifier is held.
    #[doc(alias = "KeyAlt")]
    pub fn key_alt(&self) -> bool {
        self.inner().KeyAlt
    }

    /// Returns whether Super/Command modifier is held.
    #[doc(alias = "KeySuper")]
    pub fn key_super(&self) -> bool {
        self.inner().KeySuper
    }

    /// Returns the current mouse input source.
    #[doc(alias = "MouseSource")]
    pub fn mouse_source(&self) -> crate::input::MouseSource {
        match self.inner().MouseSource {
            sys::ImGuiMouseSource_Mouse => crate::input::MouseSource::Mouse,
            sys::ImGuiMouseSource_TouchScreen => crate::input::MouseSource::TouchScreen,
            sys::ImGuiMouseSource_Pen => crate::input::MouseSource::Pen,
            _ => crate::input::MouseSource::Mouse,
        }
    }

    /// Returns the viewport id hovered by the OS mouse (if supported by the backend).
    #[doc(alias = "MouseHoveredViewport")]
    pub fn mouse_hovered_viewport(&self) -> crate::Id {
        crate::Id::from(self.inner().MouseHoveredViewport)
    }

    /// Set the viewport id hovered by the OS mouse.
    #[doc(alias = "MouseHoveredViewport")]
    pub fn set_mouse_hovered_viewport(&mut self, viewport_id: crate::Id) {
        self.inner_mut().MouseHoveredViewport = viewport_id.raw();
    }

    /// Returns whether Ctrl+LeftClick should be treated as RightClick.
    #[doc(alias = "MouseCtrlLeftAsRightClick")]
    pub fn mouse_ctrl_left_as_right_click(&self) -> bool {
        self.inner().MouseCtrlLeftAsRightClick
    }

    /// Set whether Ctrl+LeftClick should be treated as RightClick.
    #[doc(alias = "MouseCtrlLeftAsRightClick")]
    pub fn set_mouse_ctrl_left_as_right_click(&mut self, enabled: bool) {
        self.inner_mut().MouseCtrlLeftAsRightClick = enabled;
    }
}

fn is_non_modifier_key(key: sys::ImGuiKey) -> bool {
    // These native predicates exclude mouse aliases and aggregate modifier storage.
    unsafe { (sys::igIsKeyboardKey(key) || sys::igIsGamepadKey(key)) && !sys::igIsLRModKey(key) }
}
