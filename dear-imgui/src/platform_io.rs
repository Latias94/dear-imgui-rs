//! Platform IO functionality for Dear ImGui
//!
//! This module provides access to Dear ImGui's platform IO system, which handles
//! multi-viewport and platform-specific functionality.

use crate::sys;
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

    /// Get the viewport position
    pub fn pos(&self) -> [f32; 2] {
        [self.raw.Pos.x, self.raw.Pos.y]
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
}

// TODO: Add more viewport functionality:
// - Platform window handle access
// - Renderer data access
// - DPI scale information
// - Monitor information
