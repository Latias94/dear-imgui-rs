#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() -> *mut dear_imnodes_sys::ImNodesContext =
        dear_imnodes_sys::imnodes_CreateContext;

    std::hint::black_box(symbol);
}
