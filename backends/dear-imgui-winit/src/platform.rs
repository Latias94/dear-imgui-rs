//! Main platform implementation for Dear ImGui winit backend
//!
//! This module contains the core `WinitPlatform` struct and its implementation
//! for integrating Dear ImGui with winit windowing.

use instant::Instant;

use dear_imgui_rs::{BackendFlags, ConfigFlags, Context};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowAttributes};

use crate::cursor::CursorSettings;
use crate::events;

/// DPI scaling mode for the platform
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum HiDpiMode {
    /// Use the default DPI scaling
    #[default]
    Default,
    /// Use a custom scale factor
    Locked(f64),
    /// Round the scale factor to the nearest integer
    Rounded,
}

/// Main platform backend for Dear ImGui with winit integration
pub struct WinitPlatform {
    hidpi_mode: HiDpiMode,
    hidpi_factor: f64,
    cursor_cache: Option<CursorSettings>,
    #[allow(dead_code)]
    ime_enabled: bool,
    last_frame: Instant,
}

impl WinitPlatform {
    /// Create a new winit platform backend
    ///
    /// # Example
    ///
    /// ```
    /// use dear_imgui_rs::Context;
    /// use dear_imgui_winit::WinitPlatform;
    ///
    /// let mut imgui_ctx = Context::create();
    /// let mut platform = WinitPlatform::new(&mut imgui_ctx);
    /// ```
    pub fn new(imgui_ctx: &mut Context) -> Self {
        // Set backend platform name for diagnostics before borrowing Io
        let _ = imgui_ctx.set_platform_name(Some(format!(
            "dear-imgui-winit {}",
            env!("CARGO_PKG_VERSION")
        )));

        let io = imgui_ctx.io_mut();

        // Set backend flags
        let mut backend_flags = io.backend_flags();
        backend_flags.insert(BackendFlags::HAS_MOUSE_CURSORS | BackendFlags::HAS_SET_MOUSE_POS);

        #[cfg(feature = "multi-viewport")]
        {
            let mut config_flags = io.config_flags();
            config_flags.insert(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(config_flags);
            backend_flags.insert(BackendFlags::PLATFORM_HAS_VIEWPORTS);
            // When viewports are enabled, avoid moving OS cursor from ImGui (no global set in winit)
            backend_flags.remove(BackendFlags::HAS_SET_MOUSE_POS);
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
    pub fn attach_window(
        &mut self,
        window: &Window,
        hidpi_mode: HiDpiMode,
        imgui_ctx: &mut Context,
    ) {
        self.hidpi_mode = hidpi_mode;
        self.hidpi_factor = match hidpi_mode {
            HiDpiMode::Default => window.scale_factor(),
            HiDpiMode::Locked(factor) => factor,
            HiDpiMode::Rounded => window.scale_factor().round(),
        };

        // Convert via winit scale then adapt to our active HiDPI mode
        let logical_size = window.inner_size().to_logical(window.scale_factor());
        let logical_size = self.scale_size_from_winit(window, logical_size);
        let io = imgui_ctx.io_mut();

        io.set_display_size([logical_size.width as f32, logical_size.height as f32]);
        io.set_display_framebuffer_scale([self.hidpi_factor as f32, self.hidpi_factor as f32]);
    }

    /// Handle a winit event
    pub fn handle_event<T>(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &Event<T>,
    ) -> bool {
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
    fn handle_window_event(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> bool {
        match event {
            WindowEvent::Resized(physical_size) => {
                let logical_size = physical_size.to_logical(window.scale_factor());
                let logical_size = self.scale_size_from_winit(window, logical_size);
                imgui_ctx
                    .io_mut()
                    .set_display_size([logical_size.width as f32, logical_size.height as f32]);
                false
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let new_hidpi = match self.hidpi_mode {
                    HiDpiMode::Default => *scale_factor,
                    HiDpiMode::Locked(factor) => factor,
                    HiDpiMode::Rounded => scale_factor.round(),
                };
                // Adjust mouse position proportionally when DPI factor changes
                {
                    let io = imgui_ctx.io_mut();
                    let mouse = io.mouse_pos();
                    if mouse[0].is_finite() && mouse[1].is_finite() && self.hidpi_factor > 0.0 {
                        let scale = (new_hidpi / self.hidpi_factor) as f32;
                        io.set_mouse_pos([mouse[0] * scale, mouse[1] * scale]);
                    }
                }
                self.hidpi_factor = new_hidpi;

                let logical_size = window.inner_size().to_logical(window.scale_factor());
                let logical_size = self.scale_size_from_winit(window, logical_size);
                let io = imgui_ctx.io_mut();
                io.set_display_size([logical_size.width as f32, logical_size.height as f32]);
                io.set_display_framebuffer_scale([
                    self.hidpi_factor as f32,
                    self.hidpi_factor as f32,
                ]);
                false
            }
            WindowEvent::KeyboardInput { event, .. } => {
                events::handle_keyboard_input(event, imgui_ctx)
            }
            WindowEvent::CursorMoved { position, .. } => {
                // With multi-viewports enabled, feed absolute/screen coordinates like upstream backends
                #[cfg(feature = "multi-viewport")]
                {
                    if imgui_ctx
                        .io()
                        .config_flags()
                        .contains(dear_imgui::ConfigFlags::VIEWPORTS_ENABLE)
                    {
                        // Feed absolute/screen coordinates using window's client-area origin (inner_position)
                        if let Ok(base) = window.inner_position() {
                            let sx = base.x as f64 + position.x;
                            let sy = base.y as f64 + position.y;
                            return events::handle_cursor_moved([sx, sy], imgui_ctx);
                        }
                    }
                }
                // Fallback: local logical coordinates
                let position = position.to_logical(window.scale_factor());
                let position = self.scale_pos_from_winit(window, position);
                events::handle_cursor_moved([position.x, position.y], imgui_ctx)
            }
            WindowEvent::MouseInput { button, state, .. } => {
                events::handle_mouse_button(*button, *state, imgui_ctx)
            }
            WindowEvent::MouseWheel { delta, .. } => events::handle_mouse_wheel(*delta, imgui_ctx),
            // When cursor leaves the window, tell ImGui the mouse is unavailable so
            // software cursor (if enabled) won’t be drawn at the last position.
            WindowEvent::CursorLeft { .. } => {
                imgui_ctx
                    .io_mut()
                    .add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                false
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
            WindowEvent::Focused(focused) => events::handle_focused(*focused, imgui_ctx),
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

        // If backend supports setting mouse pos and ImGui requests it, honor it
        // Skip when multi-viewports are enabled (no global cursor set in winit)
        if imgui_ctx.io().want_set_mouse_pos()
            && !imgui_ctx
                .io()
                .config_flags()
                .contains(ConfigFlags::VIEWPORTS_ENABLE)
        {
            let pos = imgui_ctx.io().mouse_pos();
            let logical_pos = self
                .scale_pos_for_winit(window, LogicalPosition::new(pos[0] as f64, pos[1] as f64));
            let _ = window.set_cursor_position(logical_pos);
        }
        // Note: cursor shape update is exposed via prepare_render_with_ui()
    }

    /// Prepare frame - alias for prepare_render for compatibility
    pub fn prepare_frame(&mut self, window: &Window, imgui_ctx: &mut Context) {
        self.prepare_render(imgui_ctx, window);
    }

    /// Toggle Dear ImGui software-drawn cursor.
    /// When enabled, the OS cursor is hidden and ImGui draws the cursor in draw data.
    pub fn set_software_cursor_enabled(&mut self, imgui_ctx: &mut Context, enabled: bool) {
        imgui_ctx.io_mut().set_mouse_draw_cursor(enabled);
        // Invalidate cursor cache so next prepare_render_with_ui applies visibility change
        self.cursor_cache = None;
    }

    /// Update cursor given a Ui reference (preferred, matches upstream)
    pub fn prepare_render_with_ui(&mut self, ui: &dear_imgui_rs::Ui, window: &Window) {
        // Only change OS cursor if not disabled by config flags
        if !ui
            .io()
            .config_flags()
            .contains(ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            // Our Io wrapper does not currently expose MouseDrawCursor, assume false (OS cursor)
            let cursor = CursorSettings {
                cursor: ui.mouse_cursor(),
                draw_cursor: ui.io().mouse_draw_cursor(),
            };
            if self.cursor_cache != Some(cursor) {
                cursor.apply(window);
                self.cursor_cache = Some(cursor);
            }
        }
    }

    /// Scale a logical size from winit to our active HiDPI mode
    pub fn scale_size_from_winit(
        &self,
        window: &Window,
        logical_size: LogicalSize<f64>,
    ) -> LogicalSize<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_size,
            // Convert to physical using winit scale, then back to logical with our factor
            _ => logical_size
                .to_physical::<f64>(window.scale_factor())
                .to_logical(self.hidpi_factor),
        }
    }

    /// Scale a logical position from winit to our active HiDPI mode
    pub fn scale_pos_from_winit(
        &self,
        window: &Window,
        logical_pos: LogicalPosition<f64>,
    ) -> LogicalPosition<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_pos,
            _ => logical_pos
                .to_physical::<f64>(window.scale_factor())
                .to_logical(self.hidpi_factor),
        }
    }

    /// Scale a logical position for winit based on our active HiDPI mode
    pub fn scale_pos_for_winit(
        &self,
        window: &Window,
        logical_pos: LogicalPosition<f64>,
    ) -> LogicalPosition<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_pos,
            _ => logical_pos
                .to_physical::<f64>(self.hidpi_factor)
                .to_logical(window.scale_factor()),
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
        assert!(!platform.ime_enabled);
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
