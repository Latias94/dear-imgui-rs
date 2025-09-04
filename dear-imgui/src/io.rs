//! Input/Output system for Dear ImGui
//!
//! This module provides access to Dear ImGui's IO system, which manages
//! input events, configuration flags, and various settings.

use crate::types::{Key, MouseButton, Vec2};
use dear_imgui_sys as sys;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

bitflags::bitflags! {
    /// Configuration flags for Dear ImGui
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct ConfigFlags: i32 {
        /// Master keyboard navigation enable flag
        const NAV_ENABLE_KEYBOARD = sys::ImGuiConfigFlags_NavEnableKeyboard;
        /// Master gamepad navigation enable flag
        const NAV_ENABLE_GAMEPAD = sys::ImGuiConfigFlags_NavEnableGamepad;
        /// Instruction navigation to move the mouse cursor
        const NAV_ENABLE_SET_MOUSE_POS = sys::ImGuiConfigFlags_NavEnableSetMousePos;
        /// Instruction navigation to not set the want_capture_keyboard flag
        const NAV_NO_CAPTURE_KEYBOARD = sys::ImGuiConfigFlags_NavNoCaptureKeyboard;
        /// Instruction imgui to clear mouse position/buttons in frame()
        const NO_MOUSE = sys::ImGuiConfigFlags_NoMouse;
        /// Instruction backend to not alter mouse cursor shape and visibility
        const NO_MOUSE_CURSOR_CHANGE = sys::ImGuiConfigFlags_NoMouseCursorChange;
        /// Application is SRGB-aware
        const IS_SRGB = sys::ImGuiConfigFlags_IsSRGB;
        /// Application is using a touch screen instead of a mouse
        const IS_TOUCH_SCREEN = sys::ImGuiConfigFlags_IsTouchScreen;
        /// Enable docking functionality
        const DOCKING_ENABLE = sys::ImGuiConfigFlags_DockingEnable;
        /// Enable multi-viewport functionality
        const VIEWPORTS_ENABLE = sys::ImGuiConfigFlags_ViewportsEnable;
    }
}

bitflags::bitflags! {
    /// Backend capability flags
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct BackendFlags: i32 {
        /// Backend supports gamepad and currently has one connected
        const HAS_GAMEPAD = sys::ImGuiBackendFlags_HasGamepad;
        /// Backend supports honoring get_mouse_cursor() value to change the OS cursor shape
        const HAS_MOUSE_CURSORS = sys::ImGuiBackendFlags_HasMouseCursors;
        /// Backend supports io.want_set_mouse_pos requests to reposition the OS mouse position
        const HAS_SET_MOUSE_POS = sys::ImGuiBackendFlags_HasSetMousePos;
        /// Backend renderer supports DrawCmd::vtx_offset
        const RENDERER_HAS_VTX_OFFSET = sys::ImGuiBackendFlags_RendererHasVtxOffset;
        /// Backend supports multiple viewports
        const PLATFORM_HAS_VIEWPORTS = sys::ImGuiBackendFlags_PlatformHasViewports;
        /// Backend supports calling NewFrame() without a valid render target
        const RENDERER_HAS_VIEWPORTS = sys::ImGuiBackendFlags_RendererHasViewports;
    }
}

/// Mouse cursor types
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MouseCursor {
    None = sys::ImGuiMouseCursor_None,
    Arrow = sys::ImGuiMouseCursor_Arrow,
    TextInput = sys::ImGuiMouseCursor_TextInput,
    ResizeAll = sys::ImGuiMouseCursor_ResizeAll,
    ResizeNS = sys::ImGuiMouseCursor_ResizeNS,
    ResizeEW = sys::ImGuiMouseCursor_ResizeEW,
    ResizeNESW = sys::ImGuiMouseCursor_ResizeNESW,
    ResizeNWSE = sys::ImGuiMouseCursor_ResizeNWSE,
    Hand = sys::ImGuiMouseCursor_Hand,
    NotAllowed = sys::ImGuiMouseCursor_NotAllowed,
}

/// Dear ImGui IO structure
///
/// This is the main communication struct between your application and Dear ImGui.
/// Access via `Context::io()` or `Context::io_mut()`.
pub struct Io {
    raw: *mut sys::ImGuiIO,
}

impl Io {
    /// Create a new IO wrapper from a raw pointer
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid and lives as long as this Io
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImGuiIO) -> Self {
        Self { raw }
    }

    /// Get the raw ImGuiIO pointer
    pub(crate) fn raw(&self) -> *const sys::ImGuiIO {
        self.raw
    }

    /// Get the raw ImGuiIO pointer (mutable)
    pub(crate) fn raw_mut(&mut self) -> *mut sys::ImGuiIO {
        self.raw
    }

    /// Get configuration flags
    pub fn config_flags(&self) -> ConfigFlags {
        unsafe { ConfigFlags::from_bits_truncate((*self.raw).ConfigFlags as i32) }
    }

    /// Set configuration flags
    pub fn set_config_flags(&mut self, flags: ConfigFlags) {
        unsafe {
            (*self.raw).ConfigFlags = flags.bits();
        }
    }

    /// Get backend flags
    pub fn backend_flags(&self) -> BackendFlags {
        unsafe { BackendFlags::from_bits_truncate((*self.raw).BackendFlags as i32) }
    }

    /// Set backend flags
    pub fn set_backend_flags(&mut self, flags: BackendFlags) {
        unsafe {
            (*self.raw).BackendFlags = flags.bits();
        }
    }

    /// Get display size in pixels
    pub fn display_size(&self) -> Vec2 {
        unsafe {
            let size = (*self.raw).DisplaySize;
            Vec2::new(size.x, size.y)
        }
    }

    /// Set display size in pixels
    pub fn set_display_size(&mut self, size: Vec2) {
        unsafe {
            (*self.raw).DisplaySize = sys::ImVec2 {
                x: size.x,
                y: size.y,
            };
        }
    }

    /// Get time elapsed since last frame, in seconds
    pub fn delta_time(&self) -> f32 {
        unsafe { (*self.raw).DeltaTime }
    }

    /// Set time elapsed since last frame, in seconds
    pub fn set_delta_time(&mut self, delta_time: f32) {
        unsafe {
            (*self.raw).DeltaTime = delta_time;
        }
    }

    /// Get global font scale
    pub fn font_global_scale(&self) -> f32 {
        unsafe { (*self.raw).FontGlobalScale }
    }

    /// Set global font scale
    pub fn set_font_global_scale(&mut self, scale: f32) {
        unsafe {
            (*self.raw).FontGlobalScale = scale;
        }
    }

    /// Get mouse position
    pub fn mouse_pos(&self) -> Vec2 {
        unsafe {
            let pos = (*self.raw).MousePos;
            Vec2::new(pos.x, pos.y)
        }
    }

    /// Set mouse position
    pub fn set_mouse_pos(&mut self, pos: Vec2) {
        unsafe {
            (*self.raw).MousePos = sys::ImVec2 { x: pos.x, y: pos.y };
        }
    }

    /// Get mouse button state
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        unsafe { (*self.raw).MouseDown[button as usize] }
    }

    /// Set mouse button state
    pub fn set_mouse_down(&mut self, button: MouseButton, down: bool) {
        unsafe {
            (*self.raw).MouseDown[button as usize] = down;
        }
    }

    /// Get mouse wheel delta
    pub fn mouse_wheel(&self) -> f32 {
        unsafe { (*self.raw).MouseWheel }
    }

    /// Set mouse wheel delta
    pub fn set_mouse_wheel(&mut self, wheel: f32) {
        unsafe {
            (*self.raw).MouseWheel = wheel;
        }
    }

    /// Get horizontal mouse wheel delta
    pub fn mouse_wheel_h(&self) -> f32 {
        unsafe { (*self.raw).MouseWheelH }
    }

    /// Set horizontal mouse wheel delta
    pub fn set_mouse_wheel_h(&mut self, wheel_h: f32) {
        unsafe {
            (*self.raw).MouseWheelH = wheel_h;
        }
    }

    /// Check if Dear ImGui wants to capture mouse input
    pub fn want_capture_mouse(&self) -> bool {
        unsafe { (*self.raw).WantCaptureMouse }
    }

    /// Check if Dear ImGui wants to capture keyboard input
    pub fn want_capture_keyboard(&self) -> bool {
        unsafe { (*self.raw).WantCaptureKeyboard }
    }

    /// Check if Dear ImGui wants text input
    pub fn want_text_input(&self) -> bool {
        unsafe { (*self.raw).WantTextInput }
    }

    /// Check if Dear ImGui wants to set mouse position
    pub fn want_set_mouse_pos(&self) -> bool {
        unsafe { (*self.raw).WantSetMousePos }
    }

    /// Get framerate estimate
    pub fn framerate(&self) -> f32 {
        unsafe { (*self.raw).Framerate }
    }

    /// Add character input
    pub fn add_input_character(&mut self, character: char) {
        let mut buf = [0; 5];
        character.encode_utf8(&mut buf);
        unsafe {
            sys::ImGuiIO_AddInputCharactersUTF8(self.raw, buf.as_ptr() as *const c_char);
        }
    }

    /// Add key event
    pub fn add_key_event(&mut self, key: Key, down: bool) {
        unsafe {
            sys::ImGuiIO_AddKeyEvent(self.raw, key as i32, down);
        }
    }

    /// Add mouse position event
    pub fn add_mouse_pos_event(&mut self, x: f32, y: f32) {
        unsafe {
            sys::ImGuiIO_AddMousePosEvent(self.raw, x, y);
        }
    }

    /// Add mouse button event
    pub fn add_mouse_button_event(&mut self, button: MouseButton, down: bool) {
        unsafe {
            sys::ImGuiIO_AddMouseButtonEvent(self.raw, button as i32, down);
        }
    }

    /// Add mouse wheel event
    pub fn add_mouse_wheel_event(&mut self, wheel_x: f32, wheel_y: f32) {
        unsafe {
            sys::ImGuiIO_AddMouseWheelEvent(self.raw, wheel_x, wheel_y);
        }
    }

    /// Add focus event
    pub fn add_focus_event(&mut self, focused: bool) {
        unsafe {
            sys::ImGuiIO_AddFocusEvent(self.raw, focused);
        }
    }

    /// Clear input characters
    pub fn clear_input_characters(&mut self) {
        unsafe {
            sys::ImGuiIO_ClearInputKeys(self.raw);
        }
    }

    /// Set INI filename (None to disable)
    pub fn set_ini_filename(&mut self, filename: Option<&str>) {
        unsafe {
            if let Some(filename) = filename {
                let c_filename = std::ffi::CString::new(filename).unwrap();
                (*self.raw).IniFilename = c_filename.as_ptr();
                // Note: This leaks memory, but Dear ImGui expects the string to live forever
                std::mem::forget(c_filename);
            } else {
                (*self.raw).IniFilename = std::ptr::null();
            }
        }
    }

    /// Get INI filename
    pub fn ini_filename(&self) -> Option<&str> {
        unsafe {
            let ptr = (*self.raw).IniFilename;
            if ptr.is_null() {
                None
            } else {
                CStr::from_ptr(ptr).to_str().ok()
            }
        }
    }
}
