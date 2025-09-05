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
//! let mut imgui_ctx = Context::create();
//! let mut platform = WinitPlatform::new(&mut imgui_ctx);
//!
//! // Use in your event loop...
//! ```

use std::collections::HashMap;
use std::time::Instant;

use dear_imgui::{Context, Key as ImGuiKey, MouseCursor as ImGuiMouseCursor};
use dear_imgui_sys as sys;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{
    DeviceEvent, ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::keyboard::{Key as WinitKey, KeyLocation, NamedKey};
use winit::window::{CursorIcon as WinitCursor, Window, WindowAttributes};

/// DPI factor handling mode.
///
/// Applications that use dear-imgui might want to customize the used DPI factor and not use
/// directly the value coming from winit.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum HiDpiMode {
    /// The DPI factor from winit is used directly without adjustment
    Default,
    /// The DPI factor from winit is rounded to an integer value.
    ///
    /// This prevents the user interface from becoming blurry with non-integer scaling.
    Rounded,
    /// The DPI factor from winit is ignored, and the included value is used instead.
    ///
    /// This is useful if you want to force some DPI factor (e.g. 1.0) and not care about the value
    /// coming from winit.
    Locked(f64),
}

impl HiDpiMode {
    fn apply(&self, hidpi_factor: f64) -> f64 {
        match *self {
            HiDpiMode::Default => hidpi_factor,
            HiDpiMode::Rounded => hidpi_factor.round(),
            HiDpiMode::Locked(value) => value,
        }
    }
}

/// Winit platform backend for Dear ImGui
///
/// This struct manages the integration between Dear ImGui and winit,
/// handling input events, window management, and platform-specific functionality.
pub struct WinitPlatform {
    last_frame_time: Instant,
    mouse_buttons: [bool; 5],
    mouse_pos: [f32; 2],
    hidpi_mode: HiDpiMode,
    hidpi_factor: f64,
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
            hidpi_mode: HiDpiMode::Default,
            hidpi_factor: 1.0,
        };

        platform.configure_imgui(imgui_ctx);

        platform
    }

    /// Attach the platform to a window with specified DPI mode
    ///
    /// # Arguments
    /// * `window` - The winit window to attach to
    /// * `hidpi_mode` - The DPI handling mode to use
    pub fn attach_window(&mut self, window: &Window, hidpi_mode: HiDpiMode) {
        let scale_factor = window.scale_factor();
        self.hidpi_factor = hidpi_mode.apply(scale_factor);
        self.hidpi_mode = hidpi_mode;

        // Update display size and framebuffer scale immediately
        self.update_display_size(window);
    }

    /// Get the current DPI factor
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    /// Configure fonts for the current DPI factor
    ///
    /// This is a helper function to set up fonts with proper DPI scaling.
    /// Call this after attaching the window and before using Dear ImGui.
    ///
    /// # Arguments
    /// * `imgui_ctx` - The Dear ImGui context
    /// * `base_font_size` - The base font size in logical pixels (default: 13.0)
    pub fn configure_fonts(&self, _imgui_ctx: &mut Context, base_font_size: f32) -> f32 {
        let font_size = base_font_size * self.hidpi_factor as f32;

        // Note: FontGlobalScale is not available in our Dear ImGui version
        // Font scaling should be handled through font size instead

        // Return the calculated font size for the user to use when loading fonts
        font_size
    }

    /// Update display size based on current window and DPI settings
    fn update_display_size(&self, window: &Window) {
        unsafe {
            let io = sys::ImGui_GetIO();

            // Get physical size and convert to logical size using our DPI factor
            let physical_size = window.inner_size();
            let logical_width = physical_size.width as f64 / self.hidpi_factor;
            let logical_height = physical_size.height as f64 / self.hidpi_factor;

            (*io).DisplaySize.x = logical_width as f32;
            (*io).DisplaySize.y = logical_height as f32;
            (*io).DisplayFramebufferScale.x = self.hidpi_factor as f32;
            (*io).DisplayFramebufferScale.y = self.hidpi_factor as f32;
        }
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
    pub fn prepare_frame(&mut self, window: &Window, _imgui_ctx: &mut Context) {
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Update display size in case window was resized or DPI changed
        self.update_display_size(window);

        unsafe {
            let io = sys::ImGui_GetIO();

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
                // Convert physical position to logical position using our DPI factor
                let logical_pos = position.to_logical::<f64>(self.hidpi_factor);
                self.mouse_pos = [logical_pos.x as f32, logical_pos.y as f32];
                false
            }
            WindowEvent::Resized(_) => {
                // Update display size when window is resized
                self.update_display_size(window);
                false
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // Handle DPI changes
                let new_hidpi_factor = self.hidpi_mode.apply(*scale_factor);

                // Update mouse position to account for DPI change
                if self.mouse_pos[0].is_finite() && self.mouse_pos[1].is_finite() {
                    let scale_ratio = new_hidpi_factor / self.hidpi_factor;
                    self.mouse_pos[0] = (self.mouse_pos[0] as f64 / scale_ratio) as f32;
                    self.mouse_pos[1] = (self.mouse_pos[1] as f64 / scale_ratio) as f32;
                }

                self.hidpi_factor = new_hidpi_factor;
                self.update_display_size(window);
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
                && matches!(event.logical_key, WinitKey::Named(NamedKey::Control));
            (*io).KeyShift = event.state == ElementState::Pressed
                && matches!(event.logical_key, WinitKey::Named(NamedKey::Shift));
            (*io).KeyAlt = event.state == ElementState::Pressed
                && matches!(event.logical_key, WinitKey::Named(NamedKey::Alt));
            (*io).KeySuper = event.state == ElementState::Pressed
                && matches!(event.logical_key, WinitKey::Named(NamedKey::Super));

            // Handle key press/release
            if let Some(imgui_key) = winit_key_to_imgui_key(&event.logical_key) {
                (*io).AddKeyEvent(imgui_key, event.state == ElementState::Pressed);
            }

            // Handle text input
            if event.state == ElementState::Pressed {
                if let WinitKey::Character(ref text) = event.logical_key {
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

/// Convert winit key to Dear ImGui key
fn winit_key_to_imgui_key(key: &WinitKey) -> Option<sys::ImGuiKey> {
    match key {
        WinitKey::Named(named_key) => match named_key {
            NamedKey::Tab => Some(sys::ImGuiKey_Tab),
            NamedKey::ArrowLeft => Some(sys::ImGuiKey_LeftArrow),
            NamedKey::ArrowRight => Some(sys::ImGuiKey_RightArrow),
            NamedKey::ArrowUp => Some(sys::ImGuiKey_UpArrow),
            NamedKey::ArrowDown => Some(sys::ImGuiKey_DownArrow),
            NamedKey::PageUp => Some(sys::ImGuiKey_PageUp),
            NamedKey::PageDown => Some(sys::ImGuiKey_PageDown),
            NamedKey::Home => Some(sys::ImGuiKey_Home),
            NamedKey::End => Some(sys::ImGuiKey_End),
            NamedKey::Insert => Some(sys::ImGuiKey_Insert),
            NamedKey::Delete => Some(sys::ImGuiKey_Delete),
            NamedKey::Backspace => Some(sys::ImGuiKey_Backspace),
            NamedKey::Space => Some(sys::ImGuiKey_Space),
            NamedKey::Enter => Some(sys::ImGuiKey_Enter),
            NamedKey::Escape => Some(sys::ImGuiKey_Escape),
            NamedKey::Control => Some(sys::ImGuiKey_LeftCtrl),
            NamedKey::Shift => Some(sys::ImGuiKey_LeftShift),
            NamedKey::Alt => Some(sys::ImGuiKey_LeftAlt),
            NamedKey::Super => Some(sys::ImGuiKey_LeftSuper),
            _ => None,
        },
        WinitKey::Character(text) => {
            if text.len() == 1 {
                let ch = text.chars().next().unwrap();
                match ch {
                    'a'..='z' => Some(sys::ImGuiKey_A + (ch as u32 - 'a' as u32) as i32),
                    'A'..='Z' => Some(sys::ImGuiKey_A + (ch as u32 - 'A' as u32) as i32),
                    '0'..='9' => Some(sys::ImGuiKey_0 + (ch as u32 - '0' as u32) as i32),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_creation() {
        let mut ctx = Context::create();
        let _platform = WinitPlatform::new(&mut ctx);
    }
}
