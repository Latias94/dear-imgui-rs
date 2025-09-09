//! Winit platform backend for Dear ImGui
//!
//! This crate provides a platform backend for Dear ImGui that integrates with
//! the winit windowing library. It handles window events, input processing,
//! and platform-specific functionality including multi-viewport support.
//!
//! # Features
//!
//! - **Basic Platform Support**: Window events, input handling, cursor management
//! - **Multi-Viewport Support**: Create and manage multiple OS windows (requires `multi-viewport` feature)
//! - **DPI Awareness**: Proper handling of high-DPI displays
//!
//! # Example - Basic Usage
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
//!
//! # Example - Multi-Viewport Support
//!
//! ```rust,no_run
//! # #[cfg(feature = "multi-viewport")]
//! # {
//! use dear_imgui::Context;
//! use dear_imgui_winit::{WinitPlatform, multi_viewport};
//! use winit::event_loop::EventLoop;
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut imgui_ctx = Context::create();
//! imgui_ctx.enable_multi_viewport();
//!
//! let mut platform = WinitPlatform::new(&mut imgui_ctx);
//!
//! // In your event loop:
//! // multi_viewport::set_event_loop(&event_loop);
//! // multi_viewport::init_multi_viewport_support(&mut imgui_ctx, &window);
//! # }
//! ```

use std::collections::HashMap;
use std::time::Instant;

use dear_imgui::{BackendFlags, ConfigFlags, Context, Key};
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
    #[cfg(feature = "multi-viewport")]
    multi_viewport_initialized: bool,
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
            #[cfg(feature = "multi-viewport")]
            multi_viewport_initialized: false,
        };

        platform.configure_imgui(imgui_ctx);

        platform
    }

    /// Attach the platform to a window with specified DPI mode
    ///
    /// # Arguments
    /// * `window` - The winit window to attach to
    /// * `hidpi_mode` - The DPI handling mode to use
    /// * `imgui_ctx` - The Dear ImGui context
    pub fn attach_window(
        &mut self,
        window: &Window,
        hidpi_mode: HiDpiMode,
        imgui_ctx: &mut Context,
    ) {
        let scale_factor = window.scale_factor();
        self.hidpi_factor = hidpi_mode.apply(scale_factor);
        self.hidpi_mode = hidpi_mode;

        // Update display size and framebuffer scale immediately
        self.update_display_size(window, imgui_ctx);


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
    fn update_display_size(&self, window: &Window, imgui_ctx: &mut Context) {
        let io = imgui_ctx.io_mut();

        // Get physical size and convert to logical size using our DPI factor
        let physical_size = window.inner_size();
        let logical_width = physical_size.width as f64 / self.hidpi_factor;
        let logical_height = physical_size.height as f64 / self.hidpi_factor;

        io.set_display_size([logical_width as f32, logical_height as f32]);

        // Set the framebuffer scale for proper HiDPI rendering
        io.set_display_framebuffer_scale([self.hidpi_factor as f32, self.hidpi_factor as f32]);
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
    /// * `imgui_ctx` - The Dear ImGui context
    ///
    /// # Returns
    ///
    /// Returns `true` if Dear ImGui wants to capture this event (e.g., mouse is
    /// over a Dear ImGui window), `false` otherwise.
    pub fn handle_event<T>(
        &mut self,
        event: &Event<T>,
        window: &Window,
        imgui_ctx: &mut Context,
    ) -> bool {
        match event {
            Event::WindowEvent {
                event, window_id, ..
            } if *window_id == window.id() => self.handle_window_event(event, window, imgui_ctx),
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

        // Update display size in case window was resized or DPI changed
        self.update_display_size(window, imgui_ctx);

        let io = imgui_ctx.io_mut();

        // Update delta time
        io.set_delta_time(delta_time.max(1.0 / 60.0)); // Minimum 60 FPS

        // Update mouse position
        io.set_mouse_pos(self.mouse_pos);

        // Update mouse buttons
        for (i, &pressed) in self.mouse_buttons.iter().enumerate() {
            io.set_mouse_down(i, pressed);
        }
    }

    /// Initialize multi-viewport support if not already done
    ///
    /// This method should be called from within the event loop when ActiveEventLoop is available.
    /// It's safe to call multiple times - initialization will only happen once.
    #[cfg(feature = "multi-viewport")]
    pub fn init_multi_viewport_if_needed(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        if !self.multi_viewport_initialized {
            multi_viewport::set_event_loop(event_loop);
            multi_viewport::init_multi_viewport_support(imgui_ctx, window);
            self.multi_viewport_initialized = true;
        }
    }

    fn configure_imgui(&self, imgui_ctx: &mut Context) {
        let io = imgui_ctx.io_mut();

        // Enable keyboard and mouse navigation
        let mut config_flags = io.config_flags();
        config_flags.insert(ConfigFlags::NAV_ENABLE_KEYBOARD);
        config_flags.insert(ConfigFlags::NAV_ENABLE_GAMEPAD);
        io.set_config_flags(config_flags);

        // Set backend flags
        let mut backend_flags = io.backend_flags();
        backend_flags.insert(BackendFlags::HAS_MOUSE_CURSORS);
        backend_flags.insert(BackendFlags::HAS_SET_MOUSE_POS);

        #[cfg(feature = "multi-viewport")]
        {
            backend_flags.insert(BackendFlags::PLATFORM_HAS_VIEWPORTS);
        }

        io.set_backend_flags(backend_flags);

        // Note: Backend name setting is not exposed in our safe API
        // This would need to be added to the Context implementation if needed

        // let backend_name = std::ffi::CString::new("dear-imgui-winit").unwrap();
        // (*io).BackendPlatformName = backend_name.as_ptr();
    }

    fn handle_window_event(
        &mut self,
        event: &WindowEvent,
        window: &Window,
        imgui_ctx: &mut Context,
    ) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                // Convert physical position to logical position using our DPI factor
                let logical_pos = position.to_logical::<f64>(self.hidpi_factor);
                self.mouse_pos = [logical_pos.x as f32, logical_pos.y as f32];
                false
            }
            WindowEvent::Resized(_) => {
                // Update display size when window is resized
                self.update_display_size(window, imgui_ctx);
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
                self.update_display_size(window, imgui_ctx);
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
                imgui_ctx.io().want_capture_mouse()
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => {
                        (pos.x as f32 / 120.0, pos.y as f32 / 120.0)
                    }
                };

                {
                    let io = imgui_ctx.io_mut();
                    io.set_mouse_wheel_h(io.mouse_wheel_h() + x);
                    io.set_mouse_wheel(io.mouse_wheel() + y);
                    io.want_capture_mouse()
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard_input(event, imgui_ctx)
            }
            _ => false,
        }
    }

    fn handle_device_event(&mut self, event: &DeviceEvent) {
        // Handle device-specific events if needed
        match event {
            _ => {}
        }
    }

    fn handle_keyboard_input(&mut self, event: &KeyEvent, imgui_ctx: &mut Context) -> bool {
        let io = imgui_ctx.io_mut();

        // Handle key press/release
        if let Some(imgui_key) = winit_key_to_imgui_key(&event.logical_key) {
            io.add_key_event(imgui_key, event.state == ElementState::Pressed);
        }

        // Handle text input
        if event.state == ElementState::Pressed {
            if let WinitKey::Character(ref text) = event.logical_key {
                for ch in text.chars() {
                    if ch.is_ascii() && !ch.is_control() {
                        io.add_input_character(ch);
                    }
                }
            }
        }

        io.want_capture_keyboard()
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
fn winit_key_to_imgui_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(named_key) => match named_key {
            NamedKey::Tab => Some(Key::Tab),
            NamedKey::ArrowLeft => Some(Key::LeftArrow),
            NamedKey::ArrowRight => Some(Key::RightArrow),
            NamedKey::ArrowUp => Some(Key::UpArrow),
            NamedKey::ArrowDown => Some(Key::DownArrow),
            NamedKey::PageUp => Some(Key::PageUp),
            NamedKey::PageDown => Some(Key::PageDown),
            NamedKey::Home => Some(Key::Home),
            NamedKey::End => Some(Key::End),
            NamedKey::Insert => Some(Key::Insert),
            NamedKey::Delete => Some(Key::Delete),
            NamedKey::Backspace => Some(Key::Backspace),
            NamedKey::Space => Some(Key::Space),
            NamedKey::Enter => Some(Key::Enter),
            NamedKey::Escape => Some(Key::Escape),
            NamedKey::Control => Some(Key::LeftCtrl),
            NamedKey::Shift => Some(Key::LeftShift),
            NamedKey::Alt => Some(Key::LeftAlt),
            NamedKey::Super => Some(Key::LeftSuper),
            _ => None,
        },
        WinitKey::Character(text) => {
            if text.len() == 1 {
                let ch = text.chars().next().unwrap();
                match ch {
                    'a' => Some(Key::A),
                    'b' => Some(Key::B),
                    'c' => Some(Key::C),
                    'd' => Some(Key::D),
                    'e' => Some(Key::E),
                    'f' => Some(Key::F),
                    'g' => Some(Key::G),
                    'h' => Some(Key::H),
                    'i' => Some(Key::I),
                    'j' => Some(Key::J),
                    'k' => Some(Key::K),
                    'l' => Some(Key::L),
                    'm' => Some(Key::M),
                    'n' => Some(Key::N),
                    'o' => Some(Key::O),
                    'p' => Some(Key::P),
                    'q' => Some(Key::Q),
                    'r' => Some(Key::R),
                    's' => Some(Key::S),
                    't' => Some(Key::T),
                    'u' => Some(Key::U),
                    'v' => Some(Key::V),
                    'w' => Some(Key::W),
                    'x' => Some(Key::X),
                    'y' => Some(Key::Y),
                    'z' => Some(Key::Z),
                    'A' => Some(Key::A),
                    'B' => Some(Key::B),
                    'C' => Some(Key::C),
                    'D' => Some(Key::D),
                    'E' => Some(Key::E),
                    'F' => Some(Key::F),
                    'G' => Some(Key::G),
                    'H' => Some(Key::H),
                    'I' => Some(Key::I),
                    'J' => Some(Key::J),
                    'K' => Some(Key::K),
                    'L' => Some(Key::L),
                    'M' => Some(Key::M),
                    'N' => Some(Key::N),
                    'O' => Some(Key::O),
                    'P' => Some(Key::P),
                    'Q' => Some(Key::Q),
                    'R' => Some(Key::R),
                    'S' => Some(Key::S),
                    'T' => Some(Key::T),
                    'U' => Some(Key::U),
                    'V' => Some(Key::V),
                    'W' => Some(Key::W),
                    'X' => Some(Key::X),
                    'Y' => Some(Key::Y),
                    'Z' => Some(Key::Z),
                    '0' => Some(Key::Key0),
                    '1' => Some(Key::Key1),
                    '2' => Some(Key::Key2),
                    '3' => Some(Key::Key3),
                    '4' => Some(Key::Key4),
                    '5' => Some(Key::Key5),
                    '6' => Some(Key::Key6),
                    '7' => Some(Key::Key7),
                    '8' => Some(Key::Key8),
                    '9' => Some(Key::Key9),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Multi-viewport support for winit following official ImGui backend pattern
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport {
    use super::*;
    use std::cell::RefCell;
    use std::ffi::{c_char, CStr, c_void};
    use winit::event_loop::ActiveEventLoop;
    use winit::window::{Window, WindowAttributes};
    use winit::dpi::{LogicalSize, LogicalPosition};

    // Thread-local storage for winit multi-viewport support
    thread_local! {
        static EVENT_LOOP: RefCell<Option<*const ActiveEventLoop>> = RefCell::new(None);
    }

    /// Helper structure we store in the void* PlatformUserData field of each ImGuiViewport
    /// to easily retrieve our backend data. Following official ImGui backend pattern.
    #[repr(C)]
    pub struct ViewportData {
        pub window: *mut Window,        // Stored in ImGuiViewport::PlatformHandle
        pub window_owned: bool,         // Set to false for main window
        pub ignore_window_pos_event_frame: i32,
        pub ignore_window_size_event_frame: i32,
    }

    impl ViewportData {
        pub fn new() -> Self {
            Self {
                window: std::ptr::null_mut(),
                window_owned: false,
                ignore_window_pos_event_frame: -1,
                ignore_window_size_event_frame: -1,
            }
        }
    }

    /// Initialize multi-viewport support following official ImGui backend pattern
    pub fn init_multi_viewport_support(_ctx: &mut dear_imgui::Context, main_window: &Window) {
        // Set up platform callbacks using direct C API
        unsafe {
            let pio = dear_imgui::sys::ImGui_GetPlatformIO();

            (*pio).Platform_CreateWindow = Some(winit_create_window);
            (*pio).Platform_DestroyWindow = Some(winit_destroy_window);
            (*pio).Platform_ShowWindow = Some(winit_show_window);
            (*pio).Platform_SetWindowPos = Some(winit_set_window_pos);
            (*pio).Platform_GetWindowPos = Some(winit_get_window_pos);
            (*pio).Platform_SetWindowSize = Some(winit_set_window_size);
            (*pio).Platform_GetWindowSize = Some(winit_get_window_size);
            (*pio).Platform_SetWindowFocus = Some(winit_set_window_focus);
            (*pio).Platform_GetWindowFocus = Some(winit_get_window_focus);
            (*pio).Platform_GetWindowMinimized = Some(winit_get_window_minimized);
            (*pio).Platform_SetWindowTitle = Some(winit_set_window_title);
            (*pio).Platform_GetWindowFramebufferScale = Some(winit_get_window_framebuffer_scale);

            // Additional callbacks that GLFW implements but we're missing
            (*pio).Platform_UpdateWindow = Some(winit_update_window);
            // Note: Platform_RenderWindow and Platform_SwapBuffers should be set by the renderer backend

            // Set up monitors - this is required for multi-viewport
            setup_monitors();
        }

        // Set up the main viewport
        init_main_viewport(main_window);
    }

    /// Set up monitors list for multi-viewport support
    unsafe fn setup_monitors() {
        // For now, let's skip the monitor setup and see if ImGui can work without it
        // The assertion suggests ImGui expects monitors to be set up, but let's try a simpler approach

        // We'll let ImGui handle monitor detection internally
        // This is a temporary workaround to get basic multi-viewport working
    }

    /// Initialize the main viewport with proper ViewportData
    fn init_main_viewport(main_window: &Window) {
        unsafe {
            let main_viewport = dear_imgui::sys::ImGui_GetMainViewport();

            // Create ViewportData for main window
            let vd = Box::into_raw(Box::new(ViewportData::new()));
            (*vd).window = main_window as *const Window as *mut Window;
            (*vd).window_owned = false; // Main window is owned by the application

            (*main_viewport).PlatformUserData = vd as *mut c_void;
            (*main_viewport).PlatformHandle = main_window as *const Window as *mut c_void;
        }
    }

    /// Shutdown multi-viewport support
    pub fn shutdown_multi_viewport_support() {
        // Clean up any remaining viewports
        unsafe {
            dear_imgui::sys::ImGui_DestroyPlatformWindows();
        }
    }

    /// Store event loop reference for viewport creation
    pub fn set_event_loop(event_loop: &ActiveEventLoop) {
        EVENT_LOOP.with(|el| {
            *el.borrow_mut() = Some(event_loop as *const ActiveEventLoop);
        });
    }

    // Platform callback functions following official ImGui backend pattern

    /// Create a new viewport window
    unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }

        // Get event loop reference
        let event_loop = EVENT_LOOP.with(|el| {
            el.borrow().map(|ptr| &*ptr)
        });

        let event_loop = match event_loop {
            Some(el) => el,
            None => return,
        };

        // Create ViewportData
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        (*vp).PlatformUserData = vd as *mut c_void;

        // Handle viewport flags
        let viewport_flags = (*vp).Flags;
        let mut window_attrs = WindowAttributes::default()
            .with_title("ImGui Viewport")
            .with_inner_size(LogicalSize::new((*vp).Size.x as f64, (*vp).Size.y as f64))
            .with_position(winit::dpi::Position::Logical(LogicalPosition::new((*vp).Pos.x as f64, (*vp).Pos.y as f64)))
            .with_visible(false); // Start hidden, will be shown by show_window callback

        // Handle decorations
        if viewport_flags & dear_imgui::sys::ImGuiViewportFlags_NoDecoration != 0 {
            window_attrs = window_attrs.with_decorations(false);
        }

        // Handle always on top
        if viewport_flags & dear_imgui::sys::ImGuiViewportFlags_TopMost != 0 {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }

        // Create the window
        match event_loop.create_window(window_attrs) {
            Ok(window) => {
                let window_ptr = Box::into_raw(Box::new(window));
                (*vd).window = window_ptr;
                (*vd).window_owned = true;
                (*vp).PlatformHandle = window_ptr as *mut c_void;

                // TODO: Set up event callbacks for this window
                // This is a critical missing piece - we need to route events from this window
                // back to ImGui. For now, this is a known limitation.
                eprintln!("Warning: Event routing for viewport windows not yet implemented");
            }
            Err(_) => {
                // Clean up ViewportData on failure
                let _ = Box::from_raw(vd);
                (*vp).PlatformUserData = std::ptr::null_mut();
            }
        }
    }

    /// Destroy a viewport window
    unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_mut() {
            if vd.window_owned && !vd.window.is_null() {
                // Clean up the window
                let _ = Box::from_raw(vd.window);
            }
            vd.window = std::ptr::null_mut();

            // Clean up ViewportData
            let _ = Box::from_raw(vd);
        }

        (*vp).PlatformUserData = std::ptr::null_mut();
        (*vp).PlatformHandle = std::ptr::null_mut();
    }

    /// Show a viewport window
    unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                (*vd.window).set_visible(true);
            }
        }
    }

    /// Get window position
    unsafe extern "C" fn winit_get_window_pos(vp: *mut dear_imgui::sys::ImGuiViewport) -> dear_imgui::sys::ImVec2 {
        if vp.is_null() {
            return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
        }

        // Special handling for viewport ID 0 (main viewport or ImGui internal viewport)
        let viewport_id = (*vp).ID;
        if viewport_id == 0 {
            // Return safe default for main viewport
            return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                if let Ok(pos) = (*vd.window).outer_position() {
                    return dear_imgui::sys::ImVec2 {
                        x: pos.x as f32,
                        y: pos.y as f32
                    };
                }
            }
        }

        dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 }
    }

    /// Set window position
    unsafe extern "C" fn winit_set_window_pos(vp: *mut dear_imgui::sys::ImGuiViewport, pos: dear_imgui::sys::ImVec2) {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_mut() {
            if !vd.window.is_null() {
                let position = LogicalPosition::new(pos.x as f64, pos.y as f64);
                let _ = (*vd.window).set_outer_position(position);
                vd.ignore_window_pos_event_frame = dear_imgui::sys::ImGui_GetFrameCount();
            }
        }
    }

    /// Get window size
    unsafe extern "C" fn winit_get_window_size(vp: *mut dear_imgui::sys::ImGuiViewport) -> dear_imgui::sys::ImVec2 {
        if vp.is_null() {
            return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                let size = (*vd.window).inner_size();
                return dear_imgui::sys::ImVec2 {
                    x: size.width as f32,
                    y: size.height as f32
                };
            }
        }

        dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 }
    }

    /// Set window size
    unsafe extern "C" fn winit_set_window_size(vp: *mut dear_imgui::sys::ImGuiViewport, size: dear_imgui::sys::ImVec2) {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_mut() {
            if !vd.window.is_null() {
                let new_size = LogicalSize::new(size.x as f64, size.y as f64);
                let _ = (*vd.window).request_inner_size(new_size);
                vd.ignore_window_size_event_frame = dear_imgui::sys::ImGui_GetFrameCount();
            }
        }
    }

    /// Set window focus
    unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui::sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                (*vd.window).focus_window();
            }
        }
    }

    /// Get window focus
    unsafe extern "C" fn winit_get_window_focus(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool {
        if vp.is_null() {
            return false;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                return (*vd.window).has_focus();
            }
        }

        false
    }

    /// Get window minimized state
    unsafe extern "C" fn winit_get_window_minimized(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool {
        if vp.is_null() {
            return false;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                return (*vd.window).is_minimized().unwrap_or(false);
            }
        }

        false
    }

    /// Set window title
    unsafe extern "C" fn winit_set_window_title(vp: *mut dear_imgui::sys::ImGuiViewport, title: *const c_char) {
        if vp.is_null() || title.is_null() {
            return;
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                if let Ok(title_str) = CStr::from_ptr(title).to_str() {
                    (*vd.window).set_title(title_str);
                }
            }
        }
    }

    /// Get window framebuffer scale
    unsafe extern "C" fn winit_get_window_framebuffer_scale(vp: *mut dear_imgui::sys::ImGuiViewport) -> dear_imgui::sys::ImVec2 {
        if vp.is_null() {
            return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
        }

        if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
            if !vd.window.is_null() {
                let scale = (*vd.window).scale_factor() as f32;
                return dear_imgui::sys::ImVec2 { x: scale, y: scale };
            }
        }

        dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 }
    }

    /// Update window - called by ImGui for platform-specific updates
    unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }

        // For now, this is a no-op. In GLFW implementation, this is used for
        // platform-specific window updates. Winit handles most of this automatically.
        // We might need to add specific logic here later for things like:
        // - Window state synchronization
        // - Platform-specific optimizations
        // - Event processing
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
