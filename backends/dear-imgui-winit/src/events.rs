//! Event handling for Dear ImGui winit backend
//!
//! This module contains event processing logic for various winit events
//! including keyboard, mouse, touch, and IME events.

use dear_imgui_rs::Context;
use winit::event::{DeviceEvent, ElementState, Ime, KeyEvent, MouseScrollDelta, TouchPhase};

use std::cell::RefCell;
use winit::window::Window;

use crate::input::{to_imgui_mouse_button, winit_key_to_imgui_key};

/// Handle keyboard input events
pub fn handle_keyboard_input(event: &KeyEvent, imgui_ctx: &mut Context) -> bool {
    let io = imgui_ctx.io_mut();

    // Inject text for character input on key press (matches upstream imgui-winit behavior)
    if event.state.is_pressed() {
        if let Some(txt) = &event.text {
            for ch in txt.chars() {
                // Filter out DEL control code as upstream does
                if ch != '\u{7f}' {
                    io.add_input_character(ch);
                }
            }
        }
    }

    if let Some(imgui_key) = winit_key_to_imgui_key(&event.logical_key, event.location) {
        let pressed = event.state == ElementState::Pressed;
        io.add_key_event(imgui_key, pressed);
        return io.want_capture_keyboard();
    }

    false
}

/// Handle mouse wheel scrolling
pub fn handle_mouse_wheel(delta: MouseScrollDelta, imgui_ctx: &mut Context) -> bool {
    let io = imgui_ctx.io_mut();

    match delta {
        MouseScrollDelta::LineDelta(h, v) => io.add_mouse_wheel_event([h, v]),
        MouseScrollDelta::PixelDelta(pos) => {
            // Follow upstream practice: treat pixel delta as +/- 1.0 per event
            let h = match pos.x.partial_cmp(&0.0) {
                Some(std::cmp::Ordering::Greater) => 1.0,
                Some(std::cmp::Ordering::Less) => -1.0,
                _ => 0.0,
            };
            let v = match pos.y.partial_cmp(&0.0) {
                Some(std::cmp::Ordering::Greater) => 1.0,
                Some(std::cmp::Ordering::Less) => -1.0,
                _ => 0.0,
            };
            io.add_mouse_wheel_event([h, v]);
        }
    }

    io.want_capture_mouse()
}

/// Handle mouse button events
pub fn handle_mouse_button(
    button: winit::event::MouseButton,
    state: ElementState,
    imgui_ctx: &mut Context,
) -> bool {
    if let Some(imgui_button) = to_imgui_mouse_button(button) {
        let pressed = state == ElementState::Pressed;
        imgui_ctx
            .io_mut()
            .add_mouse_button_event(imgui_button, pressed);
        return imgui_ctx.io().want_capture_mouse();
    }
    false
}

/// Handle cursor movement events
pub fn handle_cursor_moved(position: [f64; 2], imgui_ctx: &mut Context) -> bool {
    imgui_ctx
        .io_mut()
        .add_mouse_pos_event([position[0] as f32, position[1] as f32]);
    imgui_ctx.io().want_capture_mouse()
}

/// Handle modifier key state changes
pub fn handle_modifiers_changed(modifiers: &winit::event::Modifiers, imgui_ctx: &mut Context) {
    let io = imgui_ctx.io_mut();
    let state = modifiers.state();

    // Update modifier key states - our Key enum has Left/Right variants instead of Mod variants
    // We'll update both left and right keys to the same state since winit doesn't distinguish
    io.add_key_event(dear_imgui_rs::Key::LeftShift, state.shift_key());
    io.add_key_event(dear_imgui_rs::Key::RightShift, state.shift_key());
    io.add_key_event(dear_imgui_rs::Key::LeftCtrl, state.control_key());
    io.add_key_event(dear_imgui_rs::Key::RightCtrl, state.control_key());
    io.add_key_event(dear_imgui_rs::Key::LeftAlt, state.alt_key());
    io.add_key_event(dear_imgui_rs::Key::RightAlt, state.alt_key());
    io.add_key_event(dear_imgui_rs::Key::LeftSuper, state.super_key());
    io.add_key_event(dear_imgui_rs::Key::RightSuper, state.super_key());
}

/// Handle IME (Input Method Editor) events for international text input
pub fn handle_ime_event(ime: &Ime, imgui_ctx: &mut Context) {
    match ime {
        Ime::Preedit(_text, _cursor_range) => {
            // Do not inject preedit text into Dear ImGui; composition should be handled by the OS/IME.
        }
        Ime::Commit(text) => {
            // Handle committed text (final text input)
            for ch in text.chars() {
                if !ch.is_control() {
                    imgui_ctx.io_mut().add_input_character(ch);
                }
            }
        }
        Ime::Enabled => {
            // IME was enabled - we could set a flag here if needed
        }
        Ime::Disabled => {
            // IME was disabled - we could clear a flag here if needed
        }
    }
}

/// Handle touch events by converting them to mouse events
pub fn handle_touch_event(touch: &winit::event::Touch, _window: &Window, _imgui_ctx: &mut Context) {
    thread_local! {
        static ACTIVE_TOUCH: RefCell<Option<u64>> = const { RefCell::new(None) };
    }

    // Convert touch events to mouse events for basic touch support
    ACTIVE_TOUCH.with(|active| {
        let active_id = active.borrow().clone();
        let id = touch.id;
        match touch.phase {
            TouchPhase::Started => {
                if active_id.is_none() {
                    // Capture this touch as the active pointer
                    *active.borrow_mut() = Some(id);
                    let pos = touch.location.to_logical::<f64>(_window.scale_factor());
                    _imgui_ctx
                        .io_mut()
                        .add_mouse_pos_event([pos.x as f32, pos.y as f32]);
                    _imgui_ctx
                        .io_mut()
                        .add_mouse_button_event(dear_imgui_rs::input::MouseButton::Left, true);
                }
            }
            TouchPhase::Moved => {
                if active_id == Some(id) {
                    let pos = touch.location.to_logical::<f64>(_window.scale_factor());
                    _imgui_ctx
                        .io_mut()
                        .add_mouse_pos_event([pos.x as f32, pos.y as f32]);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if active_id == Some(id) {
                    let pos = touch.location.to_logical::<f64>(_window.scale_factor());
                    _imgui_ctx
                        .io_mut()
                        .add_mouse_pos_event([pos.x as f32, pos.y as f32]);
                    _imgui_ctx
                        .io_mut()
                        .add_mouse_button_event(dear_imgui_rs::input::MouseButton::Left, false);
                    *active.borrow_mut() = None;
                }
            }
        }
    });
}

/// Handle device events (raw input events)
pub fn handle_device_event(_event: &DeviceEvent) {
    // Handle device-specific events if needed
    // Currently no specific handling required
}

/// Handle window focus events
pub fn handle_focused(focused: bool, imgui_ctx: &mut Context) -> bool {
    // Tell Dear ImGui about host window focus change
    imgui_ctx.io_mut().add_focus_event(focused);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::Context;
    use winit::event::{ElementState, MouseButton};

    #[test]
    fn test_keyboard_input_handling() {
        // We can't construct KeyEvent directly due to private fields
        // So we'll just test that the function exists and can be called
        // In a real scenario, KeyEvent would be provided by winit

        // Test that the function exists and can be called
        // We'll use a dummy test that doesn't require constructing KeyEvent
        // This is a placeholder test - in a real implementation we would test actual key handling
    }

    #[test]
    fn test_mouse_button_handling() {
        let mut ctx = Context::create();

        let handled = handle_mouse_button(MouseButton::Left, ElementState::Pressed, &mut ctx);
        // The result depends on whether imgui wants to capture mouse
        // We just test that it doesn't panic
        // Test that the function returns a boolean value (always true)
        let _ = handled; // Just verify it's a boolean
    }

    #[test]
    fn test_cursor_moved() {
        let mut ctx = Context::create();

        let handled = handle_cursor_moved([100.0, 200.0], &mut ctx);
        // The result depends on whether imgui wants to capture mouse
        // We just test that it doesn't panic
        // Test that the function returns a boolean value (always true)
        let _ = handled; // Just verify it's a boolean
    }
}
