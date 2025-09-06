use crate::sys;

/// 2D vector compatible with mint
pub type Vector2 = [f32; 2];

/// 3D vector compatible with mint
pub type Vector3 = [f32; 3];

/// 4D vector compatible with mint
pub type Vector4 = [f32; 4];

/// Mint-compatible 2D vector type
pub(crate) type MintVec2 = mint::Vector2<f32>;

/// Mint-compatible 3D vector type
pub(crate) type MintVec3 = mint::Vector3<f32>;

/// Mint-compatible 4D vector type
pub(crate) type MintVec4 = mint::Vector4<f32>;

/// Mint-compatible 2D integer vector type
pub(crate) type MintIVec2 = mint::Vector2<i32>;

/// Mint-compatible 3D integer vector type
pub(crate) type MintIVec3 = mint::Vector3<i32>;

/// Mint-compatible 4D integer vector type
pub(crate) type MintIVec4 = mint::Vector4<i32>;

/// RGBA color (4 floats, 0.0-1.0 range)
pub type Color = [f32; 4];

/// RGB color (3 floats, 0.0-1.0 range)
pub type Color3 = [f32; 3];

/// 32-bit RGBA color (0-255 range)
pub type ColorU32 = u32;

/// Condition for setting window/widget properties
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Condition {
    /// No condition (always set the variable)
    Always = sys::ImGuiCond_Always,
    /// Set the variable once per runtime session (only the first call will succeed)
    Once = sys::ImGuiCond_Once,
    /// Set the variable if the object/window has no persistently saved data (no entry in .ini file)
    FirstUseEver = sys::ImGuiCond_FirstUseEver,
    /// Set the variable if the object/window is appearing after being hidden/inactive (or the first time)
    Appearing = sys::ImGuiCond_Appearing,
}

/// Utility functions for color conversion
pub fn color_to_u32(color: Color) -> ColorU32 {
    let r = (color[0] * 255.0) as u32;
    let g = (color[1] * 255.0) as u32;
    let b = (color[2] * 255.0) as u32;
    let a = (color[3] * 255.0) as u32;
    (a << 24) | (b << 16) | (g << 8) | r
}

pub fn color_from_u32(color: ColorU32) -> Color {
    [
        ((color & 0xFF) as f32) / 255.0,
        (((color >> 8) & 0xFF) as f32) / 255.0,
        (((color >> 16) & 0xFF) as f32) / 255.0,
        (((color >> 24) & 0xFF) as f32) / 255.0,
    ]
}

pub fn color3_to_color(color: Color3) -> Color {
    [color[0], color[1], color[2], 1.0]
}

/// Common color constants
pub mod colors {
    use super::Color;

    pub const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
    pub const BLACK: Color = [0.0, 0.0, 0.0, 1.0];
    pub const RED: Color = [1.0, 0.0, 0.0, 1.0];
    pub const GREEN: Color = [0.0, 1.0, 0.0, 1.0];
    pub const BLUE: Color = [0.0, 0.0, 1.0, 1.0];
    pub const YELLOW: Color = [1.0, 1.0, 0.0, 1.0];
    pub const CYAN: Color = [0.0, 1.0, 1.0, 1.0];
    pub const MAGENTA: Color = [1.0, 0.0, 1.0, 1.0];
    pub const TRANSPARENT: Color = [0.0, 0.0, 0.0, 0.0];
}

/// Utility functions for vector operations
pub fn vec2(x: f32, y: f32) -> Vector2 {
    [x, y]
}

pub fn vec3(x: f32, y: f32, z: f32) -> Vector3 {
    [x, y, z]
}

pub fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vector4 {
    [x, y, z, w]
}

pub fn color(r: f32, g: f32, b: f32, a: f32) -> Color {
    [r, g, b, a]
}

pub fn color3(r: f32, g: f32, b: f32) -> Color3 {
    [r, g, b]
}
