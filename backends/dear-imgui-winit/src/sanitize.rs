use winit::dpi::{LogicalPosition, LogicalSize};

pub(crate) fn finite_f32(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

pub(crate) fn finite_f64_to_f32(value: f64) -> Option<f32> {
    if !value.is_finite() {
        return None;
    }

    finite_f32(value as f32)
}

pub(crate) fn finite_vec2_f64_to_f32(value: [f64; 2]) -> Option<[f32; 2]> {
    Some([finite_f64_to_f32(value[0])?, finite_f64_to_f32(value[1])?])
}

pub(crate) fn finite_vec2_f32(value: [f32; 2]) -> Option<[f32; 2]> {
    Some([finite_f32(value[0])?, finite_f32(value[1])?])
}

pub(crate) fn finite_position(value: LogicalPosition<f64>) -> Option<[f32; 2]> {
    finite_vec2_f64_to_f32([value.x, value.y])
}

pub(crate) fn finite_non_negative_size(value: LogicalSize<f64>) -> [f32; 2] {
    [
        finite_non_negative_f64_to_f32(value.width).unwrap_or(0.0),
        finite_non_negative_f64_to_f32(value.height).unwrap_or(0.0),
    ]
}

pub(crate) fn finite_non_negative_f64_to_f32(value: f64) -> Option<f32> {
    let value = finite_f64_to_f32(value)?;
    (value >= 0.0).then_some(value)
}

pub(crate) fn finite_or_zero(value: f32) -> f32 {
    finite_f32(value).unwrap_or(0.0)
}

pub(crate) fn positive_finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

pub(crate) fn positive_finite_f32_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

pub(crate) fn framebuffer_scale(value: f64, fallback: f64) -> [f32; 2] {
    let scale = positive_finite_f32_or(positive_finite_or(value, fallback) as f32, 1.0);
    [scale, scale]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_positions() {
        assert_eq!(finite_vec2_f64_to_f32([1.0, 2.0]), Some([1.0, 2.0]));
        assert_eq!(finite_vec2_f64_to_f32([f64::NAN, 2.0]), None);
        assert_eq!(finite_vec2_f64_to_f32([1.0, f64::INFINITY]), None);
        assert_eq!(finite_vec2_f64_to_f32([f64::MAX, 2.0]), None);
    }

    #[test]
    fn clamps_invalid_sizes_to_zero() {
        assert_eq!(
            finite_non_negative_size(LogicalSize::new(640.0, 480.0)),
            [640.0, 480.0]
        );
        assert_eq!(
            finite_non_negative_size(LogicalSize::new(-1.0, f64::NAN)),
            [0.0, 0.0]
        );
    }

    #[test]
    fn positive_scale_uses_fallback_for_invalid_values() {
        assert_eq!(positive_finite_or(2.0, 1.0), 2.0);
        assert_eq!(positive_finite_or(0.0, 1.0), 1.0);
        assert_eq!(positive_finite_or(f64::NAN, 1.0), 1.0);
        assert_eq!(positive_finite_or(f64::INFINITY, 1.0), 1.0);
    }
}
