//! Texture management for Dear ImGui
//!
//! This module provides access to Dear ImGui's modern texture management system
//! introduced in version 1.92+. It includes support for ImTextureData, texture
//! status management, and automatic texture updates.

use crate::sys;
use std::ffi::c_void;

/// Simple texture ID for backward compatibility
///
/// This is a simple wrapper around usize that can be used to identify textures.
/// For modern texture management, use TextureData instead.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct TextureId(usize);

impl TextureId {
    /// Creates a new texture id with the given identifier
    #[inline]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Returns the id of the TextureId
    #[inline]
    pub const fn id(self) -> usize {
        self.0
    }

    /// Creates a null texture ID
    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Checks if this texture ID is null
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl From<usize> for TextureId {
    #[inline]
    fn from(id: usize) -> Self {
        TextureId(id)
    }
}

impl<T> From<*const T> for TextureId {
    #[inline]
    fn from(ptr: *const T) -> Self {
        TextureId(ptr as usize)
    }
}

impl<T> From<*mut T> for TextureId {
    #[inline]
    fn from(ptr: *mut T) -> Self {
        TextureId(ptr as usize)
    }
}

impl From<TextureId> for *const c_void {
    #[inline]
    fn from(id: TextureId) -> Self {
        id.0 as *const c_void
    }
}

impl From<TextureId> for *mut c_void {
    #[inline]
    fn from(id: TextureId) -> Self {
        id.0 as *mut c_void
    }
}

impl Default for TextureId {
    #[inline]
    fn default() -> Self {
        Self::null()
    }
}

/// Raw texture ID type for compatibility with Dear ImGui
pub type RawTextureId = *const c_void;

/// Texture format supported by Dear ImGui
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureFormat {
    /// 4 components per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight * 4
    RGBA32 = sys::ImTextureFormat_RGBA32,
    /// 1 component per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight
    Alpha8 = sys::ImTextureFormat_Alpha8,
}

impl From<sys::ImTextureFormat> for TextureFormat {
    fn from(format: sys::ImTextureFormat) -> Self {
        match format {
            sys::ImTextureFormat_RGBA32 => TextureFormat::RGBA32,
            sys::ImTextureFormat_Alpha8 => TextureFormat::Alpha8,
            _ => TextureFormat::RGBA32, // Default fallback
        }
    }
}

impl From<TextureFormat> for sys::ImTextureFormat {
    fn from(format: TextureFormat) -> Self {
        format as sys::ImTextureFormat
    }
}

/// Status of a texture to communicate with Renderer Backend
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureStatus {
    /// Texture is ready and can be used
    OK = sys::ImTextureStatus_OK,
    /// Backend destroyed the texture
    Destroyed = sys::ImTextureStatus_Destroyed,
    /// Requesting backend to create the texture. Set status OK when done.
    WantCreate = sys::ImTextureStatus_WantCreate,
    /// Requesting backend to update specific blocks of pixels. Set status OK when done.
    WantUpdates = sys::ImTextureStatus_WantUpdates,
    /// Requesting backend to destroy the texture. Set status to Destroyed when done.
    WantDestroy = sys::ImTextureStatus_WantDestroy,
}

impl From<sys::ImTextureStatus> for TextureStatus {
    fn from(status: sys::ImTextureStatus) -> Self {
        match status {
            sys::ImTextureStatus_OK => TextureStatus::OK,
            sys::ImTextureStatus_Destroyed => TextureStatus::Destroyed,
            sys::ImTextureStatus_WantCreate => TextureStatus::WantCreate,
            sys::ImTextureStatus_WantUpdates => TextureStatus::WantUpdates,
            sys::ImTextureStatus_WantDestroy => TextureStatus::WantDestroy,
            _ => TextureStatus::Destroyed, // Default fallback
        }
    }
}

impl From<TextureStatus> for sys::ImTextureStatus {
    fn from(status: TextureStatus) -> Self {
        status as sys::ImTextureStatus
    }
}

/// Coordinates of a rectangle within a texture
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TextureRect {
    /// Upper-left X coordinate of rectangle to update
    pub x: u16,
    /// Upper-left Y coordinate of rectangle to update
    pub y: u16,
    /// Width of rectangle to update
    pub w: u16,
    /// Height of rectangle to update
    pub h: u16,
}

impl From<sys::ImTextureRect> for TextureRect {
    fn from(rect: sys::ImTextureRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        }
    }
}

impl From<TextureRect> for sys::ImTextureRect {
    fn from(rect: TextureRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        }
    }
}

/// Texture data managed by Dear ImGui
///
/// This is a wrapper around ImTextureData that provides safe access to
/// texture information and pixel data. It's used by renderer backends
/// to create, update, and destroy textures.
#[repr(transparent)]
pub struct TextureData {
    raw: sys::ImTextureData,
}

impl TextureData {
    /// Create a new empty texture data
    ///
    /// This creates a new TextureData instance with default values.
    /// The texture will be in Destroyed status and needs to be created with `create()`.
    pub fn new() -> Box<Self> {
        let raw_data = Box::new(unsafe {
            let mut data: sys::ImTextureData = std::mem::zeroed();
            data.Status = sys::ImTextureStatus_Destroyed;
            data.TexID = 0; // ImTextureID_Invalid is defined as 0
            data
        });

        // Convert to TextureData
        unsafe { Box::from_raw(Box::into_raw(raw_data) as *mut Self) }
    }

    /// Create a new texture data from raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a valid
    /// ImTextureData structure.
    pub unsafe fn from_raw(raw: *mut sys::ImTextureData) -> &'static mut Self {
        &mut *(raw as *mut Self)
    }

    /// Get the raw pointer to the underlying ImTextureData
    pub fn as_raw(&self) -> *const sys::ImTextureData {
        &self.raw as *const _
    }

    /// Get the raw mutable pointer to the underlying ImTextureData
    pub fn as_raw_mut(&mut self) -> *mut sys::ImTextureData {
        &mut self.raw as *mut _
    }

    /// Get the unique ID of this texture (for debugging)
    pub fn unique_id(&self) -> i32 {
        self.raw.UniqueID
    }

    /// Get the current status of this texture
    pub fn status(&self) -> TextureStatus {
        TextureStatus::from(self.raw.Status)
    }

    /// Set the status of this texture
    ///
    /// This should only be called by renderer backends after handling a request.
    pub fn set_status(&mut self, status: TextureStatus) {
        self.raw.Status = status.into();
    }

    /// Get the backend user data
    pub fn backend_user_data(&self) -> *mut c_void {
        self.raw.BackendUserData
    }

    /// Set the backend user data
    pub fn set_backend_user_data(&mut self, data: *mut c_void) {
        self.raw.BackendUserData = data;
    }

    /// Get the texture ID
    pub fn tex_id(&self) -> TextureId {
        TextureId::from(self.raw.TexID as usize)
    }

    /// Set the texture ID
    ///
    /// This should only be called by renderer backends after creating or destroying the texture.
    pub fn set_tex_id(&mut self, tex_id: TextureId) {
        self.raw.TexID = tex_id.id() as sys::ImTextureID;
    }

    /// Get the texture format
    pub fn format(&self) -> TextureFormat {
        TextureFormat::from(self.raw.Format)
    }

    /// Get the texture width
    pub fn width(&self) -> i32 {
        self.raw.Width
    }

    /// Get the texture height
    pub fn height(&self) -> i32 {
        self.raw.Height
    }

    /// Get the bytes per pixel
    pub fn bytes_per_pixel(&self) -> i32 {
        self.raw.BytesPerPixel
    }

    /// Get the number of unused frames
    pub fn unused_frames(&self) -> i32 {
        self.raw.UnusedFrames
    }

    /// Get the reference count
    pub fn ref_count(&self) -> u16 {
        self.raw.RefCount
    }

    /// Check if the texture uses colors (rather than just white + alpha)
    pub fn use_colors(&self) -> bool {
        self.raw.UseColors
    }

    /// Check if the texture is queued for destruction next frame
    pub fn want_destroy_next_frame(&self) -> bool {
        self.raw.WantDestroyNextFrame
    }

    /// Get the pixel data
    ///
    /// Returns None if no pixel data is available.
    pub fn pixels(&self) -> Option<&[u8]> {
        if self.raw.Pixels.is_null() {
            None
        } else {
            let size = (self.width() * self.height() * self.bytes_per_pixel()) as usize;
            unsafe {
                Some(std::slice::from_raw_parts(
                    self.raw.Pixels as *const u8,
                    size,
                ))
            }
        }
    }

    /// Get the pixel data at a specific position
    ///
    /// Returns None if no pixel data is available or coordinates are out of bounds.
    pub fn pixels_at(&self, x: i32, y: i32) -> Option<&[u8]> {
        if self.raw.Pixels.is_null() || x < 0 || y < 0 || x >= self.width() || y >= self.height() {
            None
        } else {
            let offset = (x + y * self.width()) * self.bytes_per_pixel();
            let remaining_size = ((self.width() - x) + (self.height() - y - 1) * self.width())
                * self.bytes_per_pixel();
            unsafe {
                let ptr = (self.raw.Pixels as *const u8).add(offset as usize);
                Some(std::slice::from_raw_parts(ptr, remaining_size as usize))
            }
        }
    }

    /// Get the pitch (bytes per row)
    pub fn pitch(&self) -> i32 {
        self.width() * self.bytes_per_pixel()
    }

    /// Create a new texture with the specified format and dimensions
    ///
    /// This allocates pixel data and sets the status to WantCreate.
    pub fn create(&mut self, format: TextureFormat, width: i32, height: i32) {
        unsafe {
            sys::ImTextureData_Create(self.as_raw_mut(), format.into(), width, height);
        }
    }

    /// Destroy the pixel data
    ///
    /// This frees the CPU-side pixel data but doesn't affect the GPU texture.
    pub fn destroy_pixels(&mut self) {
        unsafe {
            sys::ImTextureData_DestroyPixels(self.as_raw_mut());
        }
    }

    /// Set the pixel data for the texture
    ///
    /// This copies the provided data into the texture's pixel buffer.
    /// Note: This is a simplified implementation. In practice, you should
    /// use the Dear ImGui texture management system properly.
    pub fn set_data(&mut self, data: &[u8]) {
        unsafe {
            let raw = self.as_raw_mut();
            if !(*raw).Pixels.is_null() {
                // Free existing data
                sys::ImTextureData_DestroyPixels(self.as_raw_mut());
            }

            // For now, we'll just set the pointer to the data
            // In a real implementation, you'd want to allocate and copy
            (*raw).Pixels = data.as_ptr() as *mut u8;
        }
    }

    /// Set the width of the texture
    pub fn set_width(&mut self, width: u32) {
        unsafe {
            (*self.as_raw_mut()).Width = width as i32;
        }
    }

    /// Set the height of the texture
    pub fn set_height(&mut self, height: u32) {
        unsafe {
            (*self.as_raw_mut()).Height = height as i32;
        }
    }

    /// Set the format of the texture
    pub fn set_format(&mut self, format: TextureFormat) {
        unsafe {
            (*self.as_raw_mut()).Format = format.into();
        }
    }
}

/// Get the number of bytes per pixel for a texture format
pub fn get_format_bytes_per_pixel(format: TextureFormat) -> i32 {
    unsafe { sys::ImTextureDataGetFormatBytesPerPixel(format.into()) }
}

/// Create an ImTextureRef from a texture ID
///
/// This is the safe way to create an ImTextureRef for use with Dear ImGui.
/// Use this instead of directly constructing the sys::ImTextureRef structure.
pub fn create_texture_ref(texture_id: u64) -> sys::ImTextureRef {
    sys::ImTextureRef {
        _TexData: std::ptr::null_mut(),
        _TexID: texture_id,
    }
}

/// Get the name of a texture status (for debugging)
pub fn get_status_name(status: TextureStatus) -> &'static str {
    unsafe {
        let ptr = sys::ImTextureDataGetStatusName(status.into());
        if ptr.is_null() {
            "Unknown"
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("Invalid")
        }
    }
}

/// Get the name of a texture format (for debugging)
pub fn get_format_name(format: TextureFormat) -> &'static str {
    unsafe {
        let ptr = sys::ImTextureDataGetFormatName(format.into());
        if ptr.is_null() {
            "Unknown"
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("Invalid")
        }
    }
}
