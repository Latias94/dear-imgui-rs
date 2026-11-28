#[test]
fn native_constructors_are_linked() {
    let editor: unsafe extern "C" fn() -> *mut dear_imgui_cte_sys::TextEditor =
        dear_imgui_cte_sys::TextEditor_TextEditor;
    let bridge_editor: unsafe extern "C" fn() -> *mut dear_imgui_cte_sys::TextEditor =
        dear_imgui_cte_sys::dear_imgui_cte_text_editor_create;
    let diff: unsafe extern "C" fn() -> *mut dear_imgui_cte_sys::TextDiff =
        dear_imgui_cte_sys::TextDiff_TextDiff;

    std::hint::black_box((editor, bridge_editor, diff));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bridge_editor_creation_handles_an_allocator_refusal() {
    use dear_imgui_sys::{igGetAllocatorFunctions, igSetAllocatorFunctions};
    use std::{ffi::c_void, ptr};

    unsafe extern "C" fn reject_alloc(_size: usize, _userdata: *mut c_void) -> *mut c_void {
        ptr::null_mut()
    }

    unsafe extern "C" fn ignore_free(_allocation: *mut c_void, _userdata: *mut c_void) {}

    let mut allocate = None;
    let mut free = None;
    let mut userdata = ptr::null_mut();
    let editor = unsafe {
        igGetAllocatorFunctions(&mut allocate, &mut free, &mut userdata);
        igSetAllocatorFunctions(Some(reject_alloc), Some(ignore_free), ptr::null_mut());
        let editor = dear_imgui_cte_sys::dear_imgui_cte_text_editor_create();
        igSetAllocatorFunctions(allocate, free, userdata);
        editor
    };
    assert!(editor.is_null());
}
