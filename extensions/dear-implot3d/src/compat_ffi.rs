#![allow(non_snake_case)]

use crate::{imgui_sys, sys};

unsafe extern "C" {
    pub fn ImPlot3D_PlotToPixels_double(x: f64, y: f64, z: f64) -> imgui_sys::ImVec2_c;
    pub fn ImPlot3D_GetPlotRectPos() -> imgui_sys::ImVec2_c;
    pub fn ImPlot3D_GetPlotRectSize() -> imgui_sys::ImVec2_c;
    pub fn ImPlot3D_NextColormapColor() -> imgui_sys::ImVec4_c;
    pub fn ImPlot3D_GetColormapColor(
        idx: ::std::os::raw::c_int,
        cmap: sys::ImPlot3DColormap,
    ) -> imgui_sys::ImVec4_c;
}
