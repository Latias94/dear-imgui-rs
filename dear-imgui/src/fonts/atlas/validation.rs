use crate::sys;

pub(super) const RASTERIZER_MULTIPLY_MAX: f32 = 16_000_000.0;

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

pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(super) fn assert_non_negative_i8(caller: &str, name: &str, value: i8) {
    assert!(value >= 0, "{caller} {name} must be non-negative");
}

pub(super) fn frame_count_to_i32(caller: &str, name: &str, value: usize) -> i32 {
    i32::try_from(value)
        .unwrap_or_else(|_| panic!("{caller} {name} exceeded Dear ImGui's i32 range"))
}

pub(super) fn validate_font_size_pixels(caller: &str, name: &str, size_pixels: f32) -> f32 {
    assert_non_negative_f32(caller, name, size_pixels);
    size_pixels
}

pub(super) fn validate_font_size_pixels_option(
    caller: &str,
    name: &str,
    size_pixels: Option<f32>,
) -> f32 {
    let size_pixels = size_pixels.unwrap_or(0.0);
    validate_font_size_pixels(caller, name, size_pixels)
}

pub(super) fn assert_reference_font_size_for_metrics(
    caller: &str,
    size_pixels: f32,
    has_reference_size_dependent_metrics: bool,
) {
    assert!(
        !has_reference_size_dependent_metrics || size_pixels > 0.0,
        "{caller} glyph offset/advance overrides require a positive reference size"
    );
}

pub(super) fn assert_font_source_for_add_font(caller: &str, raw: &sys::ImFontConfig) {
    let has_font_data = !raw.FontData.is_null() && raw.FontDataSize > 0;
    let has_font_loader = !raw.FontLoader.is_null();
    assert!(
        has_font_data || has_font_loader,
        "{caller} requires FontData/FontDataSize or FontLoader"
    );
    if has_font_loader {
        unsafe {
            assert!(
                (*raw.FontLoader).FontBakedLoadGlyph.is_some(),
                "{caller} FontLoader must provide FontBakedLoadGlyph"
            );
        }
    }
}
