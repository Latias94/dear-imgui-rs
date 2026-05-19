use super::flags::ColorEditFlags;
use crate::sys;

#[inline]
pub(super) const fn color_display_mask() -> u32 {
    sys::ImGuiColorEditFlags_DisplayMask_ as u32
}

#[inline]
pub(super) const fn color_data_type_mask() -> u32 {
    sys::ImGuiColorEditFlags_DataTypeMask_ as u32
}

#[inline]
pub(super) const fn color_picker_mask() -> u32 {
    sys::ImGuiColorEditFlags_PickerMask_ as u32
}

#[inline]
pub(super) const fn color_input_mask() -> u32 {
    sys::ImGuiColorEditFlags_InputMask_ as u32
}

#[inline]
pub(super) const fn color_choice_mask() -> u32 {
    color_display_mask() | color_data_type_mask() | color_picker_mask() | color_input_mask()
}

#[inline]
pub(super) const fn color_edit_supported_mask() -> u32 {
    ColorEditFlags::all().bits() | color_choice_mask()
}

#[inline]
pub(super) const fn color_picker_supported_mask() -> u32 {
    ColorEditFlags::all().bits()
        | color_display_mask()
        | color_data_type_mask()
        | color_picker_mask()
        | color_input_mask()
}

#[inline]
pub(super) const fn color_button_supported_mask() -> u32 {
    ColorEditFlags::all().bits() | color_input_mask()
}

pub(super) fn validate_color_independent_flags(caller: &str, flags: ColorEditFlags) {
    let unsupported = flags.bits() & !ColorEditFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received non-independent ImGuiColorEditFlags bits: 0x{unsupported:X}"
    );
}

pub(super) fn validate_color_supported_bits(caller: &str, bits: u32, supported: u32) {
    let unsupported = bits & !supported;
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiColorEditFlags bits: 0x{unsupported:X}"
    );
}

pub(super) fn assert_color_single_choice_mask(caller: &str, bits: u32, mask: u32, name: &str) {
    assert!(
        (bits & mask).count_ones() <= 1,
        "{caller} accepts at most one color {name}"
    );
}

pub(super) fn assert_finite_color3(caller: &str, name: &str, color: &[f32; 3]) {
    assert!(
        color.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

pub(super) fn assert_finite_color4(caller: &str, name: &str, color: &[f32; 4]) {
    assert!(
        color.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

pub(super) fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}
