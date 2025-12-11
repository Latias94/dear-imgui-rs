use dear_imgui_reflect as reflect;

#[test]
fn member_numeric_helpers_set_expected_presets_for_f32() {
    use reflect::{MemberSettings, NumericRange, NumericWidgetKind};

    let mut member = MemberSettings::default();

    // Slider [0, 1]
    member.numerics_f32_slider_0_to_1(3);
    let numeric = member
        .numerics_f32
        .clone()
        .expect("expected numerics_f32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%.3f"));

    // Slider [-1, 1]
    member.numerics_f32_slider_minus1_to_1(2);
    let numeric = member
        .numerics_f32
        .clone()
        .expect("expected numerics_f32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min + 1.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%.2f"));

    // Drag with speed
    member.numerics_f32_drag_with_speed(0.05, 4);
    let numeric = member
        .numerics_f32
        .clone()
        .expect("expected numerics_f32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Drag));
    assert!(matches!(numeric.range, NumericRange::None));
    assert_eq!(numeric.speed, Some(0.05));
    assert_eq!(numeric.format.as_deref(), Some("%.4f"));

    // Percentage slider [0, 1]
    member.numerics_f32_percentage_slider_0_to_1(1);
    let numeric = member
        .numerics_f32
        .clone()
        .expect("expected numerics_f32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%.1f%%"));
}

#[test]
fn member_numeric_helpers_set_expected_presets_for_f64() {
    use reflect::{MemberSettings, NumericRange, NumericWidgetKind};

    let mut member = MemberSettings::default();

    // Slider [0, 1]
    member.numerics_f64_slider_0_to_1(3);
    let numeric = member
        .numerics_f64
        .clone()
        .expect("expected numerics_f64 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%.3f"));

    // Slider [-1, 1]
    member.numerics_f64_slider_minus1_to_1(2);
    let numeric = member
        .numerics_f64
        .clone()
        .expect("expected numerics_f64 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min + 1.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%.2f"));

    // Drag with speed
    member.numerics_f64_drag_with_speed(0.05, 4);
    let numeric = member
        .numerics_f64
        .clone()
        .expect("expected numerics_f64 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Drag));
    assert!(matches!(numeric.range, NumericRange::None));
    assert_eq!(numeric.speed, Some(0.05));
    assert_eq!(numeric.format.as_deref(), Some("%.4f"));

    // Percentage slider [0, 1]
    member.numerics_f64_percentage_slider_0_to_1(1);
    let numeric = member
        .numerics_f64
        .clone()
        .expect("expected numerics_f64 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%.1f%%"));
}

#[test]
fn member_numeric_helpers_set_expected_presets_for_i32_and_u32() {
    use reflect::{MemberSettings, NumericRange, NumericWidgetKind};

    // i32 decimal input with steps
    let mut member = MemberSettings::default();
    member.numerics_i32_input_decimal(1, 10);
    let numeric = member
        .numerics_i32
        .clone()
        .expect("expected numerics_i32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Input));
    assert_eq!(numeric.step, Some(1.0));
    assert_eq!(numeric.step_fast, Some(10.0));
    assert_eq!(numeric.format.as_deref(), Some("%d"));

    // i32 hex input
    member.numerics_i32_input_hex();
    let numeric = member
        .numerics_i32
        .clone()
        .expect("expected numerics_i32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Input));
    assert_eq!(numeric.format.as_deref(), Some("%x"));

    // i32 slider range
    member.numerics_i32_slider_range(-5, 5, true);
    let numeric = member
        .numerics_i32
        .clone()
        .expect("expected numerics_i32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min + 5.0).abs() < f64::EPSILON && (max - 5.0).abs() < f64::EPSILON
    ));
    assert!(numeric.clamp);
    assert!(numeric.always_clamp);
    assert_eq!(numeric.format.as_deref(), Some("%d"));

    // i32 slider 0..100
    member.numerics_i32_slider_0_to_100();
    let numeric = member
        .numerics_i32
        .clone()
        .expect("expected numerics_i32 to be set");
    assert!(matches!(numeric.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 100.0).abs() < f64::EPSILON
    ));

    // u32 decimal input with steps
    let mut member_u = MemberSettings::default();
    member_u.numerics_u32_input_decimal(1, 10);
    let numeric_u = member_u
        .numerics_u32
        .clone()
        .expect("expected numerics_u32 to be set");
    assert!(matches!(numeric_u.widget, NumericWidgetKind::Input));
    assert_eq!(numeric_u.step, Some(1.0));
    assert_eq!(numeric_u.step_fast, Some(10.0));
    assert_eq!(numeric_u.format.as_deref(), Some("%u"));

    // u32 hex input
    member_u.numerics_u32_input_hex();
    let numeric_u = member_u
        .numerics_u32
        .clone()
        .expect("expected numerics_u32 to be set");
    assert!(matches!(numeric_u.widget, NumericWidgetKind::Input));
    assert_eq!(numeric_u.format.as_deref(), Some("%x"));

    // u32 slider range
    member_u.numerics_u32_slider_range(0, 50, true);
    let numeric_u = member_u
        .numerics_u32
        .clone()
        .expect("expected numerics_u32 to be set");
    assert!(matches!(numeric_u.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric_u.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 50.0).abs() < f64::EPSILON
    ));
    assert!(numeric_u.clamp);
    assert!(numeric_u.always_clamp);
    assert_eq!(numeric_u.format.as_deref(), Some("%u"));

    // u32 slider 0..100
    member_u.numerics_u32_slider_0_to_100();
    let numeric_u = member_u
        .numerics_u32
        .clone()
        .expect("expected numerics_u32 to be set");
    assert!(matches!(numeric_u.widget, NumericWidgetKind::Slider));
    assert!(matches!(
        numeric_u.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 100.0).abs() < f64::EPSILON
    ));
}
