use dear_imgui_reflect as reflect;

#[test]
fn settings_scope_restores_global_defaults() {
    use reflect::{NumericRange, NumericTypeSettings, NumericWidgetKind};

    // Establish a known global baseline for i32 numeric settings.
    reflect::with_settings(|s| {
        *s.numerics_i32_mut() = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit {
                min: 0.0,
                max: 100.0,
            },
            ..NumericTypeSettings::default()
        };
    });

    let before = reflect::current_settings();
    assert!(matches!(
        before.numerics_i32().widget,
        NumericWidgetKind::Slider
    ));

    // Within the scope, override the widget kind; after the scope, the
    // previous global value must be restored.
    reflect::with_settings_scope(|| {
        reflect::with_settings(|s| {
            s.numerics_i32_mut().widget = NumericWidgetKind::Drag;
        });

        let inner = reflect::current_settings();
        assert!(matches!(
            inner.numerics_i32().widget,
            NumericWidgetKind::Drag
        ));
    });

    let after = reflect::current_settings();
    assert!(matches!(
        after.numerics_i32().widget,
        NumericWidgetKind::Slider
    ));
}
