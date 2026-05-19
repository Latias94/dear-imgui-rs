use crate::sys;

use super::counts::{DrawCornerFlags, PolylineFlags};

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

pub(super) fn assert_polyline_flags(caller: &str, flags: PolylineFlags) {
    assert!(
        flags.difference(PolylineFlags::CLOSED).is_empty(),
        "{caller} flags contain unsupported ImDrawFlags bits"
    );
}

pub(super) fn assert_corner_flags(caller: &str, flags: DrawCornerFlags) {
    let supported = sys::ImDrawFlags_RoundCornersMask_ as u32;
    assert!(
        flags.bits() & !supported == 0,
        "{caller} flags contain unsupported ImDrawFlags bits"
    );
}

#[cfg(test)]
pub(super) fn draw_list_counts(draw_list: *mut sys::ImDrawList) -> (i32, i32) {
    unsafe { ((*draw_list).VtxBuffer.Size, (*draw_list).IdxBuffer.Size) }
}
