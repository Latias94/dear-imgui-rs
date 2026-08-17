#![cfg(not(target_arch = "wasm32"))]

use dear_imgui_cte_sys as sys;
use std::ffi::{CStr, CString, c_char, c_void};

struct FilterState {
    calls: usize,
    output: Vec<u8>,
    fail: bool,
}

unsafe extern "C" fn append_marker(
    userdata: *mut c_void,
    input: *const c_char,
    input_len: usize,
    output: *mut *const c_char,
    output_len: *mut usize,
) -> sys::DearImGuiCteStatus {
    let state = unsafe { &mut *userdata.cast::<FilterState>() };
    state.calls += 1;
    if state.fail {
        return sys::DearImGuiCteStatus_CallbackFailed;
    }

    let input = unsafe { std::slice::from_raw_parts(input.cast::<u8>(), input_len) };
    state.output.clear();
    state.output.extend_from_slice(input);
    state.output.push(b'!');
    unsafe {
        *output = state.output.as_ptr().cast::<c_char>();
        *output_len = state.output.len();
    }
    sys::DearImGuiCteStatus_Ok
}

#[test]
fn bridge_owns_cpp_callbacks_and_preserves_length_aware_text() {
    unsafe {
        assert_eq!(
            sys::dear_imgui_cte_clear_callbacks(std::ptr::null_mut()),
            sys::DearImGuiCteStatus_NullArgument
        );
        assert_eq!(
            sys::dear_imgui_cte_text_editor_reset_autocomplete(std::ptr::null_mut()),
            sys::DearImGuiCteStatus_NullArgument
        );
        assert_eq!(
            sys::dear_imgui_cte_text_editor_clear_callbacks(std::ptr::null_mut()),
            sys::DearImGuiCteStatus_NullArgument
        );
        let mut context = std::mem::MaybeUninit::uninit();
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_state_get_context(
                std::ptr::null(),
                context.as_mut_ptr(),
            ),
            sys::DearImGuiCteStatus_NullArgument
        );

        let editor = sys::dear_imgui_cte_text_editor_create();
        assert!(!editor.is_null());
        let source = CString::new("alpha\nbeta").unwrap();
        sys::TextEditor_SetText(editor, source.as_ptr());

        let mut state = FilterState {
            calls: 0,
            output: Vec::new(),
            fail: false,
        };
        assert_eq!(
            sys::dear_imgui_cte_filter_lines(
                editor,
                Some(append_marker),
                (&mut state as *mut FilterState).cast(),
            ),
            sys::DearImGuiCteStatus_Ok
        );
        assert_eq!(state.calls, 2);

        let text = sys::TextEditor_GetText_alloc(editor);
        assert!(!text.is_null());
        assert_eq!(CStr::from_ptr(text).to_bytes(), b"alpha!\nbeta!");
        sys::TextEditor_GetText_free(text);

        state.fail = true;
        assert_eq!(
            sys::dear_imgui_cte_filter_lines(
                editor,
                Some(append_marker),
                (&mut state as *mut FilterState).cast(),
            ),
            sys::DearImGuiCteStatus_CallbackFailed
        );

        let config = sys::dear_imgui_cte_autocomplete_config_create();
        assert!(!config.is_null());
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_config_set_suggestion_width(config, 0),
            sys::DearImGuiCteStatus_Ok
        );
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_config_set_trigger_delay(config, 86_400_000),
            sys::DearImGuiCteStatus_Ok
        );
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_config_set_trigger_delay(config, 86_400_001),
            sys::DearImGuiCteStatus_Ok
        );
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_config_set_trigger_delay(config, i64::MAX as u64),
            sys::DearImGuiCteStatus_Ok
        );
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_config_set_trigger_delay(config, i64::MAX as u64 + 1),
            sys::DearImGuiCteStatus_InvalidValue
        );
        assert_eq!(
            sys::dear_imgui_cte_autocomplete_config_set_no_suggestions_label(
                config,
                std::ptr::null(),
                1,
            ),
            sys::DearImGuiCteStatus_NullArgument
        );
        assert_eq!(
            sys::dear_imgui_cte_text_editor_set_autocomplete_config(editor, config),
            sys::DearImGuiCteStatus_Ok
        );
        assert_eq!(
            sys::dear_imgui_cte_text_editor_reset_autocomplete(editor),
            sys::DearImGuiCteStatus_Ok
        );
        sys::dear_imgui_cte_autocomplete_config_destroy(config);

        assert_eq!(
            sys::dear_imgui_cte_text_editor_clear_callbacks(editor),
            sys::DearImGuiCteStatus_Ok
        );
        sys::dear_imgui_cte_text_editor_destroy(editor);
    }
}
