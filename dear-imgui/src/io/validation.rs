pub(crate) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

pub(crate) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(crate) fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

pub(crate) fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

pub(crate) fn assert_display_size(caller: &str, size: [f32; 2]) {
    assert_finite_vec2(caller, "size", size);
    assert!(
        size[0] >= 0.0 && size[1] >= 0.0,
        "{caller} size must contain non-negative values"
    );
}

pub(crate) fn assert_display_framebuffer_scale(caller: &str, scale: [f32; 2]) {
    assert_finite_vec2(caller, "scale", scale);
    assert!(
        scale[0] >= 0.0 && scale[1] >= 0.0,
        "{caller} scale must contain non-negative values"
    );
}

pub(crate) fn assert_memory_compact_timer(caller: &str, seconds: f32) {
    assert_finite_f32(caller, "seconds", seconds);
    assert!(
        seconds >= 0.0 || seconds == -1.0,
        "{caller} seconds must be non-negative, or -1.0 to disable"
    );
}

pub(crate) fn metric_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}
