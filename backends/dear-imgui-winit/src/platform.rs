//! Main platform implementation for Dear ImGui winit backend
//!
//! This module contains the core `WinitPlatform` struct and its implementation
//! for integrating Dear ImGui with winit windowing.

use std::time::Instant;

use dear_imgui::{BackendFlags, Context};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowAttributes};

use crate::cursor::CursorSettings;
use crate::events;

/// DPI scaling mode for the platform
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum HiDpiMode {
    /// Use the default DPI scaling
    Default,
    /// Use a custom scale factor
    Locked(f64),
    /// Round the scale factor to the nearest integer
    Rounded,
}

impl Default for HiDpiMode {
    fn default() -> Self {
        HiDpiMode::Default
    }
}

/// Main platform backend for Dear ImGui with winit integration
pub struct WinitPlatform {
    hidpi_mode: HiDpiMode,
    hidpi_factor: f64,
    cursor_cache: Option<CursorSettings>,
    ime_enabled: bool,
    last_frame: Instant,
}

impl WinitPlatform {
    /// Create a new winit platform backend
    ///
    /// # Example
    ///
    /// ```
    /// use dear_imgui::Context;
    /// use dear_imgui_winit::WinitPlatform;
    /// 
    /// let mut imgui_ctx = Context::create();
    /// let mut platform = WinitPlatform::new(&mut imgui_ctx);
    /// ```
    pub fn new(imgui_ctx: &mut Context) -> Self {
        let io = imgui_ctx.io_mut();

        // Set backend flags
        let mut backend_flags = io.backend_flags();
        backend_flags.insert(BackendFlags::HAS_MOUSE_CURSORS | BackendFlags::HAS_SET_MOUSE_POS);

        #[cfg(feature = "multi-viewport")]
        {
            let mut config_flags = io.config_flags();
            config_flags.insert(dear_imgui::ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(config_flags);
            backend_flags.insert(BackendFlags::PLATFORM_HAS_VIEWPORTS);
        }

        io.set_backend_flags(backend_flags);

        Self {
            hidpi_mode: HiDpiMode::default(),
            hidpi_factor: 1.0,
            cursor_cache: None,
            ime_enabled: false,
            last_frame: Instant::now(),
        }
    }

    /// Set the DPI scaling mode
    pub fn set_hidpi_mode(&mut self, hidpi_mode: HiDpiMode) {
        self.hidpi_mode = hidpi_mode;
    }

    /// Get the current DPI scaling factor
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    /// Attach the platform to a window
    pub fn attach_window(&mut self, window: &Window, hidpi_mode: HiDpiMode, imgui_ctx: &mut Context) {
        self.hidpi_mode = hidpi_mode;
        self.hidpi_factor = match hidpi_mode {
            HiDpiMode::Default => window.scale_factor(),
            HiDpiMode::Locked(factor) => factor,
            HiDpiMode::Rounded => window.scale_factor().round(),
        };

        let logical_size = window.inner_size().to_logical(self.hidpi_factor);
        let io = imgui_ctx.io_mut();

        io.set_display_size([logical_size.width, logical_size.height]);
        io.set_display_framebuffer_scale([self.hidpi_factor as f32, self.hidpi_factor as f32]);
    }

    /// Handle a winit event
    pub fn handle_event<T>(&mut self, imgui_ctx: &mut Context, window: &Window, event: &Event<T>) -> bool {
        match event {
            Event::WindowEvent { event, .. } => self.handle_window_event(imgui_ctx, window, event),
            Event::DeviceEvent { event, .. } => {
                events::handle_device_event(event);
                false
            }
            _ => false,
        }
    }

    /// Handle a window event
    fn handle_window_event(&mut self, imgui_ctx: &mut Context, window: &Window, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::Resized(physical_size) => {
                let logical_size = physical_size.to_logical(self.hidpi_factor);
                imgui_ctx.io_mut().set_display_size([logical_size.width, logical_size.height]);
                false
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.hidpi_factor = match self.hidpi_mode {
                    HiDpiMode::Default => *scale_factor,
                    HiDpiMode::Locked(factor) => factor,
                    HiDpiMode::Rounded => scale_factor.round(),
                };

                let logical_size = window.inner_size().to_logical(self.hidpi_factor);
                let io = imgui_ctx.io_mut();
                io.set_display_size([logical_size.width, logical_size.height]);
                io.set_display_framebuffer_scale([self.hidpi_factor as f32, self.hidpi_factor as f32]);
                false
            }
            WindowEvent::KeyboardInput { event, .. } => {
                events::handle_keyboard_input(event, imgui_ctx)
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical(self.hidpi_factor);
                events::handle_cursor_moved([position.x, position.y], imgui_ctx)
            }
            WindowEvent::MouseInput { button, state, .. } => {
                events::handle_mouse_button(*button, *state, imgui_ctx)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                events::handle_mouse_wheel(*delta, imgui_ctx)
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                events::handle_modifiers_changed(modifiers, imgui_ctx);
                false
            }
            WindowEvent::Ime(ime) => {
                events::handle_ime_event(ime, imgui_ctx);
                imgui_ctx.io().want_capture_keyboard()
            }
            WindowEvent::Touch(touch) => {
                events::handle_touch_event(touch, window, imgui_ctx);
                imgui_ctx.io().want_capture_mouse()
            }
            WindowEvent::Focused(focused) => {
                events::handle_focused(*focused, imgui_ctx)
            }
            _ => false,
        }
    }

    /// Prepare for rendering - should be called before Dear ImGui rendering
    pub fn prepare_render(&mut self, imgui_ctx: &mut Context, window: &Window) {
        let now = Instant::now();
        let delta = now - self.last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.last_frame = now;

        imgui_ctx.io_mut().set_delta_time(delta_s);

        // Update cursor if needed
        self.update_cursor(imgui_ctx, window);
    }

    /// Prepare frame - alias for prepare_render for compatibility
    pub fn prepare_frame(&mut self, window: &Window, imgui_ctx: &mut Context) {
        self.prepare_render(imgui_ctx, window);
    }

    /// Update the mouse cursor
    fn update_cursor(&mut self, _imgui_ctx: &Context, window: &Window) {
        // Note: Our dear-imgui doesn't have mouse_cursor() and mouse_draw_cursor() methods on Io
        // We'll need to get this information from the UI context instead
        let cursor = CursorSettings {
            cursor: None, // TODO: Get current cursor from UI context
            draw_cursor: false, // TODO: Get draw cursor setting
        };

        if self.cursor_cache != Some(cursor) {
            cursor.apply(window);
            self.cursor_cache = Some(cursor);
        }
    }

    /// Create window attributes with Dear ImGui defaults
    pub fn create_window_attributes() -> WindowAttributes {
        WindowAttributes::default()
            .with_title("Dear ImGui Window")
            .with_inner_size(LogicalSize::new(1024.0, 768.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hidpi_mode_default() {
        assert_eq!(HiDpiMode::default(), HiDpiMode::Default);
    }

    #[test]
    fn test_platform_creation() {
        let mut ctx = Context::create();
        let platform = WinitPlatform::new(&mut ctx);
        
        assert_eq!(platform.hidpi_mode, HiDpiMode::Default);
        assert_eq!(platform.hidpi_factor, 1.0);
        assert_eq!(platform.cursor_cache, None);
        assert_eq!(platform.ime_enabled, false);
    }

    #[test]
    fn test_hidpi_mode_setting() {
        let mut ctx = Context::create();
        let mut platform = WinitPlatform::new(&mut ctx);
        
        platform.set_hidpi_mode(HiDpiMode::Locked(2.0));
        assert_eq!(platform.hidpi_mode, HiDpiMode::Locked(2.0));
        
        platform.set_hidpi_mode(HiDpiMode::Rounded);
        assert_eq!(platform.hidpi_mode, HiDpiMode::Rounded);
    }

    #[test]
    fn test_window_attributes_creation() {
        let attrs = WinitPlatform::create_window_attributes();
        // Just test that it doesn't panic - actual values depend on winit defaults
        let _ = attrs;
    }
}
