use crate::{StyleVar, StyleVarType};

pub(super) fn assert_style_var_type(caller: &str, var: StyleVar, expected: StyleVarType) {
    let actual = var.value_type();
    assert_eq!(
        actual, expected,
        "{caller} expected {expected:?} style variable, got {actual:?} for {var:?}"
    );
}

pub(super) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

pub(super) fn assert_non_negative_finite_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} components must be finite"
    );
}

pub(super) fn assert_finite_rect(caller: &str, min: [f32; 2], max: [f32; 2]) {
    assert_finite_vec2(caller, "min", min);
    assert_finite_vec2(caller, "max", max);
    assert!(
        min[0] <= max[0] && min[1] <= max[1],
        "{caller} min must not exceed max"
    );
}

pub(super) fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value.iter().all(|component| *component >= 0.0),
        "{caller} {name} components must be non-negative"
    );
}

pub(super) fn assert_finite_vec4(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} components must be finite"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "expected Float style variable")]
    fn style_var_push_rejects_wrong_value_type() {
        assert_style_var_type("test", StyleVar::NodePadding, StyleVarType::Float);
    }
}
