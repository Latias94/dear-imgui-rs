use bitflags::bitflags;

use crate::sys;

bitflags! {
    /// Configuration flags
    #[repr(transparent)]
    pub struct ConfigFlags: i32 {
        /// Master keyboard navigation enable flag.
        const NAV_ENABLE_KEYBOARD = sys::ImGuiConfigFlags_NavEnableKeyboard;
        /// Master gamepad navigation enable flag.
        const NAV_ENABLE_GAMEPAD = sys::ImGuiConfigFlags_NavEnableGamepad;
        /// Instruction imgui-rs to clear mouse position/buttons in `frame()`.
        const NO_MOUSE = sys::ImGuiConfigFlags_NoMouse;
        /// Instruction backend to not alter mouse cursor shape and visibility.
        const NO_MOUSE_CURSOR_CHANGE = sys::ImGuiConfigFlags_NoMouseCursorChange;
        /// Application is SRGB-aware.
        const IS_SRGB = sys::ImGuiConfigFlags_IsSRGB;
        /// Application is using a touch screen instead of a mouse.
        const IS_TOUCH_SCREEN = sys::ImGuiConfigFlags_IsTouchScreen;

        #[cfg(feature = "docking")]
        const DOCKING_ENABLE = sys::ImGuiConfigFlags_DockingEnable;

        #[cfg(feature = "docking")]
        const VIEWPORTS_ENABLE = sys::ImGuiConfigFlags_ViewportsEnable;
    }
}

bitflags! {
    /// Backend capabilities
    #[repr(transparent)]
    pub struct BackendFlags: i32 {
        /// Backend supports gamepad and currently has one connected
        const HAS_GAMEPAD = sys::ImGuiBackendFlags_HasGamepad;
        /// Backend supports honoring `get_mouse_cursor` value to change the OS cursor shape
        const HAS_MOUSE_CURSORS = sys::ImGuiBackendFlags_HasMouseCursors;
        /// Backend supports `io.want_set_mouse_pos` requests to reposition the OS mouse position.
        const HAS_SET_MOUSE_POS = sys::ImGuiBackendFlags_HasSetMousePos;
        /// Backend renderer supports DrawCmd::vtx_offset.
        const RENDERER_HAS_VTX_OFFSET = sys::ImGuiBackendFlags_RendererHasVtxOffset;

        #[cfg(feature = "multi-viewport")]
        /// Set if the platform backend supports viewports.
        const PLATFORM_HAS_VIEWPORTS = sys::ImGuiBackendFlags_PlatformHasViewports;
        #[cfg(feature = "multi-viewport")]
        /// Set if the renderer backend supports viewports.
        const RENDERER_HAS_VIEWPORTS = sys::ImGuiBackendFlags_RendererHasViewports;
    }
}

/// Settings and inputs/outputs for imgui-rs
/// This is a transparent wrapper around ImGuiIO
#[repr(transparent)]
pub struct Io(sys::ImGuiIO);

impl Io {
    /// Creates a new Io instance from the current context
    pub(crate) fn from_raw() -> &'static mut Self {
        unsafe {
            let io_ptr = sys::ImGui_GetIO();
            &mut *(io_ptr as *mut Self)
        }
    }

    /// Main display size in pixels
    pub fn display_size(&self) -> [f32; 2] {
        [self.0.DisplaySize.x, self.0.DisplaySize.y]
    }

    /// Set main display size in pixels
    pub fn set_display_size(&mut self, size: [f32; 2]) {
        self.0.DisplaySize.x = size[0];
        self.0.DisplaySize.y = size[1];
    }

    /// Time elapsed since last frame, in seconds
    pub fn delta_time(&self) -> f32 {
        self.0.DeltaTime
    }

    /// Set time elapsed since last frame, in seconds
    pub fn set_delta_time(&mut self, delta_time: f32) {
        self.0.DeltaTime = delta_time;
    }

    /// Mouse position, in pixels
    pub fn mouse_pos(&self) -> [f32; 2] {
        [self.0.MousePos.x, self.0.MousePos.y]
    }

    /// Set mouse position, in pixels
    pub fn set_mouse_pos(&mut self, pos: [f32; 2]) {
        self.0.MousePos.x = pos[0];
        self.0.MousePos.y = pos[1];
    }

    /// Mouse wheel vertical scrolling
    pub fn mouse_wheel(&self) -> f32 {
        self.0.MouseWheel
    }

    /// Set mouse wheel vertical scrolling
    pub fn set_mouse_wheel(&mut self, wheel: f32) {
        self.0.MouseWheel = wheel;
    }

    /// Mouse wheel horizontal scrolling
    pub fn mouse_wheel_h(&self) -> f32 {
        self.0.MouseWheelH
    }

    /// Set mouse wheel horizontal scrolling
    pub fn set_mouse_wheel_h(&mut self, wheel_h: f32) {
        self.0.MouseWheelH = wheel_h;
    }

    /// Check if a mouse button is down
    pub fn mouse_down(&self, button: usize) -> bool {
        if button < 5 {
            self.0.MouseDown[button]
        } else {
            false
        }
    }

    /// Set mouse button state
    pub fn set_mouse_down(&mut self, button: usize, down: bool) {
        if button < 5 {
            self.0.MouseDown[button] = down;
        }
    }

    /// Check if imgui wants to capture mouse input
    pub fn want_capture_mouse(&self) -> bool {
        self.0.WantCaptureMouse
    }

    /// Check if imgui wants to capture keyboard input
    pub fn want_capture_keyboard(&self) -> bool {
        self.0.WantCaptureKeyboard
    }

    /// Check if imgui wants to use text input
    pub fn want_text_input(&self) -> bool {
        self.0.WantTextInput
    }

    /// Check if imgui wants to set mouse position
    pub fn want_set_mouse_pos(&self) -> bool {
        self.0.WantSetMousePos
    }

    /// Check if imgui wants to save ini settings
    pub fn want_save_ini_settings(&self) -> bool {
        self.0.WantSaveIniSettings
    }

    /// Framerate estimation, in frames per second
    pub fn framerate(&self) -> f32 {
        self.0.Framerate
    }

    /// Vertices output during last call to render
    pub fn metrics_render_vertices(&self) -> i32 {
        self.0.MetricsRenderVertices
    }

    /// Indices output during last call to render
    pub fn metrics_render_indices(&self) -> i32 {
        self.0.MetricsRenderIndices
    }

    /// Number of visible windows
    pub fn metrics_render_windows(&self) -> i32 {
        self.0.MetricsRenderWindows
    }

    /// Number of active windows
    pub fn metrics_active_windows(&self) -> i32 {
        self.0.MetricsActiveWindows
    }

    /// Configuration flags
    pub fn config_flags(&self) -> ConfigFlags {
        ConfigFlags::from_bits_truncate(self.0.ConfigFlags)
    }

    /// Set configuration flags
    pub fn set_config_flags(&mut self, flags: ConfigFlags) {
        self.0.ConfigFlags = flags.bits();
    }

    /// Backend flags
    pub fn backend_flags(&self) -> BackendFlags {
        BackendFlags::from_bits_truncate(self.0.BackendFlags)
    }

    /// Set backend flags
    pub fn set_backend_flags(&mut self, flags: BackendFlags) {
        self.0.BackendFlags = flags.bits();
    }

    /// Add a key event to the input queue
    pub fn add_key_event(&mut self, key: crate::Key, down: bool) {
        unsafe {
            sys::ImGuiIO_AddKeyEvent(&mut self.0 as *mut _, key as i32, down);
        }
    }

    /// Add a character input event to the input queue
    pub fn add_input_character(&mut self, character: char) {
        unsafe {
            sys::ImGuiIO_AddInputCharacter(&mut self.0 as *mut _, character as u32);
        }
    }

    /// Add a mouse position event to the input queue
    pub fn add_mouse_pos_event(&mut self, pos: [f32; 2]) {
        unsafe {
            sys::ImGuiIO_AddMousePosEvent(&mut self.0 as *mut _, pos[0], pos[1]);
        }
    }

    /// Add a mouse button event to the input queue
    pub fn add_mouse_button_event(&mut self, button: crate::input::MouseButton, down: bool) {
        unsafe {
            sys::ImGuiIO_AddMouseButtonEvent(&mut self.0 as *mut _, button as i32, down);
        }
    }

    /// Add a mouse wheel event to the input queue
    pub fn add_mouse_wheel_event(&mut self, wheel: [f32; 2]) {
        unsafe {
            sys::ImGuiIO_AddMouseWheelEvent(&mut self.0 as *mut _, wheel[0], wheel[1]);
        }
    }

    /// Get the global font scale (not available in current Dear ImGui version)
    /// This is a placeholder for compatibility with imgui-rs
    pub fn font_global_scale(&self) -> f32 {
        1.0 // Default scale
    }

    /// Set the global font scale (not available in current Dear ImGui version)
    /// This is a placeholder for compatibility with imgui-rs
    pub fn set_font_global_scale(&mut self, _scale: f32) {
        // No-op for now, as FontGlobalScale field is not available
    }

    /// Get the display framebuffer scale
    pub fn display_framebuffer_scale(&self) -> [f32; 2] {
        unsafe {
            let scale = self.0.DisplayFramebufferScale;
            [scale.x, scale.y]
        }
    }

    /// Set the display framebuffer scale
    /// This is important for HiDPI displays to ensure proper rendering
    pub fn set_display_framebuffer_scale(&mut self, scale: [f32; 2]) {
        unsafe {
            self.0.DisplayFramebufferScale.x = scale[0];
            self.0.DisplayFramebufferScale.y = scale[1];
        }
    }
}
