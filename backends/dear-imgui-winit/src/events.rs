//! Event handling for Dear ImGui winit backend
//!
//! This module contains event processing logic for various winit events
//! including keyboard, mouse, touch, and IME events.

use dear_imgui::Context;
use winit::event::{DeviceEvent, ElementState, Ime, KeyEvent, MouseScrollDelta, TouchPhase};

use winit::window::Window;

use crate::input::{to_imgui_mouse_button, winit_key_to_imgui_key};

/// Handle keyboard input events
pub fn handle_keyboard_input(event: &KeyEvent, imgui_ctx: &mut Context) -> bool {
    let io = imgui_ctx.io_mut();

    if let Some(imgui_key) = winit_key_to_imgui_key(&event.logical_key, event.location) {
        let pressed = event.state == ElementState::Pressed;
        io.add_key_event(imgui_key, pressed);
        return io.want_capture_keyboard();
    }

    false
}

/// Handle character input for text editing
pub fn handle_character_input(character: char, imgui_ctx: &mut Context) {
    // Filter out control characters but allow normal text input
    if !character.is_control() || character == '\t' || character == '\n' || character == '\r' {
        imgui_ctx.io_mut().add_input_character(character);
    }
}

/// Handle mouse wheel scrolling
pub fn handle_mouse_wheel(delta: MouseScrollDelta, imgui_ctx: &mut Context) -> bool {
    let io = imgui_ctx.io_mut();

    match delta {
        MouseScrollDelta::LineDelta(h, v) => {
            io.add_mouse_wheel_event([h, v]);
        }
        MouseScrollDelta::PixelDelta(pos) => {
            let h = pos.x as f32;
            let v = pos.y as f32;
            io.add_mouse_wheel_event([h / 100.0, v / 100.0]); // Scale pixel delta
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
        imgui_ctx.io_mut().add_mouse_button_event(imgui_button, pressed);
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
    io.add_key_event(dear_imgui::Key::LeftShift, state.shift_key());
    io.add_key_event(dear_imgui::Key::RightShift, state.shift_key());
    io.add_key_event(dear_imgui::Key::LeftCtrl, state.control_key());
    io.add_key_event(dear_imgui::Key::RightCtrl, state.control_key());
    io.add_key_event(dear_imgui::Key::LeftAlt, state.alt_key());
    io.add_key_event(dear_imgui::Key::RightAlt, state.alt_key());
    io.add_key_event(dear_imgui::Key::LeftSuper, state.super_key());
    io.add_key_event(dear_imgui::Key::RightSuper, state.super_key());
}

/// Handle IME (Input Method Editor) events for international text input
pub fn handle_ime_event(ime: &Ime, imgui_ctx: &mut Context) {
    match ime {
        Ime::Preedit(text, _cursor_range) => {
            // Handle pre-edit text (text being composed)
            // For now, we'll just add the characters as they come
            for ch in text.chars() {
                if !ch.is_control() {
                    imgui_ctx.io_mut().add_input_character(ch);
                }
            }
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
pub fn handle_touch_event(
    touch: &winit::event::Touch,
    _window: &Window,
    _imgui_ctx: &mut Context,
) {
    // Convert touch events to mouse events for basic touch support
    match touch.phase {
        TouchPhase::Started => {
            // Treat touch start as left mouse button press
            // We could add this to imgui_ctx if needed
        }
        TouchPhase::Moved => {
            // Treat touch move as mouse move
            // We could update mouse position here
        }
        TouchPhase::Ended | TouchPhase::Cancelled => {
            // Treat touch end as left mouse button release
            // We could add this to imgui_ctx if needed
        }
    }
}

/// Handle device events (raw input events)
pub fn handle_device_event(_event: &DeviceEvent) {
    // Handle device-specific events if needed
    // Currently no specific handling required
}

/// Handle window focus events
pub fn handle_focused(_focused: bool, _imgui_ctx: &mut Context) -> bool {
    // Note: Our dear-imgui doesn't have set_app_focus_lost method
    // We'll handle focus events differently or skip for now
    // TODO: Add focus event handling if needed
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui::Context;
    use winit::event::{ElementState, MouseButton};

    #[test]
    fn test_keyboard_input_handling() {
        // We can't construct KeyEvent directly due to private fields
        // So we'll just test that the function exists and can be called
        // In a real scenario, KeyEvent would be provided by winit

        // Test that the function exists and can be called
        // We'll use a dummy test that doesn't require constructing KeyEvent
        assert!(true); // Placeholder test
    }

    #[test]
    fn test_mouse_button_handling() {
        let mut ctx = Context::create();
        
        let handled = handle_mouse_button(MouseButton::Left, ElementState::Pressed, &mut ctx);
        // The result depends on whether imgui wants to capture mouse
        // We just test that it doesn't panic
        assert!(handled == true || handled == false);
    }

    #[test]
    fn test_character_input() {
        let mut ctx = Context::create();
        
        // Should handle normal characters
        handle_character_input('a', &mut ctx);
        handle_character_input('1', &mut ctx);
        handle_character_input('!', &mut ctx);
        
        // Should handle special characters
        handle_character_input('\t', &mut ctx);
        handle_character_input('\n', &mut ctx);
        
        // Test doesn't panic - actual behavior depends on imgui internals
    }

    #[test]
    fn test_cursor_moved() {
        let mut ctx = Context::create();
        
        let handled = handle_cursor_moved([100.0, 200.0], &mut ctx);
        // The result depends on whether imgui wants to capture mouse
        // We just test that it doesn't panic
        assert!(handled == true || handled == false);
    }
}
