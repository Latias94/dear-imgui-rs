use dear_imgui_cte_sys as sys;
use std::ffi::c_char;

const _: () = assert!(std::mem::size_of::<sys::ImWchar>() == 4);
const _: () = assert!(std::mem::size_of::<sys::DocPos_c>() == 2 * std::mem::size_of::<usize>());

#[allow(dead_code)]
fn representative_upstream_and_bridge_symbols_are_generated() {
    let _: unsafe extern "C" fn() -> *mut sys::TextEditor = sys::TextEditor_TextEditor;
    let _: unsafe extern "C" fn(*mut sys::TextEditor) = sys::TextEditor_destroy;
    let _: unsafe extern "C" fn() -> *mut sys::TextDiff = sys::TextDiff_TextDiff;
    let _: unsafe extern "C" fn() -> *mut sys::TrieAutoComplete =
        sys::TrieAutoComplete_TrieAutoComplete;
    let _: unsafe extern "C" fn() -> *mut sys::Notifications = sys::Notifications_Notifications;
    let _: unsafe extern "C" fn(*mut c_char, sys::ImWchar) -> usize = sys::CodePoint_write;
    let _ = sys::TextEditor_GetText_alloc;
    let _ = sys::TextEditor_GetText_free;
    let _ = sys::TextEditor_Render;
    let _ = sys::TextDiff_destroy;
    let _ = sys::TextDiff_Render;
    let _ = sys::DocPos_DocPos_Nil;
    let _ = sys::DocSelection_DocSelection_Nil;
    let _ = sys::VisPos_VisPos_Nil;
    let _ = sys::Glyph_Glyph_Nil;
    let _ = sys::Iterator_Iterator_Nil;
    let _ = sys::Language_Cpp;
    let _ = sys::Palette_Palette;
    let _ = sys::Palette_destroy;
    let _ = sys::TrieAutoComplete_destroy;
    let _ = sys::Notifications_destroy;
    let _ = sys::GetDejavu;
    let _ = sys::SetDejavu;
    let _ = sys::dear_imgui_cte_set_change_callback;
    let _ = sys::dear_imgui_cte_set_transaction_callback;
    let _ = sys::dear_imgui_cte_set_insert_callback;
    let _ = sys::dear_imgui_cte_set_delete_callback;
    let _ = sys::dear_imgui_cte_iterate_line_data;
    let _ = sys::dear_imgui_cte_set_line_decorator;
    let _ = sys::dear_imgui_cte_set_custom_caret_callback;
    let _ = sys::dear_imgui_cte_set_line_number_context_callback;
    let _ = sys::dear_imgui_cte_set_text_context_callback;
    let _ = sys::dear_imgui_cte_set_text_hover_callback;
    let _ = sys::dear_imgui_cte_set_language_change_callback;
    let _ = sys::dear_imgui_cte_iterate_identifiers;
    let _ = sys::dear_imgui_cte_filter_selections;
    let _ = sys::dear_imgui_cte_filter_lines;
    let _: unsafe extern "C" fn(*mut sys::TextEditor) -> sys::DearImGuiCteStatus =
        sys::dear_imgui_cte_clear_callbacks;
    let _: unsafe extern "C" fn() -> *mut sys::DearImGuiCteAutocompleteConfig =
        sys::dear_imgui_cte_autocomplete_config_create;
    let _ = sys::dear_imgui_cte_autocomplete_config_destroy;
    let _ = sys::dear_imgui_cte_autocomplete_config_set_callback;
    let _ = sys::dear_imgui_cte_text_editor_set_autocomplete_config;
    let _ = sys::dear_imgui_cte_text_editor_set_autocomplete_suggestions;
    let _ = sys::dear_imgui_cte_autocomplete_state_get_search_term;
    let _ = sys::dear_imgui_cte_autocomplete_state_get_range;
    let _ = sys::dear_imgui_cte_autocomplete_state_get_context;
    let _ = sys::dear_imgui_cte_autocomplete_state_clear_suggestions;
    let _ = sys::dear_imgui_cte_autocomplete_state_add_suggestion;
    let _ = sys::dear_imgui_cte_autocomplete_state_set_promise;
}
