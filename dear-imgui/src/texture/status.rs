use crate::sys;

/// Status of a texture to communicate with Renderer Backend
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureStatus {
    /// Texture is ready and can be used
    OK = sys::ImTextureStatus_OK as i32,
    /// Backend destroyed the texture
    Destroyed = sys::ImTextureStatus_Destroyed as i32,
    /// Requesting backend to create the texture. Set status OK when done.
    WantCreate = sys::ImTextureStatus_WantCreate as i32,
    /// Requesting backend to update specific blocks of pixels. Set status OK when done.
    WantUpdates = sys::ImTextureStatus_WantUpdates as i32,
    /// Requesting backend to destroy the texture. Set status to Destroyed when done.
    WantDestroy = sys::ImTextureStatus_WantDestroy as i32,
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

/// Get the name of a texture status (for debugging)
pub fn get_status_name(status: TextureStatus) -> &'static str {
    unsafe {
        let ptr = sys::igImTextureDataGetStatusName(status.into());
        if ptr.is_null() {
            "Unknown"
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("Invalid")
        }
    }
}
