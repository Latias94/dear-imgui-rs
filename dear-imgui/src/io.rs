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
//! // Claim managed atlas ownership before advertising managed textures.
//! let _consumer = ctx.create_synchronous_renderer_consumer().unwrap();
//! // Configure IO before starting the first frame.
//! let io = ctx.io_mut();
//! io.set_config_flags(io.config_flags() | ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
//! io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
//! ```
//!
//! Font scaling is style state rather than IO compatibility state:
//! ```compile_fail
//! # let mut ctx = dear_imgui_rs::Context::create();
//! ctx.io_mut().set_font_global_scale(1.25);
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use std::cell::UnsafeCell;
use std::ffi::{CStr, c_char, c_void};

use bitflags::bitflags;

use crate::validation::{assert_finite_f32, assert_finite_vec2};
use crate::{ColorEditOptions, sys};

mod input;
#[cfg(test)]
mod tests;

#[cfg(feature = "serde")]
crate::validation::impl_serde_i32_flags!(
    ConfigFlags,
    BackendFlags,
    ViewportFlags,
    WindowClassViewportFlags,
);

bitflags! {
    /// Configuration flags
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ConfigFlags: i32 {
        /// Master keyboard navigation enable flag.
        const NAV_ENABLE_KEYBOARD = sys::ImGuiConfigFlags_NavEnableKeyboard;
        /// Master gamepad navigation enable flag.
        const NAV_ENABLE_GAMEPAD = sys::ImGuiConfigFlags_NavEnableGamepad;
        /// Instruction imgui-rs to clear mouse position/buttons in `frame()`.
        const NO_MOUSE = sys::ImGuiConfigFlags_NoMouse;
        /// Instruction backend to not alter mouse cursor shape and visibility.
        const NO_MOUSE_CURSOR_CHANGE = sys::ImGuiConfigFlags_NoMouseCursorChange;
        /// Disable keyboard inputs and interactions.
        const NO_KEYBOARD = sys::ImGuiConfigFlags_NoKeyboard;
        /// Application is SRGB-aware.
        const IS_SRGB = sys::ImGuiConfigFlags_IsSRGB;
        /// Application is using a touch screen instead of a mouse.
        const IS_TOUCH_SCREEN = sys::ImGuiConfigFlags_IsTouchScreen;

        const DOCKING_ENABLE = sys::ImGuiConfigFlags_DockingEnable;

        const VIEWPORTS_ENABLE = sys::ImGuiConfigFlags_ViewportsEnable;
    }
}

bitflags! {
    /// Viewport policy bits that an application may override through [`crate::WindowClass`].
    ///
    /// Identity, topology, host capability, and platform output status remain owned by Dear
    /// ImGui and its backends, so they are intentionally absent from this type.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowClassViewportFlags: i32 {
        /// Disable platform decorations such as title bars and borders.
        const NO_DECORATION = sys::ImGuiViewportFlags_NoDecoration;
        /// Omit the platform task bar icon.
        const NO_TASK_BAR_ICON = sys::ImGuiViewportFlags_NoTaskBarIcon;
        /// Do not activate the platform window when it appears.
        const NO_FOCUS_ON_APPEARING = sys::ImGuiViewportFlags_NoFocusOnAppearing;
        /// Do not activate the platform window when it is clicked.
        const NO_FOCUS_ON_CLICK = sys::ImGuiViewportFlags_NoFocusOnClick;
        /// Make pointer input pass through the platform window.
        const NO_INPUTS = sys::ImGuiViewportFlags_NoInputs;
        /// Tell the renderer that the viewport framebuffer does not need to be cleared.
        const NO_RENDERER_CLEAR = sys::ImGuiViewportFlags_NoRendererClear;
        /// Prevent this viewport from automatically merging into another host viewport.
        const NO_AUTO_MERGE = sys::ImGuiViewportFlags_NoAutoMerge;
        /// Keep the platform window above ordinary windows.
        const TOP_MOST = sys::ImGuiViewportFlags_TopMost;
    }
}

bitflags! {
    /// Backend capabilities
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BackendFlags: i32 {
        /// Backend supports gamepad and currently has one connected
        const HAS_GAMEPAD = sys::ImGuiBackendFlags_HasGamepad;
        /// Backend supports honoring `get_mouse_cursor` value to change the OS cursor shape
        const HAS_MOUSE_CURSORS = sys::ImGuiBackendFlags_HasMouseCursors;
        /// Backend supports `io.want_set_mouse_pos` requests to reposition the OS mouse position.
        const HAS_SET_MOUSE_POS = sys::ImGuiBackendFlags_HasSetMousePos;
        /// Backend can report which viewport the OS mouse is hovering via `add_mouse_viewport_event`
        const HAS_MOUSE_HOVERED_VIEWPORT =
            sys::ImGuiBackendFlags_HasMouseHoveredViewport;
        /// Backend platform can honor viewport parent/child relationships.
        const HAS_PARENT_VIEWPORT = sys::ImGuiBackendFlags_HasParentViewport;
        /// Backend renderer supports DrawCmd::vtx_offset.
        const RENDERER_HAS_VTX_OFFSET = sys::ImGuiBackendFlags_RendererHasVtxOffset;
        /// Backend renderer supports ImTextureData requests to create/update/destroy textures.
        const RENDERER_HAS_TEXTURES = sys::ImGuiBackendFlags_RendererHasTextures;

        #[cfg(feature = "multi-viewport")]
        /// Set if the platform backend supports viewports.
        const PLATFORM_HAS_VIEWPORTS = sys::ImGuiBackendFlags_PlatformHasViewports;
        #[cfg(feature = "multi-viewport")]
        /// Set if the renderer backend supports viewports.
        const RENDERER_HAS_VIEWPORTS = sys::ImGuiBackendFlags_RendererHasViewports;
    }
}

bitflags! {
    /// Viewport flags for multi-viewport support
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ViewportFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Represent a Platform Window
        const IS_PLATFORM_WINDOW = sys::ImGuiViewportFlags_IsPlatformWindow;
        /// Represent a Platform Monitor (unused in our implementation)
        const IS_PLATFORM_MONITOR = sys::ImGuiViewportFlags_IsPlatformMonitor;
        /// Platform Window: is created/managed by the application (rather than a dear imgui backend)
        const OWNED_BY_APP = sys::ImGuiViewportFlags_OwnedByApp;
        /// Platform Window: Disable platform decorations: title bar, borders, etc.
        const NO_DECORATION = sys::ImGuiViewportFlags_NoDecoration;
        /// Platform Window: Disable platform task bar icon (generally set on popups/tooltips, or all windows if ImGuiConfigFlags_ViewportsNoTaskBarIcon is set)
        const NO_TASK_BAR_ICON = sys::ImGuiViewportFlags_NoTaskBarIcon;
        /// Platform Window: Don't take focus when created.
        const NO_FOCUS_ON_APPEARING = sys::ImGuiViewportFlags_NoFocusOnAppearing;
        /// Platform Window: Don't take focus when clicked on.
        const NO_FOCUS_ON_CLICK = sys::ImGuiViewportFlags_NoFocusOnClick;
        /// Platform Window: Make mouse pass through so we can drag this window while peaking behind it.
        const NO_INPUTS = sys::ImGuiViewportFlags_NoInputs;
        /// Platform Window: Renderer doesn't need to clear the framebuffer ahead (because we will fill it entirely).
        const NO_RENDERER_CLEAR = sys::ImGuiViewportFlags_NoRendererClear;
        /// Platform Window: Avoid merging this window into another host window. This can only be set via ImGuiWindowClass viewport flags override (because we need to now ahead if we are going to create a viewport in the first place!).
        const NO_AUTO_MERGE = sys::ImGuiViewportFlags_NoAutoMerge;
        /// Platform Window: Display on top (for tooltips only).
        const TOP_MOST = sys::ImGuiViewportFlags_TopMost;
        /// Viewport can host multiple imgui windows (secondary viewports are associated to a single window).
        const CAN_HOST_OTHER_WINDOWS = sys::ImGuiViewportFlags_CanHostOtherWindows;
        /// Platform Window: Window is minimized, can skip render. When minimized we tend to avoid using the viewport pos/size for clipping rectangle computation.
        const IS_MINIMIZED = sys::ImGuiViewportFlags_IsMinimized;
        /// Platform Window: Window is focused (last call to Platform_GetWindowFocus() returned true)
        const IS_FOCUSED = sys::ImGuiViewportFlags_IsFocused;
    }
}

fn validate_config_flags(caller: &str, flags: ConfigFlags) {
    let unsupported = flags.bits() & !ConfigFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiConfigFlags bits: 0x{unsupported:X}"
    );
}

fn validate_backend_flags(caller: &str, flags: BackendFlags) {
    let unsupported = flags.bits() & !BackendFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiBackendFlags bits: 0x{unsupported:X}"
    );
}

/// Settings and inputs/outputs for imgui-rs
/// This is a transparent wrapper around ImGuiIO
#[repr(transparent)]
pub struct Io(UnsafeCell<sys::ImGuiIO>);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiIO>()] = [(); std::mem::size_of::<Io>()];
const _: [(); std::mem::align_of::<sys::ImGuiIO>()] = [(); std::mem::align_of::<Io>()];

impl Io {
    #[inline]
    pub(crate) fn inner(&self) -> &sys::ImGuiIO {
        // Safety: `Io` is a transparent wrapper around the sys `ImGuiIO` value which is owned by
        // Dear ImGui. The value may be mutated by Dear ImGui even while Rust holds `&Io`, so we
        // store it behind `UnsafeCell` to make that interior mutability explicit.
        unsafe { &*self.0.get() }
    }

    #[inline]
    pub(crate) fn inner_mut(&mut self) -> &mut sys::ImGuiIO {
        // Safety: caller has `&mut Io`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.0.get() }
    }

    fn frame_count(&self) -> i32 {
        let ctx = self.inner().Ctx;
        if ctx.is_null() {
            0
        } else {
            unsafe { (*ctx).FrameCount }
        }
    }

    pub(crate) fn assert_delta_time(&self, caller: &str, delta_time: f32) {
        assert_finite_f32(caller, "delta_time", delta_time);
        if self.frame_count() == 0 {
            assert!(
                delta_time >= 0.0,
                "{caller} delta_time must be non-negative before the first frame"
            );
        } else {
            assert!(
                delta_time > 0.0,
                "{caller} delta_time must be positive after the first frame"
            );
        }
    }

    // A non-null field must point to a valid C string for the lifetime of this IO borrow.
    unsafe fn context_c_str(&self, ptr: *const c_char) -> Option<&CStr> {
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Main display size in pixels
    pub fn display_size(&self) -> [f32; 2] {
        [self.inner().DisplaySize.x, self.inner().DisplaySize.y]
    }

    /// Set main display size in pixels
    pub fn set_display_size(&mut self, size: [f32; 2]) {
        assert_display_size("Io::set_display_size()", size);
        self.inner_mut().DisplaySize.x = size[0];
        self.inner_mut().DisplaySize.y = size[1];
    }

    /// Get the display framebuffer scale
    pub fn display_framebuffer_scale(&self) -> [f32; 2] {
        let scale = self.inner().DisplayFramebufferScale;
        [scale.x, scale.y]
    }

    /// Set the strictly positive, finite display framebuffer scale.
    ///
    /// This controls HiDPI pixel density. Use a zero display size to suspend
    /// rendering for a minimized window; its pixel density remains positive.
    pub fn set_display_framebuffer_scale(&mut self, scale: [f32; 2]) {
        assert_display_framebuffer_scale("Io::set_display_framebuffer_scale()", scale);
        self.inner_mut().DisplayFramebufferScale.x = scale[0];
        self.inner_mut().DisplayFramebufferScale.y = scale[1];
    }

    /// Time elapsed since last frame, in seconds
    pub fn delta_time(&self) -> f32 {
        self.inner().DeltaTime
    }

    /// Set time elapsed since last frame, in seconds
    pub fn set_delta_time(&mut self, delta_time: f32) {
        self.assert_delta_time("Io::set_delta_time()", delta_time);
        self.inner_mut().DeltaTime = delta_time;
    }

    /// Auto-save interval for `.ini` settings, in seconds.
    #[doc(alias = "IniSavingRate")]
    pub fn ini_saving_rate(&self) -> f32 {
        self.inner().IniSavingRate
    }

    /// Set auto-save interval for `.ini` settings, in seconds.
    #[doc(alias = "IniSavingRate")]
    pub fn set_ini_saving_rate(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_ini_saving_rate()", "seconds", seconds);
        self.inner_mut().IniSavingRate = seconds;
    }

    /// Returns the current `.ini` filename, or `None` if disabled.
    ///
    /// Note: to set this safely, use `Context::set_ini_filename()`.
    #[doc(alias = "IniFilename")]
    pub fn ini_filename(&self) -> Option<&CStr> {
        unsafe { self.context_c_str(self.inner().IniFilename) }
    }

    /// Returns the current `.log` filename, or `None` if disabled.
    ///
    /// Note: to set this safely, use `Context::set_log_filename()`.
    #[doc(alias = "LogFilename")]
    pub fn log_filename(&self) -> Option<&CStr> {
        unsafe { self.context_c_str(self.inner().LogFilename) }
    }

    /// Check if imgui wants to save ini settings
    pub fn want_save_ini_settings(&self) -> bool {
        self.inner().WantSaveIniSettings
    }

    /// Returns user data pointer stored in `ImGuiIO`.
    #[doc(alias = "UserData")]
    pub fn user_data(&self) -> *mut c_void {
        self.inner().UserData
    }

    /// Set user data pointer stored in `ImGuiIO`.
    #[doc(alias = "UserData")]
    pub fn set_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().UserData = user_data;
    }

    /// Returns whether font scaling via Ctrl+MouseWheel is enabled.
    #[doc(alias = "FontAllowUserScaling")]
    pub fn font_allow_user_scaling(&self) -> bool {
        self.inner().FontAllowUserScaling
    }

    /// Set whether font scaling via Ctrl+MouseWheel is enabled.
    #[doc(alias = "FontAllowUserScaling")]
    pub fn set_font_allow_user_scaling(&mut self, enabled: bool) {
        self.inner_mut().FontAllowUserScaling = enabled;
    }

    /// Framerate estimation, in frames per second
    pub fn framerate(&self) -> f32 {
        self.inner().Framerate
    }

    /// Vertices output during last call to render
    pub fn metrics_render_vertices(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_render_vertices()",
            self.inner().MetricsRenderVertices,
        )
    }

    /// Indices output during last call to render
    pub fn metrics_render_indices(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_render_indices()",
            self.inner().MetricsRenderIndices,
        )
    }

    /// Number of visible windows
    pub fn metrics_render_windows(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_render_windows()",
            self.inner().MetricsRenderWindows,
        )
    }

    /// Number of active windows
    pub fn metrics_active_windows(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_active_windows()",
            self.inner().MetricsActiveWindows,
        )
    }

    /// Returns the backend platform name, if set.
    ///
    /// Note: to set this safely, use `Context::set_platform_name()`.
    #[doc(alias = "BackendPlatformName")]
    pub fn backend_platform_name(&self) -> Option<&CStr> {
        unsafe { self.context_c_str(self.inner().BackendPlatformName) }
    }

    /// Returns the backend renderer name, if set.
    ///
    /// Note: to set this safely, use `Context::set_renderer_name()`.
    #[doc(alias = "BackendRendererName")]
    pub fn backend_renderer_name(&self) -> Option<&CStr> {
        unsafe { self.context_c_str(self.inner().BackendRendererName) }
    }

    /// Returns the backend platform user data pointer.
    #[doc(alias = "BackendPlatformUserData")]
    pub fn backend_platform_user_data(&self) -> *mut c_void {
        self.inner().BackendPlatformUserData
    }

    /// Set the backend platform user data pointer.
    ///
    /// # Safety
    ///
    /// `user_data` must be null or point to the state expected by every installed platform
    /// callback. That state must remain valid until the callbacks are removed and the pointer is
    /// cleared.
    #[doc(alias = "BackendPlatformUserData")]
    pub unsafe fn set_backend_platform_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().BackendPlatformUserData = user_data;
    }

    /// Returns the backend renderer user data pointer.
    #[doc(alias = "BackendRendererUserData")]
    pub fn backend_renderer_user_data(&self) -> *mut c_void {
        self.inner().BackendRendererUserData
    }

    /// Set the backend renderer user data pointer.
    ///
    /// # Safety
    ///
    /// `user_data` must be null or point to the state expected by every installed renderer
    /// callback. That state must remain valid until the callbacks are removed and the pointer is
    /// cleared.
    #[doc(alias = "BackendRendererUserData")]
    pub unsafe fn set_backend_renderer_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().BackendRendererUserData = user_data;
    }

    /// Returns the backend language user data pointer.
    #[doc(alias = "BackendLanguageUserData")]
    pub fn backend_language_user_data(&self) -> *mut c_void {
        self.inner().BackendLanguageUserData
    }

    /// Set the backend language user data pointer.
    ///
    /// # Safety
    ///
    /// `user_data` must be null or point to state whose layout and lifetime match every language
    /// binding operation that may read it. Clear the pointer before releasing that state.
    #[doc(alias = "BackendLanguageUserData")]
    pub unsafe fn set_backend_language_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().BackendLanguageUserData = user_data;
    }

    /// Backend flags
    pub fn backend_flags(&self) -> BackendFlags {
        BackendFlags::from_bits_retain(self.inner().BackendFlags)
    }

    /// Set backend flags
    pub fn set_backend_flags(&mut self, flags: BackendFlags) {
        validate_backend_flags("Io::set_backend_flags()", flags);
        self.inner_mut().BackendFlags = flags.bits();
    }

    // Configuration

    /// Configuration flags
    pub fn config_flags(&self) -> ConfigFlags {
        ConfigFlags::from_bits_retain(self.inner().ConfigFlags)
    }

    /// Set configuration flags
    pub fn set_config_flags(&mut self, flags: ConfigFlags) {
        validate_config_flags("Io::set_config_flags()", flags);
        self.inner_mut().ConfigFlags = flags.bits();
    }

    // Input and interaction settings

    /// Returns the default options used by color edit and picker widgets.
    #[doc(alias = "ConfigColorEditFlags")]
    pub fn color_edit_options(&self) -> ColorEditOptions {
        ColorEditOptions::from_bits(
            self.inner().ConfigColorEditFlags as u32,
            "Io::color_edit_options()",
        )
    }

    /// Sets the default options used by color edit and picker widgets.
    ///
    /// Unspecified display, data, picker, and input modes use Dear ImGui's defaults.
    #[doc(alias = "ConfigColorEditFlags")]
    pub fn set_color_edit_options(&mut self, options: impl Into<ColorEditOptions>) {
        let options = options.into();
        options.validate("Io::set_color_edit_options()");
        self.inner_mut().ConfigColorEditFlags = options.with_default_choices().bits() as i32;
    }

    /// Returns whether to use MacOSX-style behaviors.
    #[doc(alias = "ConfigMacOSXBehaviors")]
    pub fn config_macosx_behaviors(&self) -> bool {
        self.inner().ConfigMacOSXBehaviors
    }

    /// Set whether to use MacOSX-style behaviors.
    #[doc(alias = "ConfigMacOSXBehaviors")]
    pub fn set_config_macosx_behaviors(&mut self, enabled: bool) {
        self.inner_mut().ConfigMacOSXBehaviors = enabled;
    }

    /// Returns whether to trickle input events through the queue.
    #[doc(alias = "ConfigInputTrickleEventQueue")]
    pub fn config_input_trickle_event_queue(&self) -> bool {
        self.inner().ConfigInputTrickleEventQueue
    }

    /// Set whether to trickle input events through the queue.
    #[doc(alias = "ConfigInputTrickleEventQueue")]
    pub fn set_config_input_trickle_event_queue(&mut self, enabled: bool) {
        self.inner_mut().ConfigInputTrickleEventQueue = enabled;
    }

    /// Returns whether the input text cursor blinks.
    #[doc(alias = "ConfigInputTextCursorBlink")]
    pub fn config_input_text_cursor_blink(&self) -> bool {
        self.inner().ConfigInputTextCursorBlink
    }

    /// Set whether the input text cursor blinks.
    #[doc(alias = "ConfigInputTextCursorBlink")]
    pub fn set_config_input_text_cursor_blink(&mut self, enabled: bool) {
        self.inner_mut().ConfigInputTextCursorBlink = enabled;
    }

    /// Returns whether Enter keeps the input text active.
    #[doc(alias = "ConfigInputTextEnterKeepActive")]
    pub fn config_input_text_enter_keep_active(&self) -> bool {
        self.inner().ConfigInputTextEnterKeepActive
    }

    /// Set whether Enter keeps the input text active.
    #[doc(alias = "ConfigInputTextEnterKeepActive")]
    pub fn set_config_input_text_enter_keep_active(&mut self, enabled: bool) {
        self.inner_mut().ConfigInputTextEnterKeepActive = enabled;
    }

    /// Returns whether click-drag on numeric widgets turns into text input.
    #[doc(alias = "ConfigDragClickToInputText")]
    pub fn config_drag_click_to_input_text(&self) -> bool {
        self.inner().ConfigDragClickToInputText
    }

    /// Set whether click-drag on numeric widgets turns into text input.
    #[doc(alias = "ConfigDragClickToInputText")]
    pub fn set_config_drag_click_to_input_text(&mut self, enabled: bool) {
        self.inner_mut().ConfigDragClickToInputText = enabled;
    }

    /// Returns whether windows can be moved only from their title bar.
    ///
    /// When enabled, click-dragging on empty window content will no longer move the window.
    /// This can be useful in multi-viewport setups to avoid accidental platform window moves
    /// while interacting with in-window widgets (e.g. gizmos in a scene view).
    #[doc(alias = "ConfigWindowsMoveFromTitleBarOnly")]
    pub fn config_windows_move_from_title_bar_only(&self) -> bool {
        self.inner().ConfigWindowsMoveFromTitleBarOnly
    }

    /// Set whether windows can be moved only from their title bar.
    ///
    /// Note: This is typically latched by Dear ImGui at the start of the frame. Prefer setting it
    /// during initialization or before calling `Context::frame()`.
    #[doc(alias = "ConfigWindowsMoveFromTitleBarOnly")]
    pub fn set_config_windows_move_from_title_bar_only(&mut self, enabled: bool) {
        self.inner_mut().ConfigWindowsMoveFromTitleBarOnly = enabled;
    }

    /// Returns whether windows can be resized from their edges.
    #[doc(alias = "ConfigWindowsResizeFromEdges")]
    pub fn config_windows_resize_from_edges(&self) -> bool {
        self.inner().ConfigWindowsResizeFromEdges
    }

    /// Set whether windows can be resized from their edges.
    #[doc(alias = "ConfigWindowsResizeFromEdges")]
    pub fn set_config_windows_resize_from_edges(&mut self, enabled: bool) {
        self.inner_mut().ConfigWindowsResizeFromEdges = enabled;
    }

    /// Returns whether Ctrl+C copies window contents.
    #[doc(alias = "ConfigWindowsCopyContentsWithCtrlC")]
    pub fn config_windows_copy_contents_with_ctrl_c(&self) -> bool {
        self.inner().ConfigWindowsCopyContentsWithCtrlC
    }

    /// Set whether Ctrl+C copies window contents.
    #[doc(alias = "ConfigWindowsCopyContentsWithCtrlC")]
    pub fn set_config_windows_copy_contents_with_ctrl_c(&mut self, enabled: bool) {
        self.inner_mut().ConfigWindowsCopyContentsWithCtrlC = enabled;
    }

    /// Returns whether scrollbars scroll by page.
    #[doc(alias = "ConfigScrollbarScrollByPage")]
    pub fn config_scrollbar_scroll_by_page(&self) -> bool {
        self.inner().ConfigScrollbarScrollByPage
    }

    /// Set whether scrollbars scroll by page.
    #[doc(alias = "ConfigScrollbarScrollByPage")]
    pub fn set_config_scrollbar_scroll_by_page(&mut self, enabled: bool) {
        self.inner_mut().ConfigScrollbarScrollByPage = enabled;
    }

    /// Returns the memory compact timer (seconds).
    #[doc(alias = "ConfigMemoryCompactTimer")]
    pub fn config_memory_compact_timer(&self) -> f32 {
        self.inner().ConfigMemoryCompactTimer
    }

    /// Set the memory compact timer (seconds).
    #[doc(alias = "ConfigMemoryCompactTimer")]
    pub fn set_config_memory_compact_timer(&mut self, seconds: f32) {
        assert_memory_compact_timer("Io::set_config_memory_compact_timer()", seconds);
        self.inner_mut().ConfigMemoryCompactTimer = seconds;
    }

    /// Returns the time for a double-click (seconds).
    #[doc(alias = "MouseDoubleClickTime")]
    pub fn mouse_double_click_time(&self) -> f32 {
        self.inner().MouseDoubleClickTime
    }

    /// Set the time for a double-click (seconds).
    #[doc(alias = "MouseDoubleClickTime")]
    pub fn set_mouse_double_click_time(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_mouse_double_click_time()", "seconds", seconds);
        assert!(
            seconds < self.mouse_single_click_delay(),
            "Io::set_mouse_double_click_time() seconds must be less than MouseSingleClickDelay"
        );
        self.inner_mut().MouseDoubleClickTime = seconds;
    }

    /// Returns the delay used to disambiguate a single click from repeated clicks (seconds).
    #[doc(alias = "MouseSingleClickDelay")]
    pub fn mouse_single_click_delay(&self) -> f32 {
        self.inner().MouseSingleClickDelay
    }

    /// Sets the delay used to disambiguate a single click from repeated clicks (seconds).
    #[doc(alias = "MouseSingleClickDelay")]
    pub fn set_mouse_single_click_delay(&mut self, seconds: f32) {
        assert_positive_f32("Io::set_mouse_single_click_delay()", "seconds", seconds);
        assert!(
            seconds > self.mouse_double_click_time(),
            "Io::set_mouse_single_click_delay() seconds must be greater than MouseDoubleClickTime"
        );
        self.inner_mut().MouseSingleClickDelay = seconds;
    }

    /// Returns the maximum distance to qualify as a double-click (pixels).
    #[doc(alias = "MouseDoubleClickMaxDist")]
    pub fn mouse_double_click_max_dist(&self) -> f32 {
        self.inner().MouseDoubleClickMaxDist
    }

    /// Set the maximum distance to qualify as a double-click (pixels).
    #[doc(alias = "MouseDoubleClickMaxDist")]
    pub fn set_mouse_double_click_max_dist(&mut self, pixels: f32) {
        assert_non_negative_f32("Io::set_mouse_double_click_max_dist()", "pixels", pixels);
        self.inner_mut().MouseDoubleClickMaxDist = pixels;
    }

    /// Returns the distance threshold for starting a drag (pixels).
    #[doc(alias = "MouseDragThreshold")]
    pub fn mouse_drag_threshold(&self) -> f32 {
        self.inner().MouseDragThreshold
    }

    /// Set the distance threshold for starting a drag (pixels).
    #[doc(alias = "MouseDragThreshold")]
    pub fn set_mouse_drag_threshold(&mut self, pixels: f32) {
        assert_non_negative_f32("Io::set_mouse_drag_threshold()", "pixels", pixels);
        self.inner_mut().MouseDragThreshold = pixels;
    }

    /// Returns the key repeat delay (seconds).
    #[doc(alias = "KeyRepeatDelay")]
    pub fn key_repeat_delay(&self) -> f32 {
        self.inner().KeyRepeatDelay
    }

    /// Set the key repeat delay (seconds).
    #[doc(alias = "KeyRepeatDelay")]
    pub fn set_key_repeat_delay(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_key_repeat_delay()", "seconds", seconds);
        self.inner_mut().KeyRepeatDelay = seconds;
    }

    /// Returns the key repeat rate (seconds).
    #[doc(alias = "KeyRepeatRate")]
    pub fn key_repeat_rate(&self) -> f32 {
        self.inner().KeyRepeatRate
    }

    /// Set the key repeat rate (seconds).
    #[doc(alias = "KeyRepeatRate")]
    pub fn set_key_repeat_rate(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_key_repeat_rate()", "seconds", seconds);
        self.inner_mut().KeyRepeatRate = seconds;
    }

    // Navigation

    /// Returns whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn config_nav_swap_gamepad_buttons(&self) -> bool {
        self.inner().ConfigNavSwapGamepadButtons
    }

    /// Set whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn set_config_nav_swap_gamepad_buttons(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavSwapGamepadButtons = enabled;
    }

    /// Returns whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn config_nav_move_set_mouse_pos(&self) -> bool {
        self.inner().ConfigNavMoveSetMousePos
    }

    /// Set whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn set_config_nav_move_set_mouse_pos(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavMoveSetMousePos = enabled;
    }

    /// Returns whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn config_nav_capture_keyboard(&self) -> bool {
        self.inner().ConfigNavCaptureKeyboard
    }

    /// Set whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn set_config_nav_capture_keyboard(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCaptureKeyboard = enabled;
    }

    /// Returns whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn config_nav_escape_clear_focus_item(&self) -> bool {
        self.inner().ConfigNavEscapeClearFocusItem
    }

    /// Set whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn set_config_nav_escape_clear_focus_item(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavEscapeClearFocusItem = enabled;
    }

    /// Returns whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn config_nav_escape_clear_focus_window(&self) -> bool {
        self.inner().ConfigNavEscapeClearFocusWindow
    }

    /// Set whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn set_config_nav_escape_clear_focus_window(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavEscapeClearFocusWindow = enabled;
    }

    /// Returns whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn config_nav_cursor_visible_auto(&self) -> bool {
        self.inner().ConfigNavCursorVisibleAuto
    }

    /// Set whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn set_config_nav_cursor_visible_auto(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCursorVisibleAuto = enabled;
    }

    /// Returns whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn config_nav_cursor_visible_always(&self) -> bool {
        self.inner().ConfigNavCursorVisibleAlways
    }

    /// Set whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn set_config_nav_cursor_visible_always(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCursorVisibleAlways = enabled;
    }

    // Docking

    /// Returns whether docking is prevented from splitting nodes.
    #[doc(alias = "ConfigDockingNoSplit")]
    pub fn config_docking_no_split(&self) -> bool {
        self.inner().ConfigDockingNoSplit
    }

    /// Set whether docking is prevented from splitting nodes.
    #[doc(alias = "ConfigDockingNoSplit")]
    pub fn set_config_docking_no_split(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingNoSplit = enabled;
    }

    /// Returns whether docking over other windows is disabled.
    #[doc(alias = "ConfigDockingNoDockingOver")]
    pub fn config_docking_no_docking_over(&self) -> bool {
        self.inner().ConfigDockingNoDockingOver
    }

    /// Set whether docking over other windows is disabled.
    #[doc(alias = "ConfigDockingNoDockingOver")]
    pub fn set_config_docking_no_docking_over(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingNoDockingOver = enabled;
    }

    /// Returns whether docking requires holding Shift.
    #[doc(alias = "ConfigDockingWithShift")]
    pub fn config_docking_with_shift(&self) -> bool {
        self.inner().ConfigDockingWithShift
    }

    /// Set whether docking requires holding Shift.
    #[doc(alias = "ConfigDockingWithShift")]
    pub fn set_config_docking_with_shift(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingWithShift = enabled;
    }

    /// Returns whether docking uses a tab bar when possible.
    #[doc(alias = "ConfigDockingAlwaysTabBar")]
    pub fn config_docking_always_tab_bar(&self) -> bool {
        self.inner().ConfigDockingAlwaysTabBar
    }

    /// Set whether docking uses a tab bar when possible.
    #[doc(alias = "ConfigDockingAlwaysTabBar")]
    pub fn set_config_docking_always_tab_bar(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingAlwaysTabBar = enabled;
    }

    /// Returns whether docking payloads are rendered transparently.
    #[doc(alias = "ConfigDockingTransparentPayload")]
    pub fn config_docking_transparent_payload(&self) -> bool {
        self.inner().ConfigDockingTransparentPayload
    }

    /// Set whether docking payloads are rendered transparently.
    #[doc(alias = "ConfigDockingTransparentPayload")]
    pub fn set_config_docking_transparent_payload(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingTransparentPayload = enabled;
    }

    // Viewports and DPI

    /// Returns whether viewports should avoid auto-merging.
    #[doc(alias = "ConfigViewportsNoAutoMerge")]
    pub fn config_viewports_no_auto_merge(&self) -> bool {
        self.inner().ConfigViewportsNoAutoMerge
    }

    /// Set whether viewports should avoid auto-merging.
    #[doc(alias = "ConfigViewportsNoAutoMerge")]
    pub fn set_config_viewports_no_auto_merge(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoAutoMerge = enabled;
    }

    /// Returns whether viewports should avoid task bar icons.
    #[doc(alias = "ConfigViewportsNoTaskBarIcon")]
    pub fn config_viewports_no_task_bar_icon(&self) -> bool {
        self.inner().ConfigViewportsNoTaskBarIcon
    }

    /// Set whether viewports should avoid task bar icons.
    #[doc(alias = "ConfigViewportsNoTaskBarIcon")]
    pub fn set_config_viewports_no_task_bar_icon(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoTaskBarIcon = enabled;
    }

    /// Returns whether viewports should avoid platform window decorations.
    #[doc(alias = "ConfigViewportsNoDecoration")]
    pub fn config_viewports_no_decoration(&self) -> bool {
        self.inner().ConfigViewportsNoDecoration
    }

    /// Set whether viewports should avoid platform window decorations.
    #[doc(alias = "ConfigViewportsNoDecoration")]
    pub fn set_config_viewports_no_decoration(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoDecoration = enabled;
    }

    /// Returns whether viewports should not have a default parent.
    #[doc(alias = "ConfigViewportsNoDefaultParent")]
    pub fn config_viewports_no_default_parent(&self) -> bool {
        self.inner().ConfigViewportsNoDefaultParent
    }

    /// Set whether viewports should not have a default parent.
    #[doc(alias = "ConfigViewportsNoDefaultParent")]
    pub fn set_config_viewports_no_default_parent(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoDefaultParent = enabled;
    }

    /// Returns whether platform focus also sets ImGui focus in viewports.
    #[doc(alias = "ConfigViewportsPlatformFocusSetsImGuiFocus")]
    pub fn config_viewports_platform_focus_sets_imgui_focus(&self) -> bool {
        self.inner().ConfigViewportsPlatformFocusSetsImGuiFocus
    }

    /// Set whether platform focus also sets ImGui focus in viewports.
    #[doc(alias = "ConfigViewportsPlatformFocusSetsImGuiFocus")]
    pub fn set_config_viewports_platform_focus_sets_imgui_focus(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsPlatformFocusSetsImGuiFocus = enabled;
    }

    /// Returns whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn config_dpi_scale_fonts(&self) -> bool {
        self.inner().ConfigDpiScaleFonts
    }

    /// Set whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn set_config_dpi_scale_fonts(&mut self, enabled: bool) {
        self.inner_mut().ConfigDpiScaleFonts = enabled;
    }

    /// Returns whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn config_dpi_scale_viewports(&self) -> bool {
        self.inner().ConfigDpiScaleViewports
    }

    /// Set whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn set_config_dpi_scale_viewports(&mut self, enabled: bool) {
        self.inner_mut().ConfigDpiScaleViewports = enabled;
    }

    // Debugging and error recovery

    /// Returns whether every draw list defaults to legacy stroke rendering.
    #[doc(alias = "ConfigDebugDrawListDefaultsToStrokeLegacy")]
    pub fn config_debug_draw_list_defaults_to_stroke_legacy(&self) -> bool {
        self.inner().ConfigDebugDrawListDefaultsToStrokeLegacy
    }

    /// Enables legacy stroke rendering to compare custom drawing across ImGui versions.
    #[doc(alias = "ConfigDebugDrawListDefaultsToStrokeLegacy")]
    pub fn set_config_debug_draw_list_defaults_to_stroke_legacy(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugDrawListDefaultsToStrokeLegacy = enabled;
    }

    /// Returns whether error recovery is enabled.
    #[doc(alias = "ConfigErrorRecovery")]
    pub fn config_error_recovery(&self) -> bool {
        self.inner().ConfigErrorRecovery
    }

    /// Set whether error recovery is enabled.
    #[doc(alias = "ConfigErrorRecovery")]
    pub fn set_config_error_recovery(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecovery = enabled;
    }

    /// Returns whether error recovery enables asserts.
    #[doc(alias = "ConfigErrorRecoveryEnableAssert")]
    pub fn config_error_recovery_enable_assert(&self) -> bool {
        self.inner().ConfigErrorRecoveryEnableAssert
    }

    /// Set whether error recovery enables asserts.
    #[doc(alias = "ConfigErrorRecoveryEnableAssert")]
    pub fn set_config_error_recovery_enable_assert(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecoveryEnableAssert = enabled;
    }

    /// Returns whether error recovery enables debug logs.
    #[doc(alias = "ConfigErrorRecoveryEnableDebugLog")]
    pub fn config_error_recovery_enable_debug_log(&self) -> bool {
        self.inner().ConfigErrorRecoveryEnableDebugLog
    }

    /// Set whether error recovery enables debug logs.
    #[doc(alias = "ConfigErrorRecoveryEnableDebugLog")]
    pub fn set_config_error_recovery_enable_debug_log(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecoveryEnableDebugLog = enabled;
    }

    /// Returns whether error recovery enables tooltips.
    #[doc(alias = "ConfigErrorRecoveryEnableTooltip")]
    pub fn config_error_recovery_enable_tooltip(&self) -> bool {
        self.inner().ConfigErrorRecoveryEnableTooltip
    }

    /// Set whether error recovery enables tooltips.
    #[doc(alias = "ConfigErrorRecoveryEnableTooltip")]
    pub fn set_config_error_recovery_enable_tooltip(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecoveryEnableTooltip = enabled;
    }

    /// Returns whether Dear ImGui thinks a debugger is present.
    #[doc(alias = "ConfigDebugIsDebuggerPresent")]
    pub fn config_debug_is_debugger_present(&self) -> bool {
        self.inner().ConfigDebugIsDebuggerPresent
    }

    /// Set whether Dear ImGui thinks a debugger is present.
    #[doc(alias = "ConfigDebugIsDebuggerPresent")]
    pub fn set_config_debug_is_debugger_present(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugIsDebuggerPresent = enabled;
    }

    /// Returns whether to highlight ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflicts")]
    pub fn config_debug_highlight_id_conflicts(&self) -> bool {
        self.inner().ConfigDebugHighlightIdConflicts
    }

    /// Set whether to highlight ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflicts")]
    pub fn set_config_debug_highlight_id_conflicts(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugHighlightIdConflicts = enabled;
    }

    /// Returns whether to show the item picker when highlighting ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflictsShowItemPicker")]
    pub fn config_debug_highlight_id_conflicts_show_item_picker(&self) -> bool {
        self.inner().ConfigDebugHighlightIdConflictsShowItemPicker
    }

    /// Set whether to show the item picker when highlighting ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflictsShowItemPicker")]
    pub fn set_config_debug_highlight_id_conflicts_show_item_picker(&mut self, enabled: bool) {
        self.inner_mut()
            .ConfigDebugHighlightIdConflictsShowItemPicker = enabled;
    }

    /// Returns whether `Begin()` returns `true` once.
    #[doc(alias = "ConfigDebugBeginReturnValueOnce")]
    pub fn config_debug_begin_return_value_once(&self) -> bool {
        self.inner().ConfigDebugBeginReturnValueOnce
    }

    /// Set whether `Begin()` returns `true` once.
    #[doc(alias = "ConfigDebugBeginReturnValueOnce")]
    pub fn set_config_debug_begin_return_value_once(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugBeginReturnValueOnce = enabled;
    }

    /// Returns whether `Begin()` returns `true` in a loop.
    #[doc(alias = "ConfigDebugBeginReturnValueLoop")]
    pub fn config_debug_begin_return_value_loop(&self) -> bool {
        self.inner().ConfigDebugBeginReturnValueLoop
    }

    /// Set whether `Begin()` returns `true` in a loop.
    #[doc(alias = "ConfigDebugBeginReturnValueLoop")]
    pub fn set_config_debug_begin_return_value_loop(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugBeginReturnValueLoop = enabled;
    }

    /// Returns whether to ignore focus loss.
    #[doc(alias = "ConfigDebugIgnoreFocusLoss")]
    pub fn config_debug_ignore_focus_loss(&self) -> bool {
        self.inner().ConfigDebugIgnoreFocusLoss
    }

    /// Set whether to ignore focus loss.
    #[doc(alias = "ConfigDebugIgnoreFocusLoss")]
    pub fn set_config_debug_ignore_focus_loss(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugIgnoreFocusLoss = enabled;
    }

    /// Returns whether to display ini settings debug tools.
    #[doc(alias = "ConfigDebugIniSettings")]
    pub fn config_debug_ini_settings(&self) -> bool {
        self.inner().ConfigDebugIniSettings
    }

    /// Set whether to display ini settings debug tools.
    #[doc(alias = "ConfigDebugIniSettings")]
    pub fn set_config_debug_ini_settings(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugIniSettings = enabled;
    }
}

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn assert_display_size(caller: &str, size: [f32; 2]) {
    assert_finite_vec2(caller, "size", size);
    assert!(
        size[0] >= 0.0 && size[1] >= 0.0,
        "{caller} size must contain non-negative values"
    );
}

pub(crate) fn assert_display_framebuffer_scale(caller: &str, scale: [f32; 2]) {
    assert_finite_vec2(caller, "scale", scale);
    assert!(
        scale[0] > 0.0 && scale[1] > 0.0,
        "{caller} scale must contain positive values"
    );
}

fn assert_memory_compact_timer(caller: &str, seconds: f32) {
    assert_finite_f32(caller, "seconds", seconds);
    assert!(
        seconds >= 0.0 || seconds == -1.0,
        "{caller} seconds must be non-negative, or -1.0 to disable"
    );
}

fn metric_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}
