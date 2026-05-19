use crate::sys;

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
