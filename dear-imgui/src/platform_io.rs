//! Platform IO functionality for Dear ImGui
//!
//! This module provides access to Dear ImGui's platform IO system, which handles
//! multi-viewport and platform-specific functionality.

use crate::sys;
use std::ffi::{c_char, c_void};
use std::ptr;

/// Platform IO structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiPlatformIO` that provides
/// safe access to platform-specific functionality.
#[repr(transparent)]
pub struct PlatformIo {
    raw: sys::ImGuiPlatformIO,
}

impl PlatformIo {
    /// Get a reference to the platform IO from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiPlatformIO` structure.
    pub unsafe fn from_raw(raw: *const sys::ImGuiPlatformIO) -> &'static Self {
        &*(raw as *const Self)
    }

    /// Get a mutable reference to the platform IO from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiPlatformIO` structure, and that no other references exist.
    pub unsafe fn from_raw_mut(raw: *mut sys::ImGuiPlatformIO) -> &'static mut Self {
        &mut *(raw as *mut Self)
    }

    /// Get the raw pointer to the underlying `ImGuiPlatformIO`
    pub fn as_raw(&self) -> *const sys::ImGuiPlatformIO {
        &self.raw as *const _
    }

    /// Get the raw mutable pointer to the underlying `ImGuiPlatformIO`
    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiPlatformIO {
        &mut self.raw as *mut _
    }

    /// Set platform create window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Platform_CreateWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform destroy window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Platform_DestroyWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform show window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_show_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Platform_ShowWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform set window position callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        self.raw.Platform_SetWindowPos = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform get window position callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    ) {
        self.raw.Platform_GetWindowPos = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform set window size callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        self.raw.Platform_SetWindowSize = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform get window size callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    ) {
        self.raw.Platform_GetWindowSize = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform set window focus callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_focus(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Platform_SetWindowFocus = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform get window focus callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_focus(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    ) {
        self.raw.Platform_GetWindowFocus = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform get window minimized callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_minimized(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    ) {
        self.raw.Platform_GetWindowMinimized = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform set window title callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_title(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *const c_char)>,
    ) {
        self.raw.Platform_SetWindowTitle = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform set window alpha callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_alpha(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, f32)>,
    ) {
        self.raw.Platform_SetWindowAlpha = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform update window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_update_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Platform_UpdateWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform render window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        self.raw.Platform_RenderWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set platform swap buffers callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        self.raw.Platform_SwapBuffers = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set renderer create window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Renderer_CreateWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set renderer destroy window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.raw.Renderer_DestroyWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set renderer set window size callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        self.raw.Renderer_SetWindowSize = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set renderer render window callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        self.raw.Renderer_RenderWindow = callback.map(|f| unsafe { std::mem::transmute(f) });
    }

    /// Set renderer swap buffers callback
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        self.raw.Renderer_SwapBuffers = callback.map(|f| unsafe { std::mem::transmute(f) });
    }
}

// TODO: Add safe wrappers for platform IO functionality:
// - Viewport management
// - Platform backend callbacks
// - Renderer backend callbacks
// - Monitor information
// - Platform-specific settings

/// Viewport structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiViewport` that provides
/// safe access to viewport functionality.
#[repr(transparent)]
pub struct Viewport {
    raw: sys::ImGuiViewport,
}

impl Viewport {
    /// Get a reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiViewport` structure.
    pub unsafe fn from_raw(raw: *const sys::ImGuiViewport) -> &'static Self {
        &*(raw as *const Self)
    }

    /// Get a mutable reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiViewport` structure, and that no other references exist.
    pub unsafe fn from_raw_mut(raw: *mut sys::ImGuiViewport) -> &'static mut Self {
        &mut *(raw as *mut Self)
    }

    /// Get the raw pointer to the underlying `ImGuiViewport`
    pub fn as_raw(&self) -> *const sys::ImGuiViewport {
        &self.raw as *const _
    }

    /// Get the raw mutable pointer to the underlying `ImGuiViewport`
    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiViewport {
        &mut self.raw as *mut _
    }

    /// Get the viewport ID
    pub fn id(&self) -> sys::ImGuiID {
        self.raw.ID
    }

    /// Set the viewport position
    pub fn set_pos(&mut self, pos: [f32; 2]) {
        self.raw.Pos.x = pos[0];
        self.raw.Pos.y = pos[1];
    }

    /// Get the viewport position
    pub fn pos(&self) -> [f32; 2] {
        [self.raw.Pos.x, self.raw.Pos.y]
    }

    /// Set the viewport size
    pub fn set_size(&mut self, size: [f32; 2]) {
        self.raw.Size.x = size[0];
        self.raw.Size.y = size[1];
    }

    /// Get the viewport size
    pub fn size(&self) -> [f32; 2] {
        [self.raw.Size.x, self.raw.Size.y]
    }

    /// Get the viewport work position (excluding menu bars, task bars, etc.)
    pub fn work_pos(&self) -> [f32; 2] {
        [self.raw.WorkPos.x, self.raw.WorkPos.y]
    }

    /// Get the viewport work size (excluding menu bars, task bars, etc.)
    pub fn work_size(&self) -> [f32; 2] {
        [self.raw.WorkSize.x, self.raw.WorkSize.y]
    }

    /// Check if this is the main viewport
    ///
    /// Note: Main viewport is typically identified by ID == 0 or by checking if it's not a platform window
    pub fn is_main(&self) -> bool {
        self.raw.ID == 0 || (self.raw.Flags & sys::ImGuiViewportFlags_IsPlatformWindow as i32) == 0
    }

    /// Check if the viewport is minimized
    pub fn is_minimized(&self) -> bool {
        (self.raw.Flags & sys::ImGuiViewportFlags_IsMinimized as i32) != 0
    }

    /// Check if the viewport has focus
    pub fn is_focused(&self) -> bool {
        (self.raw.Flags & sys::ImGuiViewportFlags_IsFocused as i32) != 0
    }

    /// Get the platform user data
    pub fn platform_user_data(&self) -> *mut c_void {
        self.raw.PlatformUserData
    }

    /// Set the platform user data
    pub fn set_platform_user_data(&mut self, data: *mut c_void) {
        self.raw.PlatformUserData = data;
    }

    /// Get the renderer user data
    pub fn renderer_user_data(&self) -> *mut c_void {
        self.raw.RendererUserData
    }

    /// Set the renderer user data
    pub fn set_renderer_user_data(&mut self, data: *mut c_void) {
        self.raw.RendererUserData = data;
    }

    /// Get the platform handle
    pub fn platform_handle(&self) -> *mut c_void {
        self.raw.PlatformHandle
    }

    /// Set the platform handle
    pub fn set_platform_handle(&mut self, handle: *mut c_void) {
        self.raw.PlatformHandle = handle;
    }

    /// Check if the platform window was created
    pub fn platform_window_created(&self) -> bool {
        self.raw.PlatformWindowCreated
    }

    /// Set whether the platform window was created
    pub fn set_platform_window_created(&mut self, created: bool) {
        self.raw.PlatformWindowCreated = created;
    }

    /// Check if the platform requested move
    pub fn platform_request_move(&self) -> bool {
        self.raw.PlatformRequestMove
    }

    /// Set whether the platform requested move
    pub fn set_platform_request_move(&mut self, request: bool) {
        self.raw.PlatformRequestMove = request;
    }

    /// Check if the platform requested resize
    pub fn platform_request_resize(&self) -> bool {
        self.raw.PlatformRequestResize
    }

    /// Set whether the platform requested resize
    pub fn set_platform_request_resize(&mut self, request: bool) {
        self.raw.PlatformRequestResize = request;
    }

    /// Check if the platform requested close
    pub fn platform_request_close(&self) -> bool {
        self.raw.PlatformRequestClose
    }

    /// Set whether the platform requested close
    pub fn set_platform_request_close(&mut self, request: bool) {
        self.raw.PlatformRequestClose = request;
    }

    /// Get the viewport flags
    pub fn flags(&self) -> sys::ImGuiViewportFlags {
        self.raw.Flags
    }

    /// Set the viewport flags
    pub fn set_flags(&mut self, flags: sys::ImGuiViewportFlags) {
        self.raw.Flags = flags;
    }
}

// TODO: Add more viewport functionality:
// - Platform window handle access
// - Renderer data access
// - DPI scale information
// - Monitor information
