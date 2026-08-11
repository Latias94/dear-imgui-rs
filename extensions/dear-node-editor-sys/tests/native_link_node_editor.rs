#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() -> *mut std::ffi::c_void =
        dear_node_editor_sys::dne_get_current_editor_raw;

    std::hint::black_box(symbol);
}
