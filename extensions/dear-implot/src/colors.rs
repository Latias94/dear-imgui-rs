use crate::sys;

/// Colorable plot elements
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotColorElement {
    FrameBg = sys::ImPlotCol_FrameBg as i32,
    PlotBg = sys::ImPlotCol_PlotBg as i32,
    PlotBorder = sys::ImPlotCol_PlotBorder as i32,
    LegendBg = sys::ImPlotCol_LegendBg as i32,
    LegendBorder = sys::ImPlotCol_LegendBorder as i32,
    LegendText = sys::ImPlotCol_LegendText as i32,
    TitleText = sys::ImPlotCol_TitleText as i32,
    InlayText = sys::ImPlotCol_InlayText as i32,
    AxisText = sys::ImPlotCol_AxisText as i32,
    AxisGrid = sys::ImPlotCol_AxisGrid as i32,
    AxisTick = sys::ImPlotCol_AxisTick as i32,
    AxisBg = sys::ImPlotCol_AxisBg as i32,
    AxisBgHovered = sys::ImPlotCol_AxisBgHovered as i32,
    AxisBgActive = sys::ImPlotCol_AxisBgActive as i32,
    Selection = sys::ImPlotCol_Selection as i32,
    Crosshairs = sys::ImPlotCol_Crosshairs as i32,
}
