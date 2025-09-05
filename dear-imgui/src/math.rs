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

/// Mouse cursor types
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MouseCursor {
    None = sys::ImGuiMouseCursor_None,
    Arrow = sys::ImGuiMouseCursor_Arrow,
    TextInput = sys::ImGuiMouseCursor_TextInput,
    ResizeAll = sys::ImGuiMouseCursor_ResizeAll,
    ResizeNS = sys::ImGuiMouseCursor_ResizeNS,
    ResizeEW = sys::ImGuiMouseCursor_ResizeEW,
    ResizeNESW = sys::ImGuiMouseCursor_ResizeNESW,
    ResizeNWSE = sys::ImGuiMouseCursor_ResizeNWSE,
    Hand = sys::ImGuiMouseCursor_Hand,
    NotAllowed = sys::ImGuiMouseCursor_NotAllowed,
}

/// Key indices for keyboard input
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Key {
    Tab = sys::ImGuiKey_Tab,
    LeftArrow = sys::ImGuiKey_LeftArrow,
    RightArrow = sys::ImGuiKey_RightArrow,
    UpArrow = sys::ImGuiKey_UpArrow,
    DownArrow = sys::ImGuiKey_DownArrow,
    PageUp = sys::ImGuiKey_PageUp,
    PageDown = sys::ImGuiKey_PageDown,
    Home = sys::ImGuiKey_Home,
    End = sys::ImGuiKey_End,
    Insert = sys::ImGuiKey_Insert,
    Delete = sys::ImGuiKey_Delete,
    Backspace = sys::ImGuiKey_Backspace,
    Space = sys::ImGuiKey_Space,
    Enter = sys::ImGuiKey_Enter,
    Escape = sys::ImGuiKey_Escape,
    LeftCtrl = sys::ImGuiKey_LeftCtrl,
    LeftShift = sys::ImGuiKey_LeftShift,
    LeftAlt = sys::ImGuiKey_LeftAlt,
    LeftSuper = sys::ImGuiKey_LeftSuper,
    RightCtrl = sys::ImGuiKey_RightCtrl,
    RightShift = sys::ImGuiKey_RightShift,
    RightAlt = sys::ImGuiKey_RightAlt,
    RightSuper = sys::ImGuiKey_RightSuper,
    Menu = sys::ImGuiKey_Menu,
    A = sys::ImGuiKey_A,
    B = sys::ImGuiKey_B,
    C = sys::ImGuiKey_C,
    D = sys::ImGuiKey_D,
    E = sys::ImGuiKey_E,
    F = sys::ImGuiKey_F,
    G = sys::ImGuiKey_G,
    H = sys::ImGuiKey_H,
    I = sys::ImGuiKey_I,
    J = sys::ImGuiKey_J,
    K = sys::ImGuiKey_K,
    L = sys::ImGuiKey_L,
    M = sys::ImGuiKey_M,
    N = sys::ImGuiKey_N,
    O = sys::ImGuiKey_O,
    P = sys::ImGuiKey_P,
    Q = sys::ImGuiKey_Q,
    R = sys::ImGuiKey_R,
    S = sys::ImGuiKey_S,
    T = sys::ImGuiKey_T,
    U = sys::ImGuiKey_U,
    V = sys::ImGuiKey_V,
    W = sys::ImGuiKey_W,
    X = sys::ImGuiKey_X,
    Y = sys::ImGuiKey_Y,
    Z = sys::ImGuiKey_Z,
    F1 = sys::ImGuiKey_F1,
    F2 = sys::ImGuiKey_F2,
    F3 = sys::ImGuiKey_F3,
    F4 = sys::ImGuiKey_F4,
    F5 = sys::ImGuiKey_F5,
    F6 = sys::ImGuiKey_F6,
    F7 = sys::ImGuiKey_F7,
    F8 = sys::ImGuiKey_F8,
    F9 = sys::ImGuiKey_F9,
    F10 = sys::ImGuiKey_F10,
    F11 = sys::ImGuiKey_F11,
    F12 = sys::ImGuiKey_F12,
    Apostrophe = sys::ImGuiKey_Apostrophe,
    Comma = sys::ImGuiKey_Comma,
    Minus = sys::ImGuiKey_Minus,
    Period = sys::ImGuiKey_Period,
    Slash = sys::ImGuiKey_Slash,
    Semicolon = sys::ImGuiKey_Semicolon,
    Equal = sys::ImGuiKey_Equal,
    LeftBracket = sys::ImGuiKey_LeftBracket,
    Backslash = sys::ImGuiKey_Backslash,
    RightBracket = sys::ImGuiKey_RightBracket,
    GraveAccent = sys::ImGuiKey_GraveAccent,
    CapsLock = sys::ImGuiKey_CapsLock,
    ScrollLock = sys::ImGuiKey_ScrollLock,
    NumLock = sys::ImGuiKey_NumLock,
    PrintScreen = sys::ImGuiKey_PrintScreen,
    Pause = sys::ImGuiKey_Pause,
    Keypad0 = sys::ImGuiKey_Keypad0,
    Keypad1 = sys::ImGuiKey_Keypad1,
    Keypad2 = sys::ImGuiKey_Keypad2,
    Keypad3 = sys::ImGuiKey_Keypad3,
    Keypad4 = sys::ImGuiKey_Keypad4,
    Keypad5 = sys::ImGuiKey_Keypad5,
    Keypad6 = sys::ImGuiKey_Keypad6,
    Keypad7 = sys::ImGuiKey_Keypad7,
    Keypad8 = sys::ImGuiKey_Keypad8,
    Keypad9 = sys::ImGuiKey_Keypad9,
    KeypadDecimal = sys::ImGuiKey_KeypadDecimal,
    KeypadDivide = sys::ImGuiKey_KeypadDivide,
    KeypadMultiply = sys::ImGuiKey_KeypadMultiply,
    KeypadSubtract = sys::ImGuiKey_KeypadSubtract,
    KeypadAdd = sys::ImGuiKey_KeypadAdd,
    KeypadEnter = sys::ImGuiKey_KeypadEnter,
    KeypadEqual = sys::ImGuiKey_KeypadEqual,
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
