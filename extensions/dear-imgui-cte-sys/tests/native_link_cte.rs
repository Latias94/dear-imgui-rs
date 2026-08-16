#[test]
fn native_constructors_are_linked() {
    let editor: unsafe extern "C" fn() -> *mut dear_imgui_cte_sys::TextEditor =
        dear_imgui_cte_sys::TextEditor_TextEditor;
    let diff: unsafe extern "C" fn() -> *mut dear_imgui_cte_sys::TextDiff =
        dear_imgui_cte_sys::TextDiff_TextDiff;

    std::hint::black_box((editor, diff));
}
