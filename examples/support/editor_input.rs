//! Host policy for an embedded game view. Dear ImGui still receives native IO; these recipients
//! govern engine actions, and are paired across capture-intent changes until release.

use std::collections::{HashMap, HashSet};
use winit::event::{ElementState, MouseButton};
use winit::keyboard::PhysicalKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recipient {
    Ui,
    Engine,
}

#[derive(Default)]
pub struct EditorInput {
    keys: HashMap<PhysicalKey, Recipient>,
    buttons: HashMap<MouseButton, Recipient>,
    pub engine_keys: HashSet<PhysicalKey>,
    pub camera_angles: [f64; 2],
}

impl EditorInput {
    pub fn key(
        &mut self,
        key: PhysicalKey,
        state: ElementState,
        capture_keyboard: bool,
        camera_owned: bool,
    ) -> Recipient {
        let recipient = if state.is_pressed() {
            *self
                .keys
                .entry(key)
                .or_insert(if camera_owned || !capture_keyboard {
                    Recipient::Engine
                } else {
                    Recipient::Ui
                })
        } else {
            self.keys.remove(&key).unwrap_or(Recipient::Ui)
        };
        if recipient == Recipient::Engine {
            if state.is_pressed() {
                self.engine_keys.insert(key);
            } else {
                self.engine_keys.remove(&key);
            }
        }
        recipient
    }

    pub fn button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        game_view_hovered: bool,
    ) -> Recipient {
        if state.is_pressed() {
            *self.buttons.entry(button).or_insert(if game_view_hovered {
                Recipient::Engine
            } else {
                Recipient::Ui
            })
        } else {
            self.buttons.remove(&button).unwrap_or(Recipient::Ui)
        }
    }

    pub fn focus_lost(&mut self) {
        self.keys.clear();
        self.buttons.clear();
        self.engine_keys.clear();
    }

    pub fn raw_motion(&mut self, delta: (f64, f64)) {
        self.camera_angles[0] += delta.0 * 0.002;
        self.camera_angles[1] = (self.camera_angles[1] + delta.1 * 0.002).clamp(-1.5, 1.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::KeyCode;

    #[test]
    fn release_reaches_the_press_recipient_after_capture_changes() {
        let mut input = EditorInput::default();
        let key = PhysicalKey::Code(KeyCode::KeyW);
        assert_eq!(
            input.key(key, ElementState::Pressed, false, false),
            Recipient::Engine
        );
        assert_eq!(
            input.key(key, ElementState::Released, true, false),
            Recipient::Engine
        );
        assert!(input.engine_keys.is_empty());
        assert_eq!(
            input.button(MouseButton::Right, ElementState::Pressed, true),
            Recipient::Engine
        );
        assert_eq!(
            input.button(MouseButton::Right, ElementState::Released, false),
            Recipient::Engine
        );
    }
}
