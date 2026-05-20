use crate::sys;

/// Plot location for legends, labels, etc.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotLocation {
    Center = sys::ImPlotLocation_Center as u32,
    North = sys::ImPlotLocation_North as u32,
    South = sys::ImPlotLocation_South as u32,
    West = sys::ImPlotLocation_West as u32,
    East = sys::ImPlotLocation_East as u32,
    NorthWest = sys::ImPlotLocation_NorthWest as u32,
    NorthEast = sys::ImPlotLocation_NorthEast as u32,
    SouthWest = sys::ImPlotLocation_SouthWest as u32,
    SouthEast = sys::ImPlotLocation_SouthEast as u32,
}

/// Plot orientation
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotOrientation {
    Horizontal = 0,
    Vertical = 1,
}

/// Plot condition (setup/next) matching ImPlotCond (ImGuiCond)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PlotCond {
    None = 0,
    Always = 1,
    Once = 2,
}
