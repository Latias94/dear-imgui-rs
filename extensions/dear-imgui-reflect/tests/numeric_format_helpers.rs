use dear_imgui_reflect as reflect;

macro_rules! assert_format {
    ($settings:expr, $expected:expr) => {
        assert_eq!(
            $settings.format.as_ref().map(|format| format.as_str()),
            $expected
        )
    };
}

#[test]
fn numeric_type_settings_store_exact_validated_formats() {
    let decimal = reflect::I32NumericSettings::default().with_decimal();
    assert_format!(decimal, Some("%d"));

    let signed_padded = reflect::I32NumericSettings::default()
        .try_with_zero_padded_decimal(4)
        .unwrap();
    assert_format!(signed_padded, Some("%04d"));

    let unsigned = reflect::U32NumericSettings::default().with_unsigned_decimal();
    assert_format!(unsigned, Some("%u"));

    let hex_lower = reflect::U32NumericSettings::default().with_hex(false);
    assert_format!(hex_lower, Some("%x"));

    let hex_upper = reflect::U32NumericSettings::default().with_hex(true);
    assert_format!(hex_upper, Some("%X"));

    let octal = reflect::U32NumericSettings::default().with_octal();
    assert_format!(octal, Some("%o"));

    let unsigned_padded = reflect::U32NumericSettings::default()
        .try_with_zero_padded_decimal(4)
        .unwrap();
    assert_format!(unsigned_padded, Some("%04u"));

    let float3 = reflect::F32NumericSettings::default()
        .try_with_fixed(3)
        .unwrap();
    assert_format!(float3, Some("%.3f"));

    let double4 = reflect::F64NumericSettings::default()
        .try_with_fixed(4)
        .unwrap();
    assert_format!(double4, Some("%.4f"));

    let sci_lower = reflect::F32NumericSettings::default()
        .try_with_scientific(2, false)
        .unwrap();
    assert_format!(sci_lower, Some("%.2e"));

    let sci_upper = reflect::F64NumericSettings::default()
        .try_with_scientific(5, true)
        .unwrap();
    assert_format!(sci_upper, Some("%.5E"));

    let pct = reflect::F32NumericSettings::default()
        .try_with_percentage(1)
        .unwrap();
    assert_format!(pct, Some("%.1f%%"));
}

#[test]
fn dynamic_formats_are_validated_before_storage() {
    let settings = reflect::F32NumericSettings::default()
        .try_with_format(String::from("value %.2f%%"))
        .expect("valid floating-point format");
    assert_format!(settings, Some("value %.2f%%"));

    assert!(
        reflect::F32NumericSettings::default()
            .try_with_format("%s")
            .is_err()
    );
    assert!(
        reflect::I32NumericSettings::default()
            .try_with_format("%u")
            .is_err()
    );
    assert!(
        reflect::U32NumericSettings::default()
            .try_with_format("%d")
            .is_err()
    );
    assert!(
        reflect::I32NumericSettings::default()
            .try_with_zero_padded_decimal(32)
            .is_err()
    );
    assert!(
        reflect::F32NumericSettings::default()
            .try_with_fixed(100)
            .is_err()
    );
}

#[test]
fn numeric_type_settings_presets_apply_expected_defaults() {
    use reflect::{NumericRange, NumericWidgetKind};

    let slider01 = reflect::F32NumericSettings::default()
        .try_slider_0_to_1(3)
        .unwrap();
    assert!(matches!(
        slider01.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(slider01.clamp);
    assert!(slider01.always_clamp);
    assert!(matches!(slider01.widget, NumericWidgetKind::Slider));
    assert_format!(slider01, Some("%.3f"));

    let slider_neg1_1 = reflect::F64NumericSettings::default()
        .try_slider_minus1_to_1(2)
        .unwrap();
    assert!(matches!(
        slider_neg1_1.range,
        NumericRange::Explicit { min, max }
        if (min + 1.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(slider_neg1_1.clamp);
    assert!(slider_neg1_1.always_clamp);
    assert!(matches!(slider_neg1_1.widget, NumericWidgetKind::Slider));
    assert_format!(slider_neg1_1, Some("%.2f"));

    let drag = reflect::F32NumericSettings::default()
        .try_drag_with_speed(0.01, 4)
        .unwrap();
    assert!(matches!(drag.range, NumericRange::None));
    assert_eq!(drag.speed, Some(0.01));
    assert!(matches!(drag.widget, NumericWidgetKind::Drag));
    assert_format!(drag, Some("%.4f"));

    let pct_slider = reflect::F64NumericSettings::default()
        .try_percentage_slider_0_to_1(1)
        .unwrap();
    assert!(matches!(
        pct_slider.range,
        NumericRange::Explicit { min, max }
        if (min - 0.0).abs() < f64::EPSILON && (max - 1.0).abs() < f64::EPSILON
    ));
    assert!(pct_slider.clamp);
    assert!(pct_slider.always_clamp);
    assert!(matches!(pct_slider.widget, NumericWidgetKind::Slider));
    assert_format!(pct_slider, Some("%.1f%%"));
}

#[test]
fn numeric_type_settings_flags_match_slider_and_drag_support() {
    let settings = reflect::F32NumericSettings {
        log: true,
        always_clamp: true,
        wrap_around: true,
        no_round_to_format: true,
        no_input: true,
        clamp_on_input: true,
        clamp_zero_range: true,
        no_speed_tweaks: true,
        ..reflect::F32NumericSettings::default()
    };

    let slider_flags = settings.slider_flags();
    assert!(
        !slider_flags.intersects(reflect::imgui::SliderFlags::from_bits_retain(
            reflect::imgui::sys::ImGuiSliderFlags_WrapAround
        ))
    );
    assert!(slider_flags.contains(reflect::imgui::SliderFlags::LOGARITHMIC));
    assert!(slider_flags.contains(reflect::imgui::SliderFlags::ALWAYS_CLAMP));

    let drag_flags = settings.drag_flags();
    assert!(drag_flags.contains(reflect::imgui::DragFlags::WRAP_AROUND));
    assert!(drag_flags.contains(reflect::imgui::DragFlags::LOGARITHMIC));
    assert!(drag_flags.contains(reflect::imgui::DragFlags::ALWAYS_CLAMP));
}
