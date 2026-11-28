use crate::sys;

/// Colorable plot elements
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotColorElement {
    FrameBg = sys::ImPlotCol_FrameBg as _,
    PlotBg = sys::ImPlotCol_PlotBg as _,
    PlotBorder = sys::ImPlotCol_PlotBorder as _,
    LegendBg = sys::ImPlotCol_LegendBg as _,
    LegendBorder = sys::ImPlotCol_LegendBorder as _,
    LegendText = sys::ImPlotCol_LegendText as _,
    TitleText = sys::ImPlotCol_TitleText as _,
    InlayText = sys::ImPlotCol_InlayText as _,
    AxisText = sys::ImPlotCol_AxisText as _,
    AxisGrid = sys::ImPlotCol_AxisGrid as _,
    AxisTick = sys::ImPlotCol_AxisTick as _,
    AxisBg = sys::ImPlotCol_AxisBg as _,
    AxisBgHovered = sys::ImPlotCol_AxisBgHovered as _,
    AxisBgActive = sys::ImPlotCol_AxisBgActive as _,
    Selection = sys::ImPlotCol_Selection as _,
    Crosshairs = sys::ImPlotCol_Crosshairs as _,
}
