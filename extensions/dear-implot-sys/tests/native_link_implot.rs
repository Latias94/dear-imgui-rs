#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() -> *mut dear_implot_sys::ImPlotContext =
        dear_implot_sys::ImPlot_CreateContext;

    std::hint::black_box(symbol);
}
