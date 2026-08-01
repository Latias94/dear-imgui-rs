use dear_imgui_reflect as reflect;

mod common;

use common::test_guard;

#[test]
fn reflect_sessions_isolate_settings() {
    let _guard = test_guard();
    use reflect::{I32NumericSettings, NumericRange, NumericWidgetKind};

    let mut left = reflect::ReflectSession::new();
    let right = reflect::ReflectSession::new();

    {
        let s = left.settings_mut();
        *s.numerics_i32_mut() = I32NumericSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit {
                min: 0.0,
                max: 100.0,
            },
            ..I32NumericSettings::default()
        };
    }

    assert!(matches!(
        left.settings().numerics_i32().widget,
        NumericWidgetKind::Slider
    ));
    assert!(matches!(
        right.settings().numerics_i32().widget,
        NumericWidgetKind::Input
    ));
}
