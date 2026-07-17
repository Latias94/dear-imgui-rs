use dear_implot3d_sys as sys;

#[allow(dead_code)]
fn compatibility_symbol_signatures_are_generated() {
    let _: unsafe extern "C" fn(f64, f64, f64) -> sys::ImVec2_c = sys::ImPlot3D_PlotToPixels_double;
    let _: unsafe extern "C" fn() -> sys::ImVec2_c = sys::ImPlot3D_GetPlotRectPos;
    let _: unsafe extern "C" fn() -> sys::ImVec2_c = sys::ImPlot3D_GetPlotRectSize;
    let _: unsafe extern "C" fn() -> sys::ImVec4_c = sys::ImPlot3D_NextColormapColor;
    let _: unsafe extern "C" fn(i32, sys::ImPlot3DColormap) -> sys::ImVec4_c =
        sys::ImPlot3D_GetColormapColor;
}
