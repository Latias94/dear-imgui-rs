#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() -> *mut dear_implot3d_sys::ImPlot3DContext =
        dear_implot3d_sys::ImPlot3D_CreateContext;

    std::hint::black_box(symbol);
}
