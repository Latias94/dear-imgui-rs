#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() -> *mut std::ffi::c_void =
        dear_node_editor_sys::dne_get_current_editor_raw;

    std::hint::black_box(symbol);
}

#[test]
fn shim_rejects_invalid_config_and_style_inputs() {
    use dear_node_editor_sys as sys;
    use std::ptr;

    let invalid = sys::DneConfig {
        custom_zoom_level_count: 1,
        ..Default::default()
    };
    let color = sys::DNE_STYLE_COLOR_BG;
    let invalid_color = sys::DNE_STYLE_COLOR_COUNT;
    let components = |value: sys::ImVec4_c| [value.x, value.y, value.z, value.w];

    unsafe {
        assert!(sys::dne_create_editor(&invalid).is_null());
        assert!(sys::dne_get_current_editor_raw().is_null());
        assert_eq!(components(sys::dne_get_style_color(color)), [0.0; 4]);
        assert_eq!(sys::dne_get_style_node_rounding(), 0.0);
        assert_eq!(sys::dne_get_background_click_button_index(), -1);
        assert!(!sys::dne_begin_delete());

        let editor = sys::dne_create_editor(ptr::null());
        assert!(!editor.is_null());
        sys::dne_set_current_editor(editor);
        let original = components(sys::dne_get_style_color(color));
        assert_ne!(original, [0.0; 4]);
        assert_eq!(
            components(sys::dne_get_style_color(invalid_color)),
            [0.0; 4]
        );
        sys::dne_set_style_color(invalid_color, sys::ImVec4_c::default());
        sys::dne_push_style_color(invalid_color, sys::ImVec4_c::default());
        assert_eq!(components(sys::dne_get_style_color(color)), original);

        sys::dne_destroy_editor(editor);
        assert!(sys::dne_get_current_editor_raw().is_null());
    }
}
