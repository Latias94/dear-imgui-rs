//! Winit platform backend for Dear ImGui
//!
//! This crate provides a platform backend for Dear ImGui that integrates with
//! the winit windowing library. It handles window events, input processing,
//! and platform-specific functionality.
//!
//! # Example
//!
//! ```rust,no_run
//! use dear_imgui::Context;
//! use dear_imgui_winit::WinitPlatform;
//! use winit::event_loop::EventLoop;
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut imgui_ctx = Context::new().unwrap();
//! let mut platform = WinitPlatform::new(&mut imgui_ctx);
//!
//! // Use in your event loop...
//! ```

use std::collections::HashMap;
use std::time::Instant;

use dear_imgui::{Context, ImGuiError, Result};
use dear_imgui_sys as sys;
use winit::dpi::LogicalSize;
use winit::event::{
    DeviceEvent, ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes};

/// Winit platform backend for Dear ImGui
///
/// This struct manages the integration between Dear ImGui and winit,
/// handling input events, window management, and platform-specific functionality.
pub struct WinitPlatform {
    last_frame_time: Instant,
    mouse_buttons: [bool; 5],
    mouse_pos: [f32; 2],
    key_map: HashMap<Key, sys::ImGuiKey>,
}

impl WinitPlatform {
    /// Create a new winit platform backend
    ///
    /// # Arguments
    ///
    /// * `imgui_ctx` - The Dear ImGui context to configure
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dear_imgui::Context;
    /// use dear_imgui_winit::WinitPlatform;
    ///
    /// let mut imgui_ctx = Context::new().unwrap();
    /// let platform = WinitPlatform::new(&mut imgui_ctx);
    /// ```
    pub fn new(imgui_ctx: &mut Context) -> Self {
        let mut platform = Self {
            last_frame_time: Instant::now(),
            mouse_buttons: [false; 5],
            mouse_pos: [0.0, 0.0],
            key_map: HashMap::new(),
        };

        platform.setup_key_map();
        platform.configure_imgui(imgui_ctx);

        platform
    }

    /// Handle a winit event
    ///
    /// This method should be called for each winit event to update Dear ImGui's
    /// input state accordingly.
    ///
    /// # Arguments
    ///
    /// * `event` - The winit event to process
    /// * `window` - The window associated with the event
    ///
    /// # Returns
    ///
    /// Returns `true` if Dear ImGui wants to capture this event (e.g., mouse is
    /// over a Dear ImGui window), `false` otherwise.
    pub fn handle_event<T>(&mut self, event: &Event<T>, window: &Window) -> bool {
        match event {
            Event::WindowEvent {
                event, window_id, ..
            } if *window_id == window.id() => self.handle_window_event(event, window),
            Event::DeviceEvent { event, .. } => {
                self.handle_device_event(event);
                false
            }
            _ => false,
        }
    }

    /// Prepare Dear ImGui for a new frame
    ///
    /// This method should be called at the beginning of each frame, before
    /// building the Dear ImGui UI.
    ///
    /// # Arguments
    ///
    /// * `window` - The window to prepare the frame for
    /// * `imgui_ctx` - The Dear ImGui context
    pub fn prepare_frame(&mut self, window: &Window, imgui_ctx: &mut Context) {
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        unsafe {
            let io = sys::ImGui_GetIO();

            // Update display size and framebuffer scale
            let size = window.inner_size();
            let scale_factor = window.scale_factor() as f32;
            (*io).DisplaySize.x = size.width as f32;
            (*io).DisplaySize.y = size.height as f32;
            (*io).DisplayFramebufferScale.x = scale_factor;
            (*io).DisplayFramebufferScale.y = scale_factor;

            // Update display framebuffer scale
            let scale_factor = window.scale_factor() as f32;
            (*io).DisplayFramebufferScale.x = scale_factor;
            (*io).DisplayFramebufferScale.y = scale_factor;

            // Update delta time
            (*io).DeltaTime = delta_time.max(1.0 / 60.0); // Minimum 60 FPS

            // Update mouse position
            (*io).MousePos.x = self.mouse_pos[0];
            (*io).MousePos.y = self.mouse_pos[1];

            // Update mouse buttons
            for (i, &pressed) in self.mouse_buttons.iter().enumerate() {
                if i < 5 {
                    (*io).MouseDown[i] = pressed;
                }
            }
        }
    }

    fn setup_key_map(&mut self) {
        // Map winit keys to Dear ImGui keys
        self.key_map
            .insert(Key::Named(NamedKey::Tab), sys::ImGuiKey_Tab);
        self.key_map
            .insert(Key::Named(NamedKey::ArrowLeft), sys::ImGuiKey_LeftArrow);
        self.key_map
            .insert(Key::Named(NamedKey::ArrowRight), sys::ImGuiKey_RightArrow);
        self.key_map
            .insert(Key::Named(NamedKey::ArrowUp), sys::ImGuiKey_UpArrow);
        self.key_map
            .insert(Key::Named(NamedKey::ArrowDown), sys::ImGuiKey_DownArrow);
        self.key_map
            .insert(Key::Named(NamedKey::PageUp), sys::ImGuiKey_PageUp);
        self.key_map
            .insert(Key::Named(NamedKey::PageDown), sys::ImGuiKey_PageDown);
        self.key_map
            .insert(Key::Named(NamedKey::Home), sys::ImGuiKey_Home);
        self.key_map
            .insert(Key::Named(NamedKey::End), sys::ImGuiKey_End);
        self.key_map
            .insert(Key::Named(NamedKey::Insert), sys::ImGuiKey_Insert);
        self.key_map
            .insert(Key::Named(NamedKey::Delete), sys::ImGuiKey_Delete);
        self.key_map
            .insert(Key::Named(NamedKey::Backspace), sys::ImGuiKey_Backspace);
        self.key_map
            .insert(Key::Named(NamedKey::Space), sys::ImGuiKey_Space);
        self.key_map
            .insert(Key::Named(NamedKey::Enter), sys::ImGuiKey_Enter);
        self.key_map
            .insert(Key::Named(NamedKey::Escape), sys::ImGuiKey_Escape);
        self.key_map
            .insert(Key::Named(NamedKey::Control), sys::ImGuiKey_LeftCtrl);
        self.key_map
            .insert(Key::Named(NamedKey::Shift), sys::ImGuiKey_LeftShift);
        self.key_map
            .insert(Key::Named(NamedKey::Alt), sys::ImGuiKey_LeftAlt);
        self.key_map
            .insert(Key::Named(NamedKey::Super), sys::ImGuiKey_LeftSuper);

        // Add character keys
        for c in 'A'..='Z' {
            if let Ok(key) = c.to_string().parse::<char>() {
                let imgui_key = sys::ImGuiKey_A + (c as u32 - 'A' as u32) as i32;
                self.key_map
                    .insert(Key::Character(c.to_string().into()), imgui_key);
            }
        }

        // Add number keys
        for i in 0..=9 {
            let imgui_key = sys::ImGuiKey_0 + i;
            self.key_map
                .insert(Key::Character(i.to_string().into()), imgui_key);
        }
    }

    fn configure_imgui(&self, imgui_ctx: &mut Context) {
        unsafe {
            let io = sys::ImGui_GetIO();

            // Enable keyboard and mouse navigation
            (*io).ConfigFlags |= sys::ImGuiConfigFlags_NavEnableKeyboard as i32;
            (*io).ConfigFlags |= sys::ImGuiConfigFlags_NavEnableGamepad as i32;

            // Set backend flags
            (*io).BackendFlags |= sys::ImGuiBackendFlags_HasMouseCursors as i32;
            (*io).BackendFlags |= sys::ImGuiBackendFlags_HasSetMousePos as i32;

            // Set backend name
            let backend_name = std::ffi::CString::new("dear-imgui-winit").unwrap();
            (*io).BackendPlatformName = backend_name.as_ptr();
        }
    }

    fn handle_window_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = [position.x as f32, position.y as f32];
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button_index = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Back => 3,
                    MouseButton::Forward => 4,
                    MouseButton::Other(_) => return false,
                };

                if button_index < 5 {
                    self.mouse_buttons[button_index] = *state == ElementState::Pressed;
                }

                // Check if mouse is over Dear ImGui
                unsafe {
                    let io = sys::ImGui_GetIO();
                    (*io).WantCaptureMouse
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => {
                        (pos.x as f32 / 120.0, pos.y as f32 / 120.0)
                    }
                };

                unsafe {
                    let io = sys::ImGui_GetIO();
                    (*io).MouseWheelH += x;
                    (*io).MouseWheel += y;
                    (*io).WantCaptureMouse
                }
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_input(event),
            _ => false,
        }
    }

    fn handle_device_event(&mut self, event: &DeviceEvent) {
        // Handle device-specific events if needed
        match event {
            _ => {}
        }
    }

    fn handle_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        unsafe {
            let io = sys::ImGui_GetIO();

            // Update modifier keys
            (*io).KeyCtrl = event.state == ElementState::Pressed
                && matches!(event.logical_key, Key::Named(NamedKey::Control));
            (*io).KeyShift = event.state == ElementState::Pressed
                && matches!(event.logical_key, Key::Named(NamedKey::Shift));
            (*io).KeyAlt = event.state == ElementState::Pressed
                && matches!(event.logical_key, Key::Named(NamedKey::Alt));
            (*io).KeySuper = event.state == ElementState::Pressed
                && matches!(event.logical_key, Key::Named(NamedKey::Super));

            // Handle key press/release
            if let Some(&imgui_key) = self.key_map.get(&event.logical_key) {
                let key_data = &mut (*io).KeysData[imgui_key as usize];
                key_data.Down = event.state == ElementState::Pressed;
            }

            // Handle text input
            if event.state == ElementState::Pressed {
                if let Key::Character(ref text) = event.logical_key {
                    for ch in text.chars() {
                        if ch.is_ascii() && !ch.is_control() {
                            (*io).AddInputCharacter(ch as u32);
                        }
                    }
                }
            }

            (*io).WantCaptureKeyboard
        }
    }
}

/// Helper function to create winit window attributes with Dear ImGui-friendly settings
///
/// # Arguments
///
/// * `title` - Window title
/// * `size` - Window size in logical pixels
///
/// # Returns
///
/// Returns configured window attributes
pub fn create_window_attributes(title: &str, size: LogicalSize<f32>) -> WindowAttributes {
    WindowAttributes::default()
        .with_title(title)
        .with_inner_size(size)
        .with_visible(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_creation() {
        let mut ctx = Context::new().unwrap();
        let _platform = WinitPlatform::new(&mut ctx);
    }
}
