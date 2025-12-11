use dear_imgui_reflect as reflect;

#[test]
fn numeric_type_settings_format_helpers_build_expected_strings() {
    use reflect::NumericTypeSettings;

    let decimal = NumericTypeSettings::default().with_decimal();
    assert_eq!(decimal.format.as_deref(), Some("%d"));

    let unsigned = NumericTypeSettings::default().with_unsigned();
    assert_eq!(unsigned.format.as_deref(), Some("%u"));

    let hex_lower = NumericTypeSettings::default().with_hex(false);
    assert_eq!(hex_lower.format.as_deref(), Some("%x"));

    let hex_upper = NumericTypeSettings::default().with_hex(true);
    assert_eq!(hex_upper.format.as_deref(), Some("%X"));

    let octal = NumericTypeSettings::default().with_octal();
    assert_eq!(octal.format.as_deref(), Some("%o"));

    let padded = NumericTypeSettings::default().with_int_padded(4, '0');
    assert_eq!(padded.format.as_deref(), Some("%04d"));

    let ch = NumericTypeSettings::default().with_char();
    assert_eq!(ch.format.as_deref(), Some("%c"));

    let float3 = NumericTypeSettings::default().with_float(3);
    assert_eq!(float3.format.as_deref(), Some("%.3f"));

    let double4 = NumericTypeSettings::default().with_double(4);
    assert_eq!(double4.format.as_deref(), Some("%.4lf"));

    let sci_lower = NumericTypeSettings::default().with_scientific(2, false);
    assert_eq!(sci_lower.format.as_deref(), Some("%.2e"));

    let sci_upper = NumericTypeSettings::default().with_scientific(5, true);
    assert_eq!(sci_upper.format.as_deref(), Some("%.5E"));

    let pct = NumericTypeSettings::default().with_percentage(1);
    assert_eq!(pct.format.as_deref(), Some("%.1f%%"));
}

#[test]
fn numeric_type_settings_presets_apply_expected_defaults() {
    use reflect::{NumericRange, NumericTypeSettings, NumericWidgetKind};

    // Slider [0, 1] with clamping and float format
    let slider01 = NumericTypeSettings::default().slider_0_to_1(3);
    assert!(matches!(
        slider01.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(slider01.clamp);
    assert!(slider01.always_clamp);
    assert!(matches!(slider01.widget, NumericWidgetKind::Slider));
    assert_eq!(slider01.format.as_deref(), Some("%.3f"));

    // Slider [-1, 1] with clamping and float format
    let slider_neg1_1 = NumericTypeSettings::default().slider_minus1_to_1(2);
    assert!(matches!(
        slider_neg1_1.range,
        NumericRange::Explicit { min, max }
        if (min + 1.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(slider_neg1_1.clamp);
    assert!(slider_neg1_1.always_clamp);
    assert!(matches!(slider_neg1_1.widget, NumericWidgetKind::Slider));
    assert_eq!(slider_neg1_1.format.as_deref(), Some("%.2f"));

    // Drag with explicit speed and float format
    let drag = NumericTypeSettings::default().drag_with_speed(0.01, 4);
    assert!(matches!(drag.range, NumericRange::None));
    assert_eq!(drag.speed, Some(0.01));
    assert!(matches!(drag.widget, NumericWidgetKind::Drag));
    assert_eq!(drag.format.as_deref(), Some("%.4f"));

    // Percentage slider [0, 1] with clamping and percentage format
    let pct_slider = NumericTypeSettings::default().percentage_slider_0_to_1(1);
    assert!(matches!(
        pct_slider.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(pct_slider.clamp);
    assert!(pct_slider.always_clamp);
    assert!(matches!(pct_slider.widget, NumericWidgetKind::Slider));
    assert_eq!(pct_slider.format.as_deref(), Some("%.1f%%"));
}
