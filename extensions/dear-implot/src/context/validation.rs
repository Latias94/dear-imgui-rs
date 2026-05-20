pub(super) fn assert_finite_f64(caller: &str, name: &str, value: f64) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must be finite"
    );
}

pub(super) fn assert_finite_f64_slice(caller: &str, name: &str, values: &[f64]) {
    assert!(
        values.iter().all(|value| value.is_finite()),
        "{caller} {name} must contain only finite values"
    );
}

pub(super) fn assert_axis_limit_range(caller: &str, min: f64, max: f64) {
    assert_finite_f64(caller, "min", min);
    assert_finite_f64(caller, "max", max);
    assert!(min != max, "{caller} min and max must differ");
}

pub(super) fn assert_axis_constraint_range(caller: &str, min: f64, max: f64) {
    assert_finite_f64(caller, "min", min);
    assert_finite_f64(caller, "max", max);
    assert!(min <= max, "{caller} min must be <= max");
}

pub(super) fn assert_axis_zoom_range(caller: &str, min: f64, max: f64) {
    assert_finite_f64(caller, "min", min);
    assert_finite_f64(caller, "max", max);
    assert!(min > 0.0, "{caller} min must be positive");
    assert!(min <= max, "{caller} min must be <= max");
}

pub(crate) fn axis_tick_count_to_i32(caller: &str, n_ticks: usize) -> i32 {
    assert!(n_ticks > 0, "{caller} n_ticks must be positive");
    i32::try_from(n_ticks)
        .unwrap_or_else(|_| panic!("{caller} n_ticks exceeded ImPlot's i32 range"))
}
