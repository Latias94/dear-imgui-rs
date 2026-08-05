use std::collections::HashMap;

use winit::window::WindowId;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in super::super) struct MouseLeaveState {
    buttons_down: u8,
    pending: bool,
}

impl MouseLeaveState {
    pub(in super::super) fn note_button(
        &mut self,
        button: dear_imgui_rs::input::MouseButton,
        pressed: bool,
    ) {
        let mask = 1_u8 << (button as u8);
        if pressed {
            self.buttons_down |= mask;
        } else {
            self.buttons_down &= !mask;
        }
    }

    pub(in super::super) fn note_cursor_left(&mut self) {
        self.pending = true;
    }

    pub(in super::super) fn note_cursor_available(&mut self) {
        self.pending = false;
    }

    pub(in super::super) fn note_context_focus_lost(&mut self) {
        // Winit may not deliver button releases after the pointer or keyboard focus leaves every
        // window owned by this Context. Keep the delayed-leave state recoverable in that case.
        self.buttons_down = 0;
        self.pending = true;
    }

    pub(in super::super) fn take_invalidation_due(&mut self) -> bool {
        if self.pending && self.buttons_down == 0 {
            self.pending = false;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(in super::super) struct InputOwnership {
    keys: HashMap<dear_imgui_rs::Key, WindowId>,
    mouse_buttons: HashMap<dear_imgui_rs::input::MouseButton, WindowId>,
    touch: Option<(u64, WindowId)>,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(in super::super) struct ReleasedInput {
    pub(in super::super) keys: Vec<dear_imgui_rs::Key>,
    pub(in super::super) mouse_buttons: Vec<dear_imgui_rs::input::MouseButton>,
    pub(in super::super) touch: bool,
}

impl InputOwnership {
    pub(in super::super) fn note_key(
        &mut self,
        window_id: WindowId,
        key: dear_imgui_rs::Key,
        pressed: bool,
    ) {
        if pressed {
            self.keys.insert(key, window_id);
        } else {
            self.keys.remove(&key);
        }
    }

    pub(in super::super) fn note_mouse_button(
        &mut self,
        window_id: WindowId,
        button: dear_imgui_rs::input::MouseButton,
        pressed: bool,
    ) {
        if pressed {
            self.mouse_buttons.insert(button, window_id);
        } else {
            self.mouse_buttons.remove(&button);
        }
    }

    pub(in super::super) fn note_touch(
        &mut self,
        window_id: WindowId,
        touch_id: u64,
        phase: winit::event::TouchPhase,
    ) -> Option<crate::events::TouchAction> {
        let active_id = self.touch.map(|(touch_id, _)| touch_id);
        let (next_active, action) = crate::events::touch_transition(active_id, touch_id, phase);
        match action {
            Some(crate::events::TouchAction::Press) => {
                self.touch = next_active.map(|touch_id| (touch_id, window_id));
            }
            Some(crate::events::TouchAction::Release) => self.touch = None,
            Some(crate::events::TouchAction::Move) | None => {}
        }
        action
    }

    pub(in super::super) fn retire_window(
        &mut self,
        window_id: WindowId,
        mouse_handoff: Option<WindowId>,
    ) -> ReleasedInput {
        let mut released = ReleasedInput::default();
        self.keys.retain(|key, owner| {
            if *owner == window_id {
                released.keys.push(*key);
                false
            } else {
                true
            }
        });
        self.mouse_buttons.retain(|button, owner| {
            if *owner == window_id {
                if let Some(mouse_handoff) = mouse_handoff {
                    *owner = mouse_handoff;
                    true
                } else {
                    released.mouse_buttons.push(*button);
                    false
                }
            } else {
                true
            }
        });
        if self.touch.is_some_and(|(_, owner)| owner == window_id) {
            if let Some(mouse_handoff) = mouse_handoff {
                if let Some((_, owner)) = self.touch.as_mut() {
                    *owner = mouse_handoff;
                }
            } else {
                self.touch = None;
                released.touch = true;
            }
        }
        released
    }

    pub(super) fn clear(&mut self) {
        self.keys.clear();
        self.mouse_buttons.clear();
        self.touch = None;
    }
}
