use crate::sys;

/// Markers for plot points
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Marker {
    Auto = sys::ImPlotMarker_Auto,
    None = sys::ImPlotMarker_None,
    Circle = sys::ImPlotMarker_Circle,
    Square = sys::ImPlotMarker_Square,
    Diamond = sys::ImPlotMarker_Diamond,
    Up = sys::ImPlotMarker_Up,
    Down = sys::ImPlotMarker_Down,
    Left = sys::ImPlotMarker_Left,
    Right = sys::ImPlotMarker_Right,
    Cross = sys::ImPlotMarker_Cross,
    Plus = sys::ImPlotMarker_Plus,
    Asterisk = sys::ImPlotMarker_Asterisk,
    Vertical = sys::ImPlotMarker_Vertical,
    Horizontal = sys::ImPlotMarker_Horizontal,
}
