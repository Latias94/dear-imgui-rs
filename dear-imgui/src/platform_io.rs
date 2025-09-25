//! Platform IO functionality for Dear ImGui
//!
//! This module provides access to Dear ImGui's platform IO system, which handles
//! multi-viewport and platform-specific functionality.

use crate::sys;
use std::ffi::c_void;
#[cfg(feature = "multi-viewport")]
use std::sync::Mutex;

/// Platform IO structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiPlatformIO` that provides
/// safe access to platform-specific functionality.
#[repr(transparent)]
pub struct PlatformIo {
    raw: sys::ImGuiPlatformIO,
}

// Typed-callback trampolines (avoid transmute) --------------------------------
#[cfg(feature = "multi-viewport")]
mod trampolines {
    use super::*;

    // Platform callbacks
    pub static PLATFORM_CREATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_DESTROY_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_SHOW_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_POS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    > = Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_POS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_SIZE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    > = Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_SIZE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_FOCUS_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_FOCUS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    > = Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_MINIMIZED_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_TITLE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *const c_char)>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_ALPHA_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, f32)>,
    > = Mutex::new(None);
    pub static PLATFORM_UPDATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_RENDER_WINDOW_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);
    pub static PLATFORM_SWAP_BUFFERS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);

    // Renderer callbacks
    pub static RENDERER_CREATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static RENDERER_DESTROY_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static RENDERER_SET_WINDOW_SIZE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    > = Mutex::new(None);
    pub static RENDERER_RENDER_WINDOW_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);
    pub static RENDERER_SWAP_BUFFERS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);

    // Trampolines for platform callbacks
    pub unsafe extern "C" fn platform_create_window(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *PLATFORM_CREATE_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn platform_destroy_window(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *PLATFORM_DESTROY_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn platform_show_window(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *PLATFORM_SHOW_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn platform_set_window_pos(vp: *mut sys::ImGuiViewport, p: sys::ImVec2) {
        if let Some(cb) = *PLATFORM_SET_WINDOW_POS_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, p) }
        }
    }
    pub unsafe extern "C" fn platform_get_window_pos(vp: *mut sys::ImGuiViewport) -> sys::ImVec2 {
        if let Some(cb) = *PLATFORM_GET_WINDOW_POS_CB.lock().unwrap() {
            return unsafe { cb(vp as *mut Viewport) };
        }
        sys::ImVec2 { x: 0.0, y: 0.0 }
    }
    pub unsafe extern "C" fn platform_set_window_size(vp: *mut sys::ImGuiViewport, s: sys::ImVec2) {
        if let Some(cb) = *PLATFORM_SET_WINDOW_SIZE_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, s) }
        }
    }
    pub unsafe extern "C" fn platform_get_window_size(vp: *mut sys::ImGuiViewport) -> sys::ImVec2 {
        if let Some(cb) = *PLATFORM_GET_WINDOW_SIZE_CB.lock().unwrap() {
            return unsafe { cb(vp as *mut Viewport) };
        }
        sys::ImVec2 { x: 0.0, y: 0.0 }
    }
    pub unsafe extern "C" fn platform_set_window_focus(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *PLATFORM_SET_WINDOW_FOCUS_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn platform_get_window_focus(vp: *mut sys::ImGuiViewport) -> bool {
        if let Some(cb) = *PLATFORM_GET_WINDOW_FOCUS_CB.lock().unwrap() {
            return unsafe { cb(vp as *mut Viewport) };
        }
        false
    }
    pub unsafe extern "C" fn platform_get_window_minimized(vp: *mut sys::ImGuiViewport) -> bool {
        if let Some(cb) = *PLATFORM_GET_WINDOW_MINIMIZED_CB.lock().unwrap() {
            return unsafe { cb(vp as *mut Viewport) };
        }
        false
    }
    pub unsafe extern "C" fn platform_set_window_title(
        vp: *mut sys::ImGuiViewport,
        title: *const c_char,
    ) {
        if let Some(cb) = *PLATFORM_SET_WINDOW_TITLE_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, title) }
        }
    }
    pub unsafe extern "C" fn platform_set_window_alpha(vp: *mut sys::ImGuiViewport, a: f32) {
        if let Some(cb) = *PLATFORM_SET_WINDOW_ALPHA_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, a) }
        }
    }
    pub unsafe extern "C" fn platform_update_window(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *PLATFORM_UPDATE_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn platform_render_window(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if let Some(cb) = *PLATFORM_RENDER_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, r) }
        }
    }
    pub unsafe extern "C" fn platform_swap_buffers(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if let Some(cb) = *PLATFORM_SWAP_BUFFERS_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, r) }
        }
    }

    // Trampolines for renderer callbacks
    pub unsafe extern "C" fn renderer_create_window(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *RENDERER_CREATE_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn renderer_destroy_window(vp: *mut sys::ImGuiViewport) {
        if let Some(cb) = *RENDERER_DESTROY_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport) }
        }
    }
    pub unsafe extern "C" fn renderer_set_window_size(vp: *mut sys::ImGuiViewport, s: sys::ImVec2) {
        if let Some(cb) = *RENDERER_SET_WINDOW_SIZE_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, s) }
        }
    }
    pub unsafe extern "C" fn renderer_render_window(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if let Some(cb) = *RENDERER_RENDER_WINDOW_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, r) }
        }
    }
    pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if let Some(cb) = *RENDERER_SWAP_BUFFERS_CB.lock().unwrap() {
            unsafe { cb(vp as *mut Viewport, r) }
        }
    }
}

impl PlatformIo {
    /// Get a reference to the platform IO from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiPlatformIO` structure.
    pub unsafe fn from_raw(raw: *const sys::ImGuiPlatformIO) -> &'static Self {
        unsafe { &*(raw as *const Self) }
    }

    /// Get a mutable reference to the platform IO from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiPlatformIO` structure, and that no other references exist.
    pub unsafe fn from_raw_mut(raw: *mut sys::ImGuiPlatformIO) -> &'static mut Self {
        unsafe { &mut *(raw as *mut Self) }
    }

    /// Get the raw pointer to the underlying `ImGuiPlatformIO`
    pub fn as_raw(&self) -> *const sys::ImGuiPlatformIO {
        &self.raw as *const _
    }

    /// Get the raw mutable pointer to the underlying `ImGuiPlatformIO`
    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiPlatformIO {
        &mut self.raw as *mut _
    }

    /// Set platform create window callback (raw sys pointer)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Platform_CreateWindow = callback;
    }

    /// Set platform create window callback (typed Viewport). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *PLATFORM_CREATE_WINDOW_CB.lock().unwrap() = callback;
        self.set_platform_create_window_raw(callback.map(|_| {
            trampolines::platform_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Platform_DestroyWindow = callback;
    }

    /// Set platform destroy window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *PLATFORM_DESTROY_WINDOW_CB.lock().unwrap() = callback;
        self.set_platform_destroy_window_raw(callback.map(|_| {
            trampolines::platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform show window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_show_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Platform_ShowWindow = callback;
    }

    /// Set platform show window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_show_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *PLATFORM_SHOW_WINDOW_CB.lock().unwrap() = callback;
        self.set_platform_show_window_raw(callback.map(|_| {
            trampolines::platform_show_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform set window position callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.raw.Platform_SetWindowPos = callback;
    }

    /// Set platform set window position callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        use trampolines::*;
        *PLATFORM_SET_WINDOW_POS_CB.lock().unwrap() = callback;
        self.set_platform_set_window_pos_raw(callback.map(|_| {
            trampolines::platform_set_window_pos
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
    }

    /// Set platform get window position callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2>,
    ) {
        self.raw.Platform_GetWindowPos = callback;
    }

    /// Set platform get window position callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    ) {
        use trampolines::*;
        *PLATFORM_GET_WINDOW_POS_CB.lock().unwrap() = callback;
        self.set_platform_get_window_pos_raw(callback.map(|_| {
            trampolines::platform_get_window_pos
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2
        }));
    }

    /// Set platform set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.raw.Platform_SetWindowSize = callback;
    }

    /// Set platform set window size callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        use trampolines::*;
        *PLATFORM_SET_WINDOW_SIZE_CB.lock().unwrap() = callback;
        self.set_platform_set_window_size_raw(callback.map(|_| {
            trampolines::platform_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
    }

    /// Set platform get window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2>,
    ) {
        self.raw.Platform_GetWindowSize = callback;
    }

    /// Set platform get window size callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    ) {
        use trampolines::*;
        *PLATFORM_GET_WINDOW_SIZE_CB.lock().unwrap() = callback;
        self.set_platform_get_window_size_raw(callback.map(|_| {
            trampolines::platform_get_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2
        }));
    }

    /// Set platform set window focus callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_focus_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Platform_SetWindowFocus = callback;
    }

    /// Set platform set window focus callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_focus(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *PLATFORM_SET_WINDOW_FOCUS_CB.lock().unwrap() = callback;
        self.set_platform_set_window_focus_raw(callback.map(|_| {
            trampolines::platform_set_window_focus as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform get window focus callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_focus_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool>,
    ) {
        self.raw.Platform_GetWindowFocus = callback;
    }

    /// Set platform get window focus callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_focus(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    ) {
        use trampolines::*;
        *PLATFORM_GET_WINDOW_FOCUS_CB.lock().unwrap() = callback;
        self.set_platform_get_window_focus_raw(callback.map(|_| {
            trampolines::platform_get_window_focus
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool
        }));
    }

    /// Set platform get window minimized callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_minimized_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool>,
    ) {
        self.raw.Platform_GetWindowMinimized = callback;
    }

    /// Set platform get window minimized callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_minimized(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    ) {
        use trampolines::*;
        *PLATFORM_GET_WINDOW_MINIMIZED_CB.lock().unwrap() = callback;
        self.set_platform_get_window_minimized_raw(callback.map(|_| {
            trampolines::platform_get_window_minimized
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool
        }));
    }

    /// Set platform set window title callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_title_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const c_char)>,
    ) {
        self.raw.Platform_SetWindowTitle = callback;
    }

    /// Set platform set window title callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_title(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *const c_char)>,
    ) {
        use trampolines::*;
        *PLATFORM_SET_WINDOW_TITLE_CB.lock().unwrap() = callback;
        self.set_platform_set_window_title_raw(callback.map(|_| {
            trampolines::platform_set_window_title
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *const c_char)
        }));
    }

    /// Set platform set window alpha callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_alpha_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)>,
    ) {
        self.raw.Platform_SetWindowAlpha = callback;
    }

    /// Set platform set window alpha callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_alpha(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, f32)>,
    ) {
        use trampolines::*;
        *PLATFORM_SET_WINDOW_ALPHA_CB.lock().unwrap() = callback;
        self.set_platform_set_window_alpha_raw(callback.map(|_| {
            trampolines::platform_set_window_alpha
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)
        }));
    }

    /// Set platform update window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_update_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Platform_UpdateWindow = callback;
    }

    /// Set platform update window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_update_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *PLATFORM_UPDATE_WINDOW_CB.lock().unwrap() = callback;
        self.set_platform_update_window_raw(callback.map(|_| {
            trampolines::platform_update_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.raw.Platform_RenderWindow = callback;
    }

    /// Set platform render window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        *PLATFORM_RENDER_WINDOW_CB.lock().unwrap() = callback;
        self.set_platform_render_window_raw(callback.map(|_| {
            trampolines::platform_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Set platform swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.raw.Platform_SwapBuffers = callback;
    }

    /// Set platform swap buffers callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        *PLATFORM_SWAP_BUFFERS_CB.lock().unwrap() = callback;
        self.set_platform_swap_buffers_raw(callback.map(|_| {
            trampolines::platform_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Set renderer create window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Renderer_CreateWindow = callback;
    }

    /// Set renderer create window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *RENDERER_CREATE_WINDOW_CB.lock().unwrap() = callback;
        self.set_renderer_create_window_raw(callback.map(|_| {
            trampolines::renderer_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set renderer destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.raw.Renderer_DestroyWindow = callback;
    }

    /// Set renderer destroy window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        *RENDERER_DESTROY_WINDOW_CB.lock().unwrap() = callback;
        self.set_renderer_destroy_window_raw(callback.map(|_| {
            trampolines::renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set renderer set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.raw.Renderer_SetWindowSize = callback;
    }

    /// Set renderer set window size callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        use trampolines::*;
        *RENDERER_SET_WINDOW_SIZE_CB.lock().unwrap() = callback;
        self.set_renderer_set_window_size_raw(callback.map(|_| {
            trampolines::renderer_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
    }

    /// Set renderer render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.raw.Renderer_RenderWindow = callback;
    }

    /// Set renderer render window callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        *RENDERER_RENDER_WINDOW_CB.lock().unwrap() = callback;
        self.set_renderer_render_window_raw(callback.map(|_| {
            trampolines::renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Set renderer swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.raw.Renderer_SwapBuffers = callback;
    }

    /// Set renderer swap buffers callback (typed). Unsafe due to FFI pointer cast.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        *RENDERER_SWAP_BUFFERS_CB.lock().unwrap() = callback;
        self.set_renderer_swap_buffers_raw(callback.map(|_| {
            trampolines::renderer_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Get access to the monitors vector
    #[cfg(feature = "multi-viewport")]
    pub fn monitors(&self) -> &ImVector<sys::ImGuiPlatformMonitor> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                sys::ImGuiPlatformMonitor,
                sys::ImVector_ImGuiPlatformMonitor,
            >(&self.raw.Monitors)
        }
    }

    /// Get mutable access to the monitors vector
    #[cfg(feature = "multi-viewport")]
    pub fn monitors_mut(&mut self) -> &mut ImVector<sys::ImGuiPlatformMonitor> {
        unsafe {
            crate::internal::imvector_cast_mut::<
                sys::ImGuiPlatformMonitor,
                sys::ImVector_ImGuiPlatformMonitor,
            >(&mut self.raw.Monitors)
        }
    }

    /// Get access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub fn viewports(&self) -> &ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&self.raw.Viewports)
        }
    }

    /// Get mutable access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_mut(&mut self) -> &mut ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_mut::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&mut self.raw.Viewports)
        }
    }

    /// Get an iterator over all viewports
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_iter(&self) -> impl Iterator<Item = &Viewport> {
        self.viewports()
            .iter()
            .map(|&ptr| unsafe { Viewport::from_raw(ptr) })
    }

    /// Get a mutable iterator over all viewports
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_iter_mut(&mut self) -> impl Iterator<Item = &mut Viewport> {
        self.viewports_mut()
            .iter_mut()
            .map(|&mut ptr| unsafe { Viewport::from_raw_mut(ptr) })
    }

    /// Get an iterator over all textures managed by the platform
    ///
    /// This is used by renderer backends during shutdown to destroy all textures.
    pub fn textures(&self) -> crate::render::draw_data::TextureIterator<'_> {
        unsafe {
            let vector = &self.raw.Textures;
            crate::render::draw_data::TextureIterator::new(
                vector.Data,
                vector.Data.add(vector.Size as usize),
            )
        }
    }

    /// Get the number of textures managed by the platform
    pub fn textures_count(&self) -> usize {
        self.raw.Textures.Size as usize
    }

    /// Get a specific texture by index
    ///
    /// Returns None if the index is out of bounds.
    pub fn texture(&self, index: usize) -> Option<&crate::texture::TextureData> {
        unsafe {
            let vector = &self.raw.Textures;
            if index >= vector.Size as usize {
                return None;
            }
            let texture_ptr = *vector.Data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw(texture_ptr))
        }
    }

    /// Get a mutable reference to a specific texture by index
    ///
    /// Returns None if the index is out of bounds.
    pub fn texture_mut(&mut self, index: usize) -> Option<&mut crate::texture::TextureData> {
        unsafe {
            let vector = &self.raw.Textures;
            if index >= vector.Size as usize {
                return None;
            }
            let texture_ptr = *vector.Data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw(texture_ptr))
        }
    }

    /// Set the renderer render state
    ///
    /// This is used by renderer backends to expose their current render state
    /// to draw callbacks during rendering. The pointer should remain valid
    /// during the entire render_draw_data() call.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The pointer is valid for the duration of the render call
    /// - The pointed-to data matches the expected render state structure for the backend
    /// - The pointer is set to null after rendering is complete
    pub unsafe fn set_renderer_render_state(&mut self, render_state: *mut std::ffi::c_void) {
        self.raw.Renderer_RenderState = render_state;
    }

    /// Get the current renderer render state
    ///
    /// Returns the render state pointer that was set by the renderer backend.
    /// This is typically used by draw callbacks to access the current render state.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The returned pointer is cast to the correct render state type for the backend
    /// - The pointer is only used during the render_draw_data() call
    pub unsafe fn renderer_render_state(&self) -> *mut std::ffi::c_void {
        self.raw.Renderer_RenderState
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
        unsafe { &*(raw as *const Self) }
    }

    /// Get a mutable reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// `ImGuiViewport` structure, and that no other references exist.
    pub unsafe fn from_raw_mut(raw: *mut sys::ImGuiViewport) -> &'static mut Self {
        unsafe { &mut *(raw as *mut Self) }
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
    #[cfg(feature = "multi-viewport")]
    pub fn is_main(&self) -> bool {
        self.raw.ID == 0
            || (self.raw.Flags & (crate::ViewportFlags::IS_PLATFORM_WINDOW.bits())) == 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_main(&self) -> bool {
        self.raw.ID == 0
    }

    /// Check if this is a platform window (not the main viewport)
    #[cfg(feature = "multi-viewport")]
    pub fn is_platform_window(&self) -> bool {
        (self.raw.Flags & (crate::ViewportFlags::IS_PLATFORM_WINDOW.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_platform_window(&self) -> bool {
        false
    }

    /// Check if this is a platform monitor
    #[cfg(feature = "multi-viewport")]
    pub fn is_platform_monitor(&self) -> bool {
        (self.raw.Flags & (crate::ViewportFlags::IS_PLATFORM_MONITOR.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_platform_monitor(&self) -> bool {
        false
    }

    /// Check if this viewport is owned by the application
    #[cfg(feature = "multi-viewport")]
    pub fn is_owned_by_app(&self) -> bool {
        (self.raw.Flags & (crate::ViewportFlags::OWNED_BY_APP.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_owned_by_app(&self) -> bool {
        false
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

    /// Get the DPI scale factor
    #[cfg(feature = "multi-viewport")]
    pub fn dpi_scale(&self) -> f32 {
        self.raw.DpiScale
    }

    /// Set the DPI scale factor
    #[cfg(feature = "multi-viewport")]
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.raw.DpiScale = scale;
    }

    /// Get the parent viewport ID
    #[cfg(feature = "multi-viewport")]
    pub fn parent_viewport_id(&self) -> sys::ImGuiID {
        self.raw.ParentViewportId
    }

    /// Set the parent viewport ID
    #[cfg(feature = "multi-viewport")]
    pub fn set_parent_viewport_id(&mut self, id: sys::ImGuiID) {
        self.raw.ParentViewportId = id;
    }

    /// Get the draw data pointer
    #[cfg(feature = "multi-viewport")]
    pub fn draw_data(&self) -> *mut sys::ImDrawData {
        self.raw.DrawData
    }

    /// Get the draw data as a reference (if available)
    #[cfg(feature = "multi-viewport")]
    pub fn draw_data_ref(&self) -> Option<&sys::ImDrawData> {
        if self.raw.DrawData.is_null() {
            None
        } else {
            Some(unsafe { &*self.raw.DrawData })
        }
    }

    /// Get the framebuffer scale
    #[cfg(feature = "multi-viewport")]
    pub fn framebuffer_scale(&self) -> [f32; 2] {
        [self.raw.FramebufferScale.x, self.raw.FramebufferScale.y]
    }

    /// Set the framebuffer scale
    #[cfg(feature = "multi-viewport")]
    pub fn set_framebuffer_scale(&mut self, scale: [f32; 2]) {
        self.raw.FramebufferScale.x = scale[0];
        self.raw.FramebufferScale.y = scale[1];
    }
}

// TODO: Add more viewport functionality:
// - Platform window handle access
// - Renderer data access
// - DPI scale information
// - Monitor information
