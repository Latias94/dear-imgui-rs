use super::{TextureDataError, TextureFormat};
use crate::texture::format::texture_format_bytes_per_pixel;

#[derive(Clone, Copy, Debug)]
pub(super) struct TextureLayout {
    pub width: u32,
    pub height: u32,
    pub bytes_per_pixel: usize,
    pub row_pitch: usize,
    pub byte_len: usize,
}

impl TextureLayout {
    fn from_parts(
        width: u32,
        height: u32,
        bytes_per_pixel: usize,
    ) -> Result<Self, TextureDataError> {
        let row_pitch = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(bytes_per_pixel));
        let byte_len = row_pitch.and_then(|pitch| {
            usize::try_from(height)
                .ok()
                .and_then(|height| pitch.checked_mul(height))
        });
        let Some((row_pitch, byte_len)) = row_pitch.zip(byte_len) else {
            return Err(TextureDataError::ByteSizeOutOfRange {
                width,
                height,
                bytes_per_pixel,
            });
        };
        if byte_len > i32::MAX as usize {
            return Err(TextureDataError::ByteSizeOutOfRange {
                width,
                height,
                bytes_per_pixel,
            });
        }
        Ok(Self {
            width,
            height,
            bytes_per_pixel,
            row_pitch,
            byte_len,
        })
    }
}

pub(super) fn validate_new_texture_layout(
    format: TextureFormat,
    width: u32,
    height: u32,
) -> Result<TextureLayout, TextureDataError> {
    if width == 0 || height == 0 {
        return Err(TextureDataError::InvalidDimensions { width, height });
    }
    if i32::try_from(width).is_err() {
        return Err(TextureDataError::WidthOutOfRange(width));
    }
    if i32::try_from(height).is_err() {
        return Err(TextureDataError::HeightOutOfRange(height));
    }
    TextureLayout::from_parts(width, height, texture_format_bytes_per_pixel(format))
}

pub(super) fn validate_native_texture_layout(
    width: i32,
    height: i32,
    bytes_per_pixel: i32,
) -> Result<TextureLayout, TextureDataError> {
    if width <= 0 || height <= 0 || bytes_per_pixel <= 0 {
        return Err(TextureDataError::InvalidLayout {
            width,
            height,
            bytes_per_pixel,
        });
    }
    let width = u32::try_from(width).expect("positive i32 width must fit u32");
    let height = u32::try_from(height).expect("positive i32 height must fit u32");
    let bytes_per_pixel =
        usize::try_from(bytes_per_pixel).expect("positive i32 bytes_per_pixel must fit usize");
    TextureLayout::from_parts(width, height, bytes_per_pixel)
}

pub(super) fn require_exact_payload(
    expected: usize,
    actual: usize,
) -> Result<(), TextureDataError> {
    if expected == actual {
        Ok(())
    } else {
        Err(TextureDataError::ByteLengthMismatch { expected, actual })
    }
}

pub(super) fn non_negative_texture_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}
