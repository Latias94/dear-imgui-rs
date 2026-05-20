use crate::sys;

/// Markers for plot points
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Marker {
    Auto = sys::ImPlotMarker_Auto as i32,
    None = sys::ImPlotMarker_None as i32,
    Circle = sys::ImPlotMarker_Circle as i32,
    Square = sys::ImPlotMarker_Square as i32,
    Diamond = sys::ImPlotMarker_Diamond as i32,
    Up = sys::ImPlotMarker_Up as i32,
    Down = sys::ImPlotMarker_Down as i32,
    Left = sys::ImPlotMarker_Left as i32,
    Right = sys::ImPlotMarker_Right as i32,
    Cross = sys::ImPlotMarker_Cross as i32,
    Plus = sys::ImPlotMarker_Plus as i32,
    Asterisk = sys::ImPlotMarker_Asterisk as i32,
}
