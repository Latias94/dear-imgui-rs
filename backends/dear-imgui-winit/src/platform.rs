//! Main platform implementation for Dear ImGui winit backend
//!
//! This module contains the core `WinitPlatform` struct and its implementation
//! for integrating Dear ImGui with winit windowing.

use instant::Instant;
use std::ffi::c_void;

use dear_imgui_rs::{BackendFlags, ConfigFlags, Context};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowAttributes};

use crate::cursor::CursorSettings;
use crate::events;

/// IME hook: Dear ImGui calls this when the text input caret moves. We forward
/// the position to winit so platforms that support it can position the IME
/// candidate/composition window near the caret.
unsafe extern "C" fn imgui_winit_set_ime_data(
    _ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    data: *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
) {
    use dear_imgui_rs::sys::{ImGuiPlatformImeData, ImGuiViewport};

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        if viewport.is_null() || data.is_null() {
            return;
        }

        // Retrieve the window pointer we stored in Platform_ImeUserData.
        let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
        if pio.is_null() {
            return;
        }

        let user_data = (*pio).Platform_ImeUserData as *const Window;
        if user_data.is_null() {
            return;
        }

        // Safety: we rely on the application to keep the Window alive while the
        // ImGui context is active. This matches typical winit usage.
        let window: &Window = &*user_data;
        let ime: &ImGuiPlatformImeData = &*data;
        let vp: &ImGuiViewport = &*viewport;

        // If IME is not visible and not expecting text input, there's nothing to do.
        if !ime.WantVisible && !ime.WantTextInput {
            return;
        }

        // Dear ImGui gives InputPos in the same coordinate space as the viewport's
        // Pos. Convert to client-area coordinates by subtracting viewport origin.
        let rel_x = (ime.InputPos.x - vp.Pos.x) as f64;
        let rel_y = (ime.InputPos.y - vp.Pos.y) as f64;

        let pos = LogicalPosition::new(rel_x, rel_y);

        // Use the reported line height as a reasonable IME region height. Width is
        // not very important for most IME implementations.
        let line_h = if ime.InputLineHeight > 0.0 {
            ime.InputLineHeight as f64
        } else {
            16.0
        };
        let size = LogicalSize::new(line_h, line_h);

        window.set_ime_cursor_area(pos, size);
    }));
    if res.is_err() {
        eprintln!("dear-imgui-winit: panic in Platform_SetImeDataFn");
        std::process::abort();
    }
}

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
    ime_enabled: bool,
    ime_auto_manage: bool,
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
            // Mark that this platform backend is capable of handling viewports.
            // Note: we intentionally DO NOT enable `ConfigFlags::VIEWPORTS_ENABLE` here.
            // Multi-viewport is an opt-in feature and should be enabled explicitly via:
            //
            //     imgui_ctx.enable_multi_viewport();
            //
            // This matches Dear ImGui's guidance and avoids partially-enabled viewport
            // behavior when the renderer/platform callbacks are not fully wired.
            backend_flags.insert(BackendFlags::PLATFORM_HAS_VIEWPORTS);
            // Backend can also report hovered viewport ids via `Io::add_mouse_viewport_event`.
            backend_flags.insert(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT);
            // We keep `HAS_SET_MOUSE_POS` flag: `prepare_render()` will avoid using it
            // whenever `ConfigFlags::VIEWPORTS_ENABLE` is actually set.
        }

        io.set_backend_flags(backend_flags);

        Self {
            hidpi_mode: HiDpiMode::default(),
            hidpi_factor: 1.0,
            cursor_cache: None,
            ime_enabled: false,
            ime_auto_manage: true,
            last_frame: Instant::now(),
        }
    }

    /// Set the DPI scaling mode
    pub fn set_hidpi_mode(&mut self, hidpi_mode: HiDpiMode) {
        self.hidpi_mode = hidpi_mode;
    }

    /// Enable or disable IME events for the attached window.
    ///
    /// Winit does not deliver `WindowEvent::Ime` events unless IME is explicitly
    /// allowed on the window. When `ime_auto_manage` is enabled (default), the
    /// backend will override this based on `io.want_text_input()` every frame.
    /// Use this helper for immediate overrides (e.g. when auto-management is
    /// disabled or you want to force a specific state for a while).
    pub fn set_ime_allowed(&mut self, window: &Window, allowed: bool) {
        window.set_ime_allowed(allowed);
        self.ime_enabled = allowed;
    }

    /// Returns whether IME is currently allowed for the attached window.
    ///
    /// This reflects the last state set via `set_ime_allowed` or IME
    /// `WindowEvent::Ime(Enabled/Disabled)` notifications.
    pub fn ime_enabled(&self) -> bool {
        self.ime_enabled
    }

    /// Enable or disable automatic IME management.
    ///
    /// When enabled (default), the backend will call `set_ime_allowed` based on
    /// Dear ImGui's `io.want_text_input()` flag each frame, turning IME on
    /// while text widgets are active and off otherwise. When disabled, IME
    /// state is left entirely under application control.
    pub fn set_ime_auto_management(&mut self, enabled: bool) {
        self.ime_auto_manage = enabled;
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

        // Enable IME by default so WindowEvent::Ime events and IME composition
        // are available on desktop platforms. Auto-management (when enabled)
        // will further refine this for text widgets.
        self.set_ime_allowed(window, true);

        // Register Dear ImGui -> winit IME bridge so text input widgets can
        // move the platform IME candidate/composition window near the caret.
        unsafe {
            let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if !pio.is_null() {
                // Store a pointer to the main window; backend assumes a single
                // primary window for IME purposes.
                (*pio).Platform_ImeUserData = window as *const Window as *mut c_void;
                // Install our callback (once per context).
                if (*pio).Platform_SetImeDataFn.is_none() {
                    (*pio).Platform_SetImeDataFn = Some(imgui_winit_set_ime_data);
                }
            }
        }
    }

    /// Handle a winit event.
    ///
    /// This is the most general entry point: pass the full `Event<T>` from
    /// your event loop and the backend will dispatch to the appropriate
    /// handlers. For `ApplicationHandler::window_event`, where you already
    /// receive a `WindowEvent` for a specific window, you can use
    /// `handle_window_event` instead and avoid constructing a synthetic
    /// `Event::WindowEvent`.
    pub fn handle_event<T>(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &Event<T>,
    ) -> bool {
        match event {
            Event::WindowEvent { event, .. } => {
                self.handle_window_event_internal(imgui_ctx, window, event)
            }
            Event::DeviceEvent { event, .. } => {
                events::handle_device_event(event);
                false
            }
            _ => false,
        }
    }

    /// Handle a single window event for a given window.
    ///
    /// This is a convenience wrapper for frameworks that already route
    /// window-local events, such as winit's `ApplicationHandler::window_event`,
    /// and don't need to build a full `Event::WindowEvent` value.
    pub fn handle_window_event(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> bool {
        self.handle_window_event_internal(imgui_ctx, window, event)
    }

    /// Internal implementation for window event handling.
    fn handle_window_event_internal(
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
                        .contains(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE)
                    {
                        // Main window always maps to the main Dear ImGui viewport.
                        let main_viewport_id = imgui_ctx.main_viewport().id();
                        imgui_ctx
                            .io_mut()
                            .add_mouse_viewport_event(main_viewport_id.into());
                        // Feed absolute/screen coordinates in logical pixels, matching io.DisplaySize.
                        let scale = window.scale_factor();
                        let pos_logical = position.to_logical::<f64>(scale);
                        if let Ok(base_phys) = window.inner_position() {
                            let base_logical = base_phys.to_logical::<f64>(scale);
                            let sx = base_logical.x + pos_logical.x;
                            let sy = base_logical.y + pos_logical.y;
                            return events::handle_cursor_moved([sx, sy], imgui_ctx);
                        } else if let Ok(base_phys) = window.outer_position() {
                            let base_logical = base_phys.to_logical::<f64>(scale);
                            let sx = base_logical.x + pos_logical.x;
                            let sy = base_logical.y + pos_logical.y;
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
                {
                    let io = imgui_ctx.io_mut();
                    io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                    // No Dear ImGui viewport is hovered anymore.
                    io.add_mouse_viewport_event(dear_imgui_rs::Id::default());
                }
                false
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                events::handle_modifiers_changed(modifiers, imgui_ctx);
                false
            }
            WindowEvent::Ime(ime) => {
                events::handle_ime_event(ime, imgui_ctx);
                // Track IME enabled/disabled state based on winit notifications.
                self.ime_enabled = !matches!(ime, winit::event::Ime::Disabled);
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

        // In multi-viewport mode, keep DisplaySize/FramebufferScale in sync every frame.
        // This matches upstream backends (SDL/GLFW) and avoids stale or spurious DPI
        // changes affecting the main viewport after platform windows are moved.
        #[cfg(feature = "multi-viewport")]
        {
            if imgui_ctx
                .io()
                .config_flags()
                .contains(ConfigFlags::VIEWPORTS_ENABLE)
            {
                let winit_scale = window.scale_factor();
                let hidpi = match self.hidpi_mode {
                    HiDpiMode::Default => winit_scale,
                    HiDpiMode::Locked(factor) => factor,
                    HiDpiMode::Rounded => winit_scale.round(),
                };
                self.hidpi_factor = hidpi;

                let logical_size = window.inner_size().to_logical(winit_scale);
                let logical_size = self.scale_size_from_winit(window, logical_size);
                let io = imgui_ctx.io_mut();
                io.set_display_size([logical_size.width as f32, logical_size.height as f32]);
                io.set_display_framebuffer_scale([hidpi as f32, hidpi as f32]);
            }
        }

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
        // Auto-manage IME allowed state based on Dear ImGui's intent. This lets
        // the platform show/hide IME (and soft keyboards on mobile) only when
        // text input widgets are active.
        if self.ime_auto_manage {
            let want_text = ui.io().want_text_input();
            if want_text && !self.ime_enabled {
                window.set_ime_allowed(true);
                self.ime_enabled = true;
            } else if !want_text && self.ime_enabled {
                window.set_ime_allowed(false);
                self.ime_enabled = false;
            }
        }

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
    use crate::test_util::test_sync::lock_context;

    #[test]
    fn test_hidpi_mode_default() {
        assert_eq!(HiDpiMode::default(), HiDpiMode::Default);
    }

    #[test]
    fn test_platform_creation() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let platform = WinitPlatform::new(&mut ctx);

        assert_eq!(platform.hidpi_mode, HiDpiMode::Default);
        assert_eq!(platform.hidpi_factor, 1.0);
        assert_eq!(platform.cursor_cache, None);
        assert!(!platform.ime_enabled);
    }

    #[test]
    fn test_hidpi_mode_setting() {
        let _guard = lock_context();
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
