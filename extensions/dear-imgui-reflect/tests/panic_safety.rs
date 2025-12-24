use dear_imgui_reflect as reflect;

mod common;

use common::test_guard;

#[test]
fn settings_scope_restores_on_panic() {
    let _guard = test_guard();

    use reflect::NumericWidgetKind;

    // Establish a baseline.
    reflect::with_settings(|s| {
        s.numerics_i32_mut().widget = NumericWidgetKind::Input;
    });
    let before = reflect::current_settings().numerics_i32().widget;

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reflect::with_settings_scope(|| {
            reflect::with_settings(|s| {
                s.numerics_i32_mut().widget = NumericWidgetKind::Slider;
            });
            panic!("boom");
        });
    }));

    let after = reflect::current_settings().numerics_i32().widget;
    assert_eq!(
        std::mem::discriminant(&before),
        std::mem::discriminant(&after)
    );
}
