#[test]
fn native_symbol_is_linked() {
    let symbol: unsafe extern "C" fn() = dear_imguizmo_sys::ImGuizmo_BeginFrame;

    std::hint::black_box(symbol);
    std::hint::black_box(dear_imguizmo_sys::ImGuizmo_ResetInteraction as unsafe extern "C" fn());
}
