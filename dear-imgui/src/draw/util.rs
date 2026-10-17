use crate::sys;

use super::counts::DrawFlags;

pub(super) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

pub(super) fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

pub(super) fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

pub(super) fn count_to_i32(caller: &str, name: &str, value: usize) -> i32 {
    i32::try_from(value)
        .unwrap_or_else(|_| panic!("{caller} {name} exceeded Dear ImGui's i32 range"))
}

pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: sys::ImVec2) {
    assert!(
        value.x.is_finite() && value.y.is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(super) fn assert_non_negative_vec2(caller: &str, name: &str, value: sys::ImVec2) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value.x >= 0.0 && value.y >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

pub(super) fn assert_finite_vec4(caller: &str, name: &str, value: sys::ImVec4) {
    assert!(
        value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(super) fn finite_vec2(caller: &str, name: &str, value: impl Into<sys::ImVec2>) -> sys::ImVec2 {
    let value = value.into();
    assert_finite_vec2(caller, name, value);
    value
}

pub(super) fn non_negative_vec2(
    caller: &str,
    name: &str,
    value: impl Into<sys::ImVec2>,
) -> sys::ImVec2 {
    let value = value.into();
    assert_non_negative_vec2(caller, name, value);
    value
}

pub(super) fn finite_vec4(caller: &str, name: &str, value: impl Into<sys::ImVec4>) -> sys::ImVec4 {
    let value = value.into();
    assert_finite_vec4(caller, name, value);
    value
}

pub(super) fn assert_path_not_empty(draw_list: *mut sys::ImDrawList, caller: &str) {
    let path_size = unsafe { (*draw_list)._Path.Size };
    assert!(
        path_size > 0,
        "{caller} requires a current path point; call path_line_to() first"
    );
}

pub(super) fn assert_arc_fast_steps(caller: &str, a_min_of_12: i32, a_max_of_12: i32) {
    assert!(
        (0..=12).contains(&a_min_of_12),
        "{caller} a_min_of_12 must be in 0..=12"
    );
    assert!(
        (0..=12).contains(&a_max_of_12),
        "{caller} a_max_of_12 must be in 0..=12"
    );
}

pub(super) const STROKE_POSITION: DrawFlags =
    DrawFlags::from_bits_retain(sys::ImDrawFlags_StrokeMask_ as u32);

pub(super) const STROKE_FLAGS: DrawFlags = DrawFlags::from_bits_retain(
    STROKE_POSITION.bits()
        | DrawFlags::JOIN_MITER.bits()
        | DrawFlags::CAP_SQUARE.bits()
        | DrawFlags::AA_LINES.bits()
        | DrawFlags::AA_LINE_ENDS.bits(),
);

pub(super) const FILL_FLAGS: DrawFlags =
    DrawFlags::from_bits_retain(DrawFlags::AA_FILL.bits() | DrawFlags::JOIN_MITER.bits());

pub(super) const CORNER_FLAGS: DrawFlags =
    DrawFlags::from_bits_retain(sys::ImDrawFlags_RoundMask_ as u32);

pub(super) fn assert_draw_flags(caller: &str, flags: DrawFlags, supported: DrawFlags) {
    assert!(
        flags.difference(supported).is_empty(),
        "{caller} flags contain unsupported ImDrawFlags bits"
    );
    let stroke = flags.bits() & sys::ImDrawFlags_StrokeMask_ as u32;
    assert!(
        matches!(stroke >> 17, 0 | 1 | 2 | 3 | 4 | 7),
        "{caller} flags contain an invalid stroke position"
    );
    assert!(
        !flags.contains(DrawFlags::ROUND_NONE) || !flags.intersects(DrawFlags::ROUND_ALL),
        "{caller} ROUND_NONE cannot be combined with rounded corners"
    );
}

pub(super) fn assert_polyline_flags(caller: &str, flags: DrawFlags) {
    assert_draw_flags(caller, flags, STROKE_FLAGS | FILL_FLAGS | DrawFlags::CLOSED);
}

pub(super) fn assert_corner_flags(caller: &str, flags: DrawFlags) {
    assert_draw_flags(caller, flags, CORNER_FLAGS);
}

#[cfg(test)]
pub(super) fn draw_list_counts(draw_list: *mut sys::ImDrawList) -> (i32, i32) {
    unsafe { ((*draw_list).VtxBuffer.Size, (*draw_list).IdxBuffer.Size) }
}
