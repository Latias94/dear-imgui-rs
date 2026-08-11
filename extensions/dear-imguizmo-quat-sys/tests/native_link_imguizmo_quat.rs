#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() -> f32 = dear_imguizmo_quat_sys::imguiGizmo_getDollyScale;

    std::hint::black_box(symbol);
}
