use crate::sys;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Mouse button identifier
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseButton {
    /// Left mouse button
    Left = sys::ImGuiMouseButton_Left as i32,
    /// Right mouse button
    Right = sys::ImGuiMouseButton_Right as i32,
    /// Middle mouse button
    Middle = sys::ImGuiMouseButton_Middle as i32,
    /// Extra mouse button 1 (typically Back)
    Extra1 = 3,
    /// Extra mouse button 2 (typically Forward)
    Extra2 = 4,
}

/// Mouse cursor types
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseCursor {
    /// No cursor
    None = sys::ImGuiMouseCursor_None as i32,
    /// Arrow cursor
    Arrow = sys::ImGuiMouseCursor_Arrow as i32,
    /// Text input I-beam cursor
    TextInput = sys::ImGuiMouseCursor_TextInput as i32,
    /// Resize all directions cursor
    ResizeAll = sys::ImGuiMouseCursor_ResizeAll as i32,
    /// Resize north-south cursor
    ResizeNS = sys::ImGuiMouseCursor_ResizeNS as i32,
    /// Resize east-west cursor
    ResizeEW = sys::ImGuiMouseCursor_ResizeEW as i32,
    /// Resize northeast-southwest cursor
    ResizeNESW = sys::ImGuiMouseCursor_ResizeNESW as i32,
    /// Resize northwest-southeast cursor
    ResizeNWSE = sys::ImGuiMouseCursor_ResizeNWSE as i32,
    /// Hand cursor
    Hand = sys::ImGuiMouseCursor_Hand as i32,
    /// Not allowed cursor
    NotAllowed = sys::ImGuiMouseCursor_NotAllowed as i32,
}

/// Source of mouse-like input events.
///
/// Backends can use this to mark whether a mouse event originates from a
/// physical mouse, a touch screen, or a pen/stylus so Dear ImGui can
/// correctly handle multiple input sources.
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseSource {
    /// Events coming from a physical mouse
    Mouse = sys::ImGuiMouseSource_Mouse as i32,
    /// Events coming from a touch screen
    TouchScreen = sys::ImGuiMouseSource_TouchScreen as i32,
    /// Events coming from a pen or stylus
    Pen = sys::ImGuiMouseSource_Pen as i32,
}

impl From<MouseButton> for sys::ImGuiMouseButton {
    #[inline]
    fn from(value: MouseButton) -> sys::ImGuiMouseButton {
        value as sys::ImGuiMouseButton
    }
}

impl From<MouseSource> for sys::ImGuiMouseSource {
    #[inline]
    fn from(value: MouseSource) -> sys::ImGuiMouseSource {
        value as sys::ImGuiMouseSource
    }
}
