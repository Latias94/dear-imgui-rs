use crate::{CteError, CteResult, sys, validation::validate_finite_f32};
use dear_imgui_rs::{FontConfig, FontLoaderFlags, FontSource};
use std::{ffi::c_void, slice};

/// Returns cimCTE's bundled DejaVu font as a safe Dear ImGui font source.
///
/// Add the returned source to the context's [`dear_imgui_rs::FontAtlas`] before
/// the renderer builds or uploads the atlas. Unlike cimCTE's raw `SetDejavu`,
/// this helper does not clear the atlas or bypass renderer texture management.
pub fn dejavu_font_source(size_pixels: f32) -> CteResult<FontSource<'static>> {
    bundled_font_source(
        "dejavu_font_source",
        "DejaVu",
        size_pixels,
        sys::GetDejavu,
        FontLoaderFlags::MONO_HINTING,
    )
}

/// Returns cimCTE's bundled Noto Sans SC font through the managed font atlas.
///
/// Add the returned source before renderer initialization. The compressed bytes
/// are immutable upstream storage and remain valid for the application's lifetime.
pub fn noto_sans_sc_font_source(size_pixels: f32) -> CteResult<FontSource<'static>> {
    bundled_font_source(
        "noto_sans_sc_font_source",
        "Noto Sans SC",
        size_pixels,
        sys::Getnotosans,
        FontLoaderFlags::LIGHT_HINTING,
    )
}

fn bundled_font_source(
    operation: &'static str,
    name: &str,
    size_pixels: f32,
    getter: unsafe extern "C" fn(*mut *mut c_void) -> std::ffi::c_int,
    flags: FontLoaderFlags,
) -> CteResult<FontSource<'static>> {
    validate_finite_f32(operation, "size_pixels", size_pixels)?;
    if size_pixels <= 0.0 {
        return Err(CteError::InvalidValue {
            operation,
            parameter: "size_pixels",
            requirement: "greater than zero",
        });
    }

    let config = FontConfig::new()
        .name(name)
        .oversample_h(1)
        .oversample_v(1)
        .font_loader_flags(flags);
    let mut data: *mut c_void = std::ptr::null_mut();
    let size = unsafe { getter(&mut data) };
    assert!(
        !data.is_null() && size > 0,
        "cimCTE returned invalid bundled font data"
    );
    let size = usize::try_from(size).expect("positive c_int must fit usize");
    let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), size) };
    let source = unsafe { FontSource::compressed_ttf_data_with_size(bytes, size_pixels) };
    Ok(source.with_config(config))
}
