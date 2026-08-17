use crate::{CteError, CteResult, sys, validation::validate_finite_f32};
use dear_imgui_rs::{FontConfig, FontLoaderFlags, FontSource};
use std::{ffi::c_void, slice};

/// Returns cimCTE's bundled DejaVu font as a safe Dear ImGui font source.
///
/// Add the returned source to the context's [`dear_imgui_rs::FontAtlas`] before
/// the renderer builds or uploads the atlas. Unlike cimCTE's raw `SetDejavu`,
/// this helper does not clear the atlas or bypass renderer texture management.
pub fn dejavu_font_source(size_pixels: f32) -> CteResult<FontSource<'static>> {
    const OPERATION: &str = "dejavu_font_source";
    validate_finite_f32(OPERATION, "size_pixels", size_pixels)?;
    if size_pixels <= 0.0 {
        return Err(CteError::InvalidValue {
            operation: OPERATION,
            parameter: "size_pixels",
            requirement: "greater than zero",
        });
    }

    let config = FontConfig::new()
        .name("DejaVu")
        .oversample_h(1)
        .oversample_v(1)
        .font_loader_flags(FontLoaderFlags::MONO_HINTING);
    let source =
        unsafe { FontSource::compressed_ttf_data_with_size(bundled_dejavu_data(), size_pixels) };
    Ok(source.with_config(config))
}

fn bundled_dejavu_data() -> &'static [u8] {
    let mut data: *mut c_void = std::ptr::null_mut();
    let size = unsafe { sys::GetDejavu(&mut data) };
    assert!(
        !data.is_null() && size > 0,
        "cimCTE returned invalid bundled DejaVu font data"
    );
    let size = usize::try_from(size).expect("positive c_int must fit usize");
    unsafe { slice::from_raw_parts(data.cast::<u8>(), size) }
}
