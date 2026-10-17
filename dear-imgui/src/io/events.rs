use crate::io::{Io, assert_finite_vec2};
use crate::sys;

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

    /// Add one UTF-16 code unit, preserving Dear ImGui's surrogate-pair state.
    #[doc(alias = "AddInputCharacterUTF16")]
    pub fn add_input_character_utf16(&mut self, code_unit: u16) {
        unsafe {
            sys::ImGuiIO_AddInputCharacterUTF16(self.inner_mut() as *mut _, code_unit);
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
}

fn is_non_modifier_key(key: sys::ImGuiKey) -> bool {
    // These native predicates exclude mouse aliases and aggregate modifier storage.
    unsafe { (sys::igIsKeyboardKey(key) || sys::igIsGamepadKey(key)) && !sys::igIsLRModKey(key) }
}

#[cfg(test)]
mod tests {
    use crate::sys;

    #[test]
    fn non_modifier_key_reset_preserves_text_mouse_and_native_modifiers() {
        let mut context = crate::Context::create();
        context.io_mut().set_config_input_trickle_event_queue(false);
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        for macos_behaviors in [false, true] {
            context
                .io_mut()
                .set_config_macosx_behaviors(macos_behaviors);
            context.io_mut().add_key_event(crate::Key::ModCtrl, true);
            context.io_mut().add_key_event(crate::Key::LeftCtrl, true);
            context.io_mut().add_key_event(crate::Key::Backspace, true);
            context.io_mut().add_input_characters_utf8("甲");
            context
                .io_mut()
                .add_mouse_button_event(crate::input::MouseButton::Left, true);
            let _ = context.frame();
            assert!(context.io().inner().KeysData[key_data_index(crate::Key::Backspace)].Down);
            let modifiers = context.io().inner().KeyMods;
            assert_ne!(modifiers, 0);
            let characters = context.io().inner().InputQueueCharacters.Size;
            assert_eq!(characters, 1);
            // macOS maps Ctrl+left-click to right-click; preserve the native result.
            let mouse_down = context.io().inner().MouseDown;
            assert!(mouse_down.iter().any(|&down| down));

            context.io_mut().add_key_event(crate::Key::Delete, true);
            context.io_mut().add_key_event(crate::Key::ModShift, true);
            context.io_mut().add_input_characters_utf8("乙");
            context.io_mut().add_mouse_wheel_event([0.0, 1.0]);
            context.io_mut().clear_non_modifier_keys();
            assert!(!context.io().inner().KeysData[key_data_index(crate::Key::Backspace)].Down);
            assert_eq!(context.io().inner().KeyMods, modifiers);
            assert_eq!(context.io().inner().InputQueueCharacters.Size, characters);
            assert_eq!(context.io().inner().MouseDown, mouse_down);
            let _ = context.render_legacy();

            let ui = context.frame();
            assert!(!ui.is_key_down(crate::Key::Backspace));
            assert!(!ui.is_key_down(crate::Key::Delete));
            assert!(ui.is_key_down(if macos_behaviors {
                crate::Key::LeftSuper
            } else {
                crate::Key::LeftCtrl
            }));
            assert!(ui.io().key_shift());
            assert_eq!(ui.io().key_super(), macos_behaviors);
            assert_eq!(ui.io().key_ctrl(), !macos_behaviors);
            assert_eq!(ui.io().inner().MouseDown, mouse_down);
            assert_eq!(ui.io().inner().MouseWheel, 1.0);
            let characters = &ui.io().inner().InputQueueCharacters;
            assert_eq!(characters.Size, 1);
            assert_eq!(unsafe { *characters.Data }, '乙' as u32);
            let _ = context.render_legacy();
            context.io_mut().clear_input_keys();
            context.io_mut().clear_input_mouse();
        }
    }

    #[test]
    fn extended_input_events_update_and_clear_context_state() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();

        ctx.io_mut().set_app_accepting_events(true);
        ctx.io_mut().add_key_analog_event(crate::Key::A, true, 0.5);
        ctx.io_mut()
            .set_key_event_native_data(crate::Key::A, 65, 30, Some(65));
        assert!(ctx.frame().is_key_down(crate::Key::A));
        ctx.io_mut().clear_input_keys();
        assert!(!ctx.io().inner().KeysData[key_data_index(crate::Key::A)].Down);
        let _ = ctx.render_legacy();

        ctx.io_mut().add_key_event(crate::Key::A, true);
        ctx.io_mut().clear_events_queue();
        assert!(!ctx.frame().is_key_down(crate::Key::A));
        let _ = ctx.render_legacy();

        ctx.io_mut().set_app_accepting_events(false);
        ctx.io_mut().add_key_event(crate::Key::A, true);
        assert!(!ctx.frame().is_key_down(crate::Key::A));
        let _ = ctx.render_legacy();

        ctx.io_mut().set_app_accepting_events(true);
        ctx.io_mut().add_input_character_utf16(b'A' as u16);
        ctx.io_mut().add_input_characters_utf8("B界");
        let _ = ctx.frame();
        assert_eq!(ctx.io().inner().InputQueueCharacters.Size, 3);
        ctx.io_mut().clear_input_keys();
        assert_eq!(ctx.io().inner().InputQueueCharacters.Size, 0);

        ctx.io_mut().inner_mut().MouseDown[0] = true;
        ctx.io_mut().clear_input_mouse();
        assert!(!ctx.io().inner().MouseDown[0]);
        let _ = ctx.render_legacy();
    }

    fn key_data_index(key: crate::Key) -> usize {
        usize::try_from(key as sys::ImGuiKey - sys::ImGuiKey_NamedKey_BEGIN).unwrap()
    }

    #[test]
    fn extended_input_events_validate_values_before_ffi() {
        let mut ctx = crate::Context::create();
        let io = ctx.io_mut();

        for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    io.add_key_analog_event(crate::Key::GamepadL2, true, value);
                }))
                .is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                io.set_key_event_native_data(crate::Key::ModCtrl, 0, 0, None);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                io.set_key_event_native_data(crate::Key::A, 0, 0, Some(512));
            }))
            .is_err()
        );
    }
}
