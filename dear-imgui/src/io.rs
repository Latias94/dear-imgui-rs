//! IO: inputs, configuration and backend capabilities
//!
//! This module wraps Dear ImGui's `ImGuiIO` and related flag types. Access the
//! per-frame IO object via [`Ui::io`], then read inputs or tweak configuration
//! and backend capability flags.
//!
//! Example: enable docking and multi-viewports, and set renderer flags.
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! // Configure IO before starting a frame
//! let io = ctx.io_mut();
//! io.set_config_flags(io.config_flags() | ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
//! io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
//! # let _ = ctx.frame();
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use bitflags::bitflags;

use crate::sys;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, c_void};

#[cfg(feature = "serde")]
impl Serialize for ConfigFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ConfigFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(ConfigFlags::from_bits_truncate(bits))
    }
}

#[cfg(feature = "serde")]
impl Serialize for BackendFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for BackendFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(BackendFlags::from_bits_truncate(bits))
    }
}

#[cfg(all(feature = "serde", feature = "multi-viewport"))]
impl Serialize for ViewportFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(all(feature = "serde", feature = "multi-viewport"))]
impl<'de> Deserialize<'de> for ViewportFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(ViewportFlags::from_bits_truncate(bits))
    }
}

bitflags! {
    /// Configuration flags
    #[repr(transparent)]
    pub struct ConfigFlags: i32 {
        /// Master keyboard navigation enable flag.
        const NAV_ENABLE_KEYBOARD = sys::ImGuiConfigFlags_NavEnableKeyboard as i32;
        /// Master gamepad navigation enable flag.
        const NAV_ENABLE_GAMEPAD = sys::ImGuiConfigFlags_NavEnableGamepad as i32;
        /// Instruction imgui-rs to clear mouse position/buttons in `frame()`.
        const NO_MOUSE = sys::ImGuiConfigFlags_NoMouse as i32;
        /// Instruction backend to not alter mouse cursor shape and visibility.
        const NO_MOUSE_CURSOR_CHANGE = sys::ImGuiConfigFlags_NoMouseCursorChange as i32;
        /// Application is SRGB-aware.
        const IS_SRGB = sys::ImGuiConfigFlags_IsSRGB as i32;
        /// Application is using a touch screen instead of a mouse.
        const IS_TOUCH_SCREEN = sys::ImGuiConfigFlags_IsTouchScreen as i32;

        const DOCKING_ENABLE = sys::ImGuiConfigFlags_DockingEnable as i32;

        const VIEWPORTS_ENABLE = sys::ImGuiConfigFlags_ViewportsEnable as i32;
    }
}

bitflags! {
    /// Backend capabilities
    #[repr(transparent)]
    pub struct BackendFlags: i32 {
        /// Backend supports gamepad and currently has one connected
        const HAS_GAMEPAD = sys::ImGuiBackendFlags_HasGamepad as i32;
        /// Backend supports honoring `get_mouse_cursor` value to change the OS cursor shape
        const HAS_MOUSE_CURSORS = sys::ImGuiBackendFlags_HasMouseCursors as i32;
        /// Backend supports `io.want_set_mouse_pos` requests to reposition the OS mouse position.
        const HAS_SET_MOUSE_POS = sys::ImGuiBackendFlags_HasSetMousePos as i32;
        /// Backend can report which viewport the OS mouse is hovering via `add_mouse_viewport_event`
        const HAS_MOUSE_HOVERED_VIEWPORT =
            sys::ImGuiBackendFlags_HasMouseHoveredViewport as i32;
        /// Backend renderer supports DrawCmd::vtx_offset.
        const RENDERER_HAS_VTX_OFFSET = sys::ImGuiBackendFlags_RendererHasVtxOffset as i32;
        /// Backend renderer supports ImTextureData requests to create/update/destroy textures.
        const RENDERER_HAS_TEXTURES = sys::ImGuiBackendFlags_RendererHasTextures as i32;

        #[cfg(feature = "multi-viewport")]
        /// Set if the platform backend supports viewports.
        const PLATFORM_HAS_VIEWPORTS = sys::ImGuiBackendFlags_PlatformHasViewports as i32;
        #[cfg(feature = "multi-viewport")]
        /// Set if the renderer backend supports viewports.
        const RENDERER_HAS_VIEWPORTS = sys::ImGuiBackendFlags_RendererHasViewports as i32;
    }
}

#[cfg(feature = "multi-viewport")]
bitflags! {
    /// Viewport flags for multi-viewport support
    #[repr(transparent)]
    pub struct ViewportFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Represent a Platform Window
        const IS_PLATFORM_WINDOW = sys::ImGuiViewportFlags_IsPlatformWindow as i32;
        /// Represent a Platform Monitor (unused in our implementation)
        const IS_PLATFORM_MONITOR = sys::ImGuiViewportFlags_IsPlatformMonitor as i32;
        /// Platform Window: is created/managed by the application (rather than a dear imgui backend)
        const OWNED_BY_APP = sys::ImGuiViewportFlags_OwnedByApp as i32;
        /// Platform Window: Disable platform decorations: title bar, borders, etc.
        const NO_DECORATION = sys::ImGuiViewportFlags_NoDecoration as i32;
        /// Platform Window: Disable platform task bar icon (generally set on popups/tooltips, or all windows if ImGuiConfigFlags_ViewportsNoTaskBarIcon is set)
        const NO_TASK_BAR_ICON = sys::ImGuiViewportFlags_NoTaskBarIcon as i32;
        /// Platform Window: Don't take focus when created.
        const NO_FOCUS_ON_APPEARING = sys::ImGuiViewportFlags_NoFocusOnAppearing as i32;
        /// Platform Window: Don't take focus when clicked on.
        const NO_FOCUS_ON_CLICK = sys::ImGuiViewportFlags_NoFocusOnClick as i32;
        /// Platform Window: Make mouse pass through so we can drag this window while peaking behind it.
        const NO_INPUTS = sys::ImGuiViewportFlags_NoInputs as i32;
        /// Platform Window: Renderer doesn't need to clear the framebuffer ahead (because we will fill it entirely).
        const NO_RENDERER_CLEAR = sys::ImGuiViewportFlags_NoRendererClear as i32;
        /// Platform Window: Avoid merging this window into another host window. This can only be set via ImGuiWindowClass viewport flags override (because we need to now ahead if we are going to create a viewport in the first place!).
        const NO_AUTO_MERGE = sys::ImGuiViewportFlags_NoAutoMerge as i32;
        /// Platform Window: Display on top (for tooltips only).
        const TOP_MOST = sys::ImGuiViewportFlags_TopMost as i32;
        /// Viewport can host multiple imgui windows (secondary viewports are associated to a single window).
        const CAN_HOST_OTHER_WINDOWS = sys::ImGuiViewportFlags_CanHostOtherWindows as i32;
        /// Platform Window: Window is minimized, can skip render. When minimized we tend to avoid using the viewport pos/size for clipping rectangle computation.
        const IS_MINIMIZED = sys::ImGuiViewportFlags_IsMinimized as i32;
        /// Platform Window: Window is focused (last call to Platform_GetWindowFocus() returned true)
        const IS_FOCUSED = sys::ImGuiViewportFlags_IsFocused as i32;
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
            // Note: our bindings expose igGetIO_Nil which resolves against the
            // current context. Keep using _Nil variant until regular symbol is exported.
            //
            // SAFETY: `igGetIO_Nil()` returns a pointer owned by the currently active
            // ImGui context. The returned reference must not be used after the context
            // is destroyed or when another context becomes current.
            let io_ptr = sys::igGetIO_Nil();
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

    /// Auto-save interval for `.ini` settings, in seconds.
    #[doc(alias = "IniSavingRate")]
    pub fn ini_saving_rate(&self) -> f32 {
        self.0.IniSavingRate
    }

    /// Set auto-save interval for `.ini` settings, in seconds.
    #[doc(alias = "IniSavingRate")]
    pub fn set_ini_saving_rate(&mut self, seconds: f32) {
        self.0.IniSavingRate = seconds;
    }

    /// Returns the current `.ini` filename, or `None` if disabled.
    ///
    /// Note: to set this safely, use `Context::set_ini_filename()`.
    #[doc(alias = "IniFilename")]
    pub fn ini_filename(&self) -> Option<&CStr> {
        let ptr = self.0.IniFilename;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns the current `.log` filename, or `None` if disabled.
    ///
    /// Note: to set this safely, use `Context::set_log_filename()`.
    #[doc(alias = "LogFilename")]
    pub fn log_filename(&self) -> Option<&CStr> {
        let ptr = self.0.LogFilename;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns user data pointer stored in `ImGuiIO`.
    #[doc(alias = "UserData")]
    pub fn user_data(&self) -> *mut c_void {
        self.0.UserData
    }

    /// Set user data pointer stored in `ImGuiIO`.
    #[doc(alias = "UserData")]
    pub fn set_user_data(&mut self, user_data: *mut c_void) {
        self.0.UserData = user_data;
    }

    /// Returns whether font scaling via Ctrl+MouseWheel is enabled.
    #[doc(alias = "FontAllowUserScaling")]
    pub fn font_allow_user_scaling(&self) -> bool {
        self.0.FontAllowUserScaling
    }

    /// Set whether font scaling via Ctrl+MouseWheel is enabled.
    #[doc(alias = "FontAllowUserScaling")]
    pub fn set_font_allow_user_scaling(&mut self, enabled: bool) {
        self.0.FontAllowUserScaling = enabled;
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

    /// Check if a mouse button is down (typed).
    pub fn mouse_down_button(&self, button: crate::input::MouseButton) -> bool {
        self.mouse_down(button as i32 as usize)
    }

    /// Set mouse button state
    pub fn set_mouse_down(&mut self, button: usize, down: bool) {
        if button < 5 {
            self.0.MouseDown[button] = down;
        }
    }

    /// Set mouse button state (typed).
    pub fn set_mouse_down_button(&mut self, button: crate::input::MouseButton, down: bool) {
        self.set_mouse_down(button as i32 as usize, down);
    }

    /// Check if imgui wants to capture mouse input
    pub fn want_capture_mouse(&self) -> bool {
        self.0.WantCaptureMouse
    }

    /// Returns whether ImGui wants to capture mouse, unless a popup is closing.
    #[doc(alias = "WantCaptureMouseUnlessPopupClose")]
    pub fn want_capture_mouse_unless_popup_close(&self) -> bool {
        self.0.WantCaptureMouseUnlessPopupClose
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
    /// Whether ImGui requests software-drawn mouse cursor
    pub fn mouse_draw_cursor(&self) -> bool {
        self.0.MouseDrawCursor
    }
    /// Enable or disable software-drawn mouse cursor
    pub fn set_mouse_draw_cursor(&mut self, draw: bool) {
        self.0.MouseDrawCursor = draw;
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

    /// Returns whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn config_nav_swap_gamepad_buttons(&self) -> bool {
        self.0.ConfigNavSwapGamepadButtons
    }

    /// Set whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn set_config_nav_swap_gamepad_buttons(&mut self, enabled: bool) {
        self.0.ConfigNavSwapGamepadButtons = enabled;
    }

    /// Returns whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn config_nav_move_set_mouse_pos(&self) -> bool {
        self.0.ConfigNavMoveSetMousePos
    }

    /// Set whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn set_config_nav_move_set_mouse_pos(&mut self, enabled: bool) {
        self.0.ConfigNavMoveSetMousePos = enabled;
    }

    /// Returns whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn config_nav_capture_keyboard(&self) -> bool {
        self.0.ConfigNavCaptureKeyboard
    }

    /// Set whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn set_config_nav_capture_keyboard(&mut self, enabled: bool) {
        self.0.ConfigNavCaptureKeyboard = enabled;
    }

    /// Returns whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn config_nav_escape_clear_focus_item(&self) -> bool {
        self.0.ConfigNavEscapeClearFocusItem
    }

    /// Set whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn set_config_nav_escape_clear_focus_item(&mut self, enabled: bool) {
        self.0.ConfigNavEscapeClearFocusItem = enabled;
    }

    /// Returns whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn config_nav_escape_clear_focus_window(&self) -> bool {
        self.0.ConfigNavEscapeClearFocusWindow
    }

    /// Set whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn set_config_nav_escape_clear_focus_window(&mut self, enabled: bool) {
        self.0.ConfigNavEscapeClearFocusWindow = enabled;
    }

    /// Returns whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn config_nav_cursor_visible_auto(&self) -> bool {
        self.0.ConfigNavCursorVisibleAuto
    }

    /// Set whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn set_config_nav_cursor_visible_auto(&mut self, enabled: bool) {
        self.0.ConfigNavCursorVisibleAuto = enabled;
    }

    /// Returns whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn config_nav_cursor_visible_always(&self) -> bool {
        self.0.ConfigNavCursorVisibleAlways
    }

    /// Set whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn set_config_nav_cursor_visible_always(&mut self, enabled: bool) {
        self.0.ConfigNavCursorVisibleAlways = enabled;
    }

    /// Returns whether docking is prevented from splitting nodes.
    #[doc(alias = "ConfigDockingNoSplit")]
    pub fn config_docking_no_split(&self) -> bool {
        self.0.ConfigDockingNoSplit
    }

    /// Set whether docking is prevented from splitting nodes.
    #[doc(alias = "ConfigDockingNoSplit")]
    pub fn set_config_docking_no_split(&mut self, enabled: bool) {
        self.0.ConfigDockingNoSplit = enabled;
    }

    /// Returns whether docking over other windows is disabled.
    #[doc(alias = "ConfigDockingNoDockingOver")]
    pub fn config_docking_no_docking_over(&self) -> bool {
        self.0.ConfigDockingNoDockingOver
    }

    /// Set whether docking over other windows is disabled.
    #[doc(alias = "ConfigDockingNoDockingOver")]
    pub fn set_config_docking_no_docking_over(&mut self, enabled: bool) {
        self.0.ConfigDockingNoDockingOver = enabled;
    }

    /// Returns whether docking requires holding Shift.
    #[doc(alias = "ConfigDockingWithShift")]
    pub fn config_docking_with_shift(&self) -> bool {
        self.0.ConfigDockingWithShift
    }

    /// Set whether docking requires holding Shift.
    #[doc(alias = "ConfigDockingWithShift")]
    pub fn set_config_docking_with_shift(&mut self, enabled: bool) {
        self.0.ConfigDockingWithShift = enabled;
    }

    /// Returns whether docking uses a tab bar when possible.
    #[doc(alias = "ConfigDockingAlwaysTabBar")]
    pub fn config_docking_always_tab_bar(&self) -> bool {
        self.0.ConfigDockingAlwaysTabBar
    }

    /// Set whether docking uses a tab bar when possible.
    #[doc(alias = "ConfigDockingAlwaysTabBar")]
    pub fn set_config_docking_always_tab_bar(&mut self, enabled: bool) {
        self.0.ConfigDockingAlwaysTabBar = enabled;
    }

    /// Returns whether docking payloads are rendered transparently.
    #[doc(alias = "ConfigDockingTransparentPayload")]
    pub fn config_docking_transparent_payload(&self) -> bool {
        self.0.ConfigDockingTransparentPayload
    }

    /// Set whether docking payloads are rendered transparently.
    #[doc(alias = "ConfigDockingTransparentPayload")]
    pub fn set_config_docking_transparent_payload(&mut self, enabled: bool) {
        self.0.ConfigDockingTransparentPayload = enabled;
    }

    /// Returns whether viewports should avoid auto-merging.
    #[doc(alias = "ConfigViewportsNoAutoMerge")]
    pub fn config_viewports_no_auto_merge(&self) -> bool {
        self.0.ConfigViewportsNoAutoMerge
    }

    /// Set whether viewports should avoid auto-merging.
    #[doc(alias = "ConfigViewportsNoAutoMerge")]
    pub fn set_config_viewports_no_auto_merge(&mut self, enabled: bool) {
        self.0.ConfigViewportsNoAutoMerge = enabled;
    }

    /// Returns whether viewports should avoid task bar icons.
    #[doc(alias = "ConfigViewportsNoTaskBarIcon")]
    pub fn config_viewports_no_task_bar_icon(&self) -> bool {
        self.0.ConfigViewportsNoTaskBarIcon
    }

    /// Set whether viewports should avoid task bar icons.
    #[doc(alias = "ConfigViewportsNoTaskBarIcon")]
    pub fn set_config_viewports_no_task_bar_icon(&mut self, enabled: bool) {
        self.0.ConfigViewportsNoTaskBarIcon = enabled;
    }

    /// Returns whether viewports should avoid platform window decorations.
    #[doc(alias = "ConfigViewportsNoDecoration")]
    pub fn config_viewports_no_decoration(&self) -> bool {
        self.0.ConfigViewportsNoDecoration
    }

    /// Set whether viewports should avoid platform window decorations.
    #[doc(alias = "ConfigViewportsNoDecoration")]
    pub fn set_config_viewports_no_decoration(&mut self, enabled: bool) {
        self.0.ConfigViewportsNoDecoration = enabled;
    }

    /// Returns whether viewports should not have a default parent.
    #[doc(alias = "ConfigViewportsNoDefaultParent")]
    pub fn config_viewports_no_default_parent(&self) -> bool {
        self.0.ConfigViewportsNoDefaultParent
    }

    /// Set whether viewports should not have a default parent.
    #[doc(alias = "ConfigViewportsNoDefaultParent")]
    pub fn set_config_viewports_no_default_parent(&mut self, enabled: bool) {
        self.0.ConfigViewportsNoDefaultParent = enabled;
    }

    /// Returns whether platform focus also sets ImGui focus in viewports.
    #[doc(alias = "ConfigViewportsPlatformFocusSetsImGuiFocus")]
    pub fn config_viewports_platform_focus_sets_imgui_focus(&self) -> bool {
        self.0.ConfigViewportsPlatformFocusSetsImGuiFocus
    }

    /// Set whether platform focus also sets ImGui focus in viewports.
    #[doc(alias = "ConfigViewportsPlatformFocusSetsImGuiFocus")]
    pub fn set_config_viewports_platform_focus_sets_imgui_focus(&mut self, enabled: bool) {
        self.0.ConfigViewportsPlatformFocusSetsImGuiFocus = enabled;
    }

    /// Returns whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn config_dpi_scale_fonts(&self) -> bool {
        self.0.ConfigDpiScaleFonts
    }

    /// Set whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn set_config_dpi_scale_fonts(&mut self, enabled: bool) {
        self.0.ConfigDpiScaleFonts = enabled;
    }

    /// Returns whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn config_dpi_scale_viewports(&self) -> bool {
        self.0.ConfigDpiScaleViewports
    }

    /// Set whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn set_config_dpi_scale_viewports(&mut self, enabled: bool) {
        self.0.ConfigDpiScaleViewports = enabled;
    }

    /// Returns whether to use MacOSX-style behaviors.
    #[doc(alias = "ConfigMacOSXBehaviors")]
    pub fn config_macosx_behaviors(&self) -> bool {
        self.0.ConfigMacOSXBehaviors
    }

    /// Set whether to use MacOSX-style behaviors.
    #[doc(alias = "ConfigMacOSXBehaviors")]
    pub fn set_config_macosx_behaviors(&mut self, enabled: bool) {
        self.0.ConfigMacOSXBehaviors = enabled;
    }

    /// Returns whether to trickle input events through the queue.
    #[doc(alias = "ConfigInputTrickleEventQueue")]
    pub fn config_input_trickle_event_queue(&self) -> bool {
        self.0.ConfigInputTrickleEventQueue
    }

    /// Set whether to trickle input events through the queue.
    #[doc(alias = "ConfigInputTrickleEventQueue")]
    pub fn set_config_input_trickle_event_queue(&mut self, enabled: bool) {
        self.0.ConfigInputTrickleEventQueue = enabled;
    }

    /// Returns whether the input text cursor blinks.
    #[doc(alias = "ConfigInputTextCursorBlink")]
    pub fn config_input_text_cursor_blink(&self) -> bool {
        self.0.ConfigInputTextCursorBlink
    }

    /// Set whether the input text cursor blinks.
    #[doc(alias = "ConfigInputTextCursorBlink")]
    pub fn set_config_input_text_cursor_blink(&mut self, enabled: bool) {
        self.0.ConfigInputTextCursorBlink = enabled;
    }

    /// Returns whether Enter keeps the input text active.
    #[doc(alias = "ConfigInputTextEnterKeepActive")]
    pub fn config_input_text_enter_keep_active(&self) -> bool {
        self.0.ConfigInputTextEnterKeepActive
    }

    /// Set whether Enter keeps the input text active.
    #[doc(alias = "ConfigInputTextEnterKeepActive")]
    pub fn set_config_input_text_enter_keep_active(&mut self, enabled: bool) {
        self.0.ConfigInputTextEnterKeepActive = enabled;
    }

    /// Returns whether click-drag on numeric widgets turns into text input.
    #[doc(alias = "ConfigDragClickToInputText")]
    pub fn config_drag_click_to_input_text(&self) -> bool {
        self.0.ConfigDragClickToInputText
    }

    /// Set whether click-drag on numeric widgets turns into text input.
    #[doc(alias = "ConfigDragClickToInputText")]
    pub fn set_config_drag_click_to_input_text(&mut self, enabled: bool) {
        self.0.ConfigDragClickToInputText = enabled;
    }

    /// Returns whether windows can be moved only from their title bar.
    ///
    /// When enabled, click-dragging on empty window content will no longer move the window.
    /// This can be useful in multi-viewport setups to avoid accidental platform window moves
    /// while interacting with in-window widgets (e.g. gizmos in a scene view).
    #[doc(alias = "ConfigWindowsMoveFromTitleBarOnly")]
    pub fn config_windows_move_from_title_bar_only(&self) -> bool {
        self.0.ConfigWindowsMoveFromTitleBarOnly
    }

    /// Set whether windows can be moved only from their title bar.
    ///
    /// Note: This is typically latched by Dear ImGui at the start of the frame. Prefer setting it
    /// during initialization or before calling `Context::frame()`.
    #[doc(alias = "ConfigWindowsMoveFromTitleBarOnly")]
    pub fn set_config_windows_move_from_title_bar_only(&mut self, enabled: bool) {
        self.0.ConfigWindowsMoveFromTitleBarOnly = enabled;
    }

    /// Returns whether windows can be resized from their edges.
    #[doc(alias = "ConfigWindowsResizeFromEdges")]
    pub fn config_windows_resize_from_edges(&self) -> bool {
        self.0.ConfigWindowsResizeFromEdges
    }

    /// Set whether windows can be resized from their edges.
    #[doc(alias = "ConfigWindowsResizeFromEdges")]
    pub fn set_config_windows_resize_from_edges(&mut self, enabled: bool) {
        self.0.ConfigWindowsResizeFromEdges = enabled;
    }

    /// Returns whether Ctrl+C copies window contents.
    #[doc(alias = "ConfigWindowsCopyContentsWithCtrlC")]
    pub fn config_windows_copy_contents_with_ctrl_c(&self) -> bool {
        self.0.ConfigWindowsCopyContentsWithCtrlC
    }

    /// Set whether Ctrl+C copies window contents.
    #[doc(alias = "ConfigWindowsCopyContentsWithCtrlC")]
    pub fn set_config_windows_copy_contents_with_ctrl_c(&mut self, enabled: bool) {
        self.0.ConfigWindowsCopyContentsWithCtrlC = enabled;
    }

    /// Returns whether scrollbars scroll by page.
    #[doc(alias = "ConfigScrollbarScrollByPage")]
    pub fn config_scrollbar_scroll_by_page(&self) -> bool {
        self.0.ConfigScrollbarScrollByPage
    }

    /// Set whether scrollbars scroll by page.
    #[doc(alias = "ConfigScrollbarScrollByPage")]
    pub fn set_config_scrollbar_scroll_by_page(&mut self, enabled: bool) {
        self.0.ConfigScrollbarScrollByPage = enabled;
    }

    /// Returns the memory compact timer (seconds).
    #[doc(alias = "ConfigMemoryCompactTimer")]
    pub fn config_memory_compact_timer(&self) -> f32 {
        self.0.ConfigMemoryCompactTimer
    }

    /// Set the memory compact timer (seconds).
    #[doc(alias = "ConfigMemoryCompactTimer")]
    pub fn set_config_memory_compact_timer(&mut self, seconds: f32) {
        self.0.ConfigMemoryCompactTimer = seconds;
    }

    /// Returns the time for a double-click (seconds).
    #[doc(alias = "MouseDoubleClickTime")]
    pub fn mouse_double_click_time(&self) -> f32 {
        self.0.MouseDoubleClickTime
    }

    /// Set the time for a double-click (seconds).
    #[doc(alias = "MouseDoubleClickTime")]
    pub fn set_mouse_double_click_time(&mut self, seconds: f32) {
        self.0.MouseDoubleClickTime = seconds;
    }

    /// Returns the maximum distance to qualify as a double-click (pixels).
    #[doc(alias = "MouseDoubleClickMaxDist")]
    pub fn mouse_double_click_max_dist(&self) -> f32 {
        self.0.MouseDoubleClickMaxDist
    }

    /// Set the maximum distance to qualify as a double-click (pixels).
    #[doc(alias = "MouseDoubleClickMaxDist")]
    pub fn set_mouse_double_click_max_dist(&mut self, pixels: f32) {
        self.0.MouseDoubleClickMaxDist = pixels;
    }

    /// Returns the distance threshold for starting a drag (pixels).
    #[doc(alias = "MouseDragThreshold")]
    pub fn mouse_drag_threshold(&self) -> f32 {
        self.0.MouseDragThreshold
    }

    /// Set the distance threshold for starting a drag (pixels).
    #[doc(alias = "MouseDragThreshold")]
    pub fn set_mouse_drag_threshold(&mut self, pixels: f32) {
        self.0.MouseDragThreshold = pixels;
    }

    /// Returns the key repeat delay (seconds).
    #[doc(alias = "KeyRepeatDelay")]
    pub fn key_repeat_delay(&self) -> f32 {
        self.0.KeyRepeatDelay
    }

    /// Set the key repeat delay (seconds).
    #[doc(alias = "KeyRepeatDelay")]
    pub fn set_key_repeat_delay(&mut self, seconds: f32) {
        self.0.KeyRepeatDelay = seconds;
    }

    /// Returns the key repeat rate (seconds).
    #[doc(alias = "KeyRepeatRate")]
    pub fn key_repeat_rate(&self) -> f32 {
        self.0.KeyRepeatRate
    }

    /// Set the key repeat rate (seconds).
    #[doc(alias = "KeyRepeatRate")]
    pub fn set_key_repeat_rate(&mut self, seconds: f32) {
        self.0.KeyRepeatRate = seconds;
    }

    /// Returns whether error recovery is enabled.
    #[doc(alias = "ConfigErrorRecovery")]
    pub fn config_error_recovery(&self) -> bool {
        self.0.ConfigErrorRecovery
    }

    /// Set whether error recovery is enabled.
    #[doc(alias = "ConfigErrorRecovery")]
    pub fn set_config_error_recovery(&mut self, enabled: bool) {
        self.0.ConfigErrorRecovery = enabled;
    }

    /// Returns whether error recovery enables asserts.
    #[doc(alias = "ConfigErrorRecoveryEnableAssert")]
    pub fn config_error_recovery_enable_assert(&self) -> bool {
        self.0.ConfigErrorRecoveryEnableAssert
    }

    /// Set whether error recovery enables asserts.
    #[doc(alias = "ConfigErrorRecoveryEnableAssert")]
    pub fn set_config_error_recovery_enable_assert(&mut self, enabled: bool) {
        self.0.ConfigErrorRecoveryEnableAssert = enabled;
    }

    /// Returns whether error recovery enables debug logs.
    #[doc(alias = "ConfigErrorRecoveryEnableDebugLog")]
    pub fn config_error_recovery_enable_debug_log(&self) -> bool {
        self.0.ConfigErrorRecoveryEnableDebugLog
    }

    /// Set whether error recovery enables debug logs.
    #[doc(alias = "ConfigErrorRecoveryEnableDebugLog")]
    pub fn set_config_error_recovery_enable_debug_log(&mut self, enabled: bool) {
        self.0.ConfigErrorRecoveryEnableDebugLog = enabled;
    }

    /// Returns whether error recovery enables tooltips.
    #[doc(alias = "ConfigErrorRecoveryEnableTooltip")]
    pub fn config_error_recovery_enable_tooltip(&self) -> bool {
        self.0.ConfigErrorRecoveryEnableTooltip
    }

    /// Set whether error recovery enables tooltips.
    #[doc(alias = "ConfigErrorRecoveryEnableTooltip")]
    pub fn set_config_error_recovery_enable_tooltip(&mut self, enabled: bool) {
        self.0.ConfigErrorRecoveryEnableTooltip = enabled;
    }

    /// Returns whether Dear ImGui thinks a debugger is present.
    #[doc(alias = "ConfigDebugIsDebuggerPresent")]
    pub fn config_debug_is_debugger_present(&self) -> bool {
        self.0.ConfigDebugIsDebuggerPresent
    }

    /// Set whether Dear ImGui thinks a debugger is present.
    #[doc(alias = "ConfigDebugIsDebuggerPresent")]
    pub fn set_config_debug_is_debugger_present(&mut self, enabled: bool) {
        self.0.ConfigDebugIsDebuggerPresent = enabled;
    }

    /// Returns whether to highlight ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflicts")]
    pub fn config_debug_highlight_id_conflicts(&self) -> bool {
        self.0.ConfigDebugHighlightIdConflicts
    }

    /// Set whether to highlight ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflicts")]
    pub fn set_config_debug_highlight_id_conflicts(&mut self, enabled: bool) {
        self.0.ConfigDebugHighlightIdConflicts = enabled;
    }

    /// Returns whether to show the item picker when highlighting ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflictsShowItemPicker")]
    pub fn config_debug_highlight_id_conflicts_show_item_picker(&self) -> bool {
        self.0.ConfigDebugHighlightIdConflictsShowItemPicker
    }

    /// Set whether to show the item picker when highlighting ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflictsShowItemPicker")]
    pub fn set_config_debug_highlight_id_conflicts_show_item_picker(&mut self, enabled: bool) {
        self.0.ConfigDebugHighlightIdConflictsShowItemPicker = enabled;
    }

    /// Returns whether `Begin()` returns `true` once.
    #[doc(alias = "ConfigDebugBeginReturnValueOnce")]
    pub fn config_debug_begin_return_value_once(&self) -> bool {
        self.0.ConfigDebugBeginReturnValueOnce
    }

    /// Set whether `Begin()` returns `true` once.
    #[doc(alias = "ConfigDebugBeginReturnValueOnce")]
    pub fn set_config_debug_begin_return_value_once(&mut self, enabled: bool) {
        self.0.ConfigDebugBeginReturnValueOnce = enabled;
    }

    /// Returns whether `Begin()` returns `true` in a loop.
    #[doc(alias = "ConfigDebugBeginReturnValueLoop")]
    pub fn config_debug_begin_return_value_loop(&self) -> bool {
        self.0.ConfigDebugBeginReturnValueLoop
    }

    /// Set whether `Begin()` returns `true` in a loop.
    #[doc(alias = "ConfigDebugBeginReturnValueLoop")]
    pub fn set_config_debug_begin_return_value_loop(&mut self, enabled: bool) {
        self.0.ConfigDebugBeginReturnValueLoop = enabled;
    }

    /// Returns whether to ignore focus loss.
    #[doc(alias = "ConfigDebugIgnoreFocusLoss")]
    pub fn config_debug_ignore_focus_loss(&self) -> bool {
        self.0.ConfigDebugIgnoreFocusLoss
    }

    /// Set whether to ignore focus loss.
    #[doc(alias = "ConfigDebugIgnoreFocusLoss")]
    pub fn set_config_debug_ignore_focus_loss(&mut self, enabled: bool) {
        self.0.ConfigDebugIgnoreFocusLoss = enabled;
    }

    /// Returns whether to display ini settings debug tools.
    #[doc(alias = "ConfigDebugIniSettings")]
    pub fn config_debug_ini_settings(&self) -> bool {
        self.0.ConfigDebugIniSettings
    }

    /// Set whether to display ini settings debug tools.
    #[doc(alias = "ConfigDebugIniSettings")]
    pub fn set_config_debug_ini_settings(&mut self, enabled: bool) {
        self.0.ConfigDebugIniSettings = enabled;
    }

    /// Returns the backend platform name, if set.
    ///
    /// Note: to set this safely, use `Context::set_platform_name()`.
    #[doc(alias = "BackendPlatformName")]
    pub fn backend_platform_name(&self) -> Option<&CStr> {
        let ptr = self.0.BackendPlatformName;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns the backend renderer name, if set.
    ///
    /// Note: to set this safely, use `Context::set_renderer_name()`.
    #[doc(alias = "BackendRendererName")]
    pub fn backend_renderer_name(&self) -> Option<&CStr> {
        let ptr = self.0.BackendRendererName;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns the backend platform user data pointer.
    #[doc(alias = "BackendPlatformUserData")]
    pub fn backend_platform_user_data(&self) -> *mut c_void {
        self.0.BackendPlatformUserData
    }

    /// Set the backend platform user data pointer.
    #[doc(alias = "BackendPlatformUserData")]
    pub fn set_backend_platform_user_data(&mut self, user_data: *mut c_void) {
        self.0.BackendPlatformUserData = user_data;
    }

    /// Returns the backend renderer user data pointer.
    #[doc(alias = "BackendRendererUserData")]
    pub fn backend_renderer_user_data(&self) -> *mut c_void {
        self.0.BackendRendererUserData
    }

    /// Set the backend renderer user data pointer.
    #[doc(alias = "BackendRendererUserData")]
    pub fn set_backend_renderer_user_data(&mut self, user_data: *mut c_void) {
        self.0.BackendRendererUserData = user_data;
    }

    /// Returns the backend language user data pointer.
    #[doc(alias = "BackendLanguageUserData")]
    pub fn backend_language_user_data(&self) -> *mut c_void {
        self.0.BackendLanguageUserData
    }

    /// Set the backend language user data pointer.
    #[doc(alias = "BackendLanguageUserData")]
    pub fn set_backend_language_user_data(&mut self, user_data: *mut c_void) {
        self.0.BackendLanguageUserData = user_data;
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
            sys::ImGuiIO_AddKeyEvent(&mut self.0 as *mut _, key.into(), down);
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
            sys::ImGuiIO_AddMouseButtonEvent(&mut self.0 as *mut _, button.into(), down);
        }
    }

    /// Add a mouse wheel event to the input queue
    pub fn add_mouse_wheel_event(&mut self, wheel: [f32; 2]) {
        unsafe {
            sys::ImGuiIO_AddMouseWheelEvent(&mut self.0 as *mut _, wheel[0], wheel[1]);
        }
    }

    /// Add a mouse source event to the input queue.
    ///
    /// When the input source switches between mouse / touch screen / pen,
    /// backends should call this before submitting other mouse events for
    /// the frame.
    pub fn add_mouse_source_event(&mut self, source: crate::input::MouseSource) {
        unsafe {
            sys::ImGuiIO_AddMouseSourceEvent(&mut self.0 as *mut _, source.into());
        }
    }

    /// Queue the hovered viewport id for the current frame.
    ///
    /// When multi-viewport is enabled and the backend can reliably obtain
    /// the ImGui viewport hovered by the OS mouse, it should set
    /// `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT` and call this once per
    /// frame.
    pub fn add_mouse_viewport_event(&mut self, viewport_id: crate::Id) {
        unsafe {
            sys::ImGuiIO_AddMouseViewportEvent(&mut self.0 as *mut _, viewport_id.raw());
        }
    }

    /// Notify Dear ImGui that the application window gained or lost focus
    /// This mirrors `io.AddFocusEvent()` in Dear ImGui and is used by platform backends.
    pub fn add_focus_event(&mut self, focused: bool) {
        unsafe {
            sys::ImGuiIO_AddFocusEvent(&mut self.0 as *mut _, focused);
        }
    }

    /// Get the global font scale (not available in current Dear ImGui version)
    /// Compatibility shim: maps to style.FontScaleMain (Dear ImGui 1.92+)
    pub fn font_global_scale(&self) -> f32 {
        unsafe { (*sys::igGetStyle()).FontScaleMain }
    }

    /// Set the global font scale (not available in current Dear ImGui version)
    /// Compatibility shim: maps to style.FontScaleMain (Dear ImGui 1.92+)
    pub fn set_font_global_scale(&mut self, _scale: f32) {
        unsafe {
            (*sys::igGetStyle()).FontScaleMain = _scale;
        }
    }

    /// Get the display framebuffer scale
    pub fn display_framebuffer_scale(&self) -> [f32; 2] {
        let scale = self.0.DisplayFramebufferScale;
        [scale.x, scale.y]
    }

    /// Returns the mouse delta from the previous frame, in pixels.
    #[doc(alias = "MouseDelta")]
    pub fn mouse_delta(&self) -> [f32; 2] {
        let delta = self.0.MouseDelta;
        [delta.x, delta.y]
    }

    /// Returns whether Ctrl modifier is held.
    #[doc(alias = "KeyCtrl")]
    pub fn key_ctrl(&self) -> bool {
        self.0.KeyCtrl
    }

    /// Returns whether Shift modifier is held.
    #[doc(alias = "KeyShift")]
    pub fn key_shift(&self) -> bool {
        self.0.KeyShift
    }

    /// Returns whether Alt modifier is held.
    #[doc(alias = "KeyAlt")]
    pub fn key_alt(&self) -> bool {
        self.0.KeyAlt
    }

    /// Returns whether Super/Command modifier is held.
    #[doc(alias = "KeySuper")]
    pub fn key_super(&self) -> bool {
        self.0.KeySuper
    }

    /// Returns the current mouse input source.
    #[doc(alias = "MouseSource")]
    pub fn mouse_source(&self) -> crate::input::MouseSource {
        match self.0.MouseSource {
            sys::ImGuiMouseSource_Mouse => crate::input::MouseSource::Mouse,
            sys::ImGuiMouseSource_TouchScreen => crate::input::MouseSource::TouchScreen,
            sys::ImGuiMouseSource_Pen => crate::input::MouseSource::Pen,
            _ => crate::input::MouseSource::Mouse,
        }
    }

    /// Returns the viewport id hovered by the OS mouse (if supported by the backend).
    #[doc(alias = "MouseHoveredViewport")]
    pub fn mouse_hovered_viewport(&self) -> crate::Id {
        crate::Id::from(self.0.MouseHoveredViewport)
    }

    /// Returns whether Ctrl+LeftClick should be treated as RightClick.
    #[doc(alias = "MouseCtrlLeftAsRightClick")]
    pub fn mouse_ctrl_left_as_right_click(&self) -> bool {
        self.0.MouseCtrlLeftAsRightClick
    }

    /// Set whether Ctrl+LeftClick should be treated as RightClick.
    #[doc(alias = "MouseCtrlLeftAsRightClick")]
    pub fn set_mouse_ctrl_left_as_right_click(&mut self, enabled: bool) {
        self.0.MouseCtrlLeftAsRightClick = enabled;
    }

    /// Set the display framebuffer scale
    /// This is important for HiDPI displays to ensure proper rendering
    pub fn set_display_framebuffer_scale(&mut self, scale: [f32; 2]) {
        self.0.DisplayFramebufferScale.x = scale[0];
        self.0.DisplayFramebufferScale.y = scale[1];
    }
}
