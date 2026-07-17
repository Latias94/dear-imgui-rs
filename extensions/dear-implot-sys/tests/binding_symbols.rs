use dear_implot_sys as sys;
use std::os::raw::c_char;

#[allow(dead_code)]
fn compatibility_symbol_signatures_are_generated() {
    let _: unsafe extern "C" fn(f64, f64, sys::ImVec4_c, sys::ImVec2_c, bool, *const c_char) =
        sys::ImPlot_Annotation_Str0;
    let _: unsafe extern "C" fn(f64, sys::ImVec4_c, *const c_char) = sys::ImPlot_TagX_Str0;
    let _: unsafe extern "C" fn(f64, sys::ImVec4_c, *const c_char) = sys::ImPlot_TagY_Str0;
    let _: unsafe extern "C" fn() -> sys::ImVec2_c = sys::ImPlot_GetPlotPos;
    let _: unsafe extern "C" fn() -> sys::ImVec2_c = sys::ImPlot_GetPlotSize;
}
