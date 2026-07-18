use dear_imnodes_sys as sys;

#[allow(dead_code)]
fn compatibility_symbol_signatures_are_generated() {
    let _: unsafe extern "C" fn() -> sys::ImVec2_c = sys::imnodes_EditorContextGetPanning;
    let _: unsafe extern "C" fn() -> *mut bool = sys::imnodes_getIOKeyShiftPtr;
    let _: unsafe extern "C" fn() -> *mut bool = sys::imnodes_getIOKeyAltPtr;
    let _: unsafe extern "C" fn() = sys::imnodes_EditorContextResetToDefault;
    let _: unsafe extern "C" fn() -> *mut sys::ImNodesEditorContext =
        sys::imnodes_EditorContextGetCurrent;
    let _: unsafe extern "C" fn(*mut sys::ImNodesEditorContext) =
        sys::imnodes_EditorContextResetToDefaultIfCurrent;
    let _: unsafe extern "C" fn(i32) -> sys::ImVec2_c = sys::imnodes_GetNodeScreenSpacePos;
    let _: unsafe extern "C" fn(i32) -> sys::ImVec2_c = sys::imnodes_GetNodeEditorSpacePos;
    let _: unsafe extern "C" fn(i32) -> sys::ImVec2_c = sys::imnodes_GetNodeDimensions;
}
