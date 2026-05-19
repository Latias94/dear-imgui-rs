use crate::sys;

pub(super) fn texture_format_bytes_per_pixel(format: TextureFormat) -> usize {
    match format {
        TextureFormat::RGBA32 => 4,
        TextureFormat::Alpha8 => 1,
    }
}

pub(super) fn texture_format_bytes_per_pixel_i32(format: TextureFormat) -> i32 {
    i32::try_from(texture_format_bytes_per_pixel(format))
        .expect("texture format bytes per pixel exceeded i32 range")
}

/// Texture format supported by Dear ImGui
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureFormat {
    /// 4 components per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight * 4
    RGBA32 = sys::ImTextureFormat_RGBA32 as i32,
    /// 1 component per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight
    Alpha8 = sys::ImTextureFormat_Alpha8 as i32,
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

/// Get the number of bytes per pixel for a texture format
pub fn get_format_bytes_per_pixel(format: TextureFormat) -> usize {
    texture_format_bytes_per_pixel(format)
}

/// Get the name of a texture format (for debugging)
pub fn get_format_name(format: TextureFormat) -> &'static str {
    unsafe {
        let ptr = sys::igImTextureDataGetFormatName(format.into());
        if ptr.is_null() {
            "Unknown"
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("Invalid")
        }
    }
}
