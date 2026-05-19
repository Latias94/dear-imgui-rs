pub(super) fn checked_texture_dimension_to_i32(caller: &str, name: &str, value: u32) -> i32 {
    assert!(value > 0, "{caller} {name} must be positive");
    i32::try_from(value).unwrap_or_else(|_| panic!("{caller} {name} exceeded i32 range"))
}

pub(super) fn checked_texture_byte_len(
    caller: &str,
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
) -> usize {
    assert!(width > 0, "{caller} width must be positive");
    assert!(height > 0, "{caller} height must be positive");
    assert!(
        bytes_per_pixel > 0,
        "{caller} bytes_per_pixel must be positive"
    );

    let width = usize::try_from(width).expect("positive width must fit usize");
    let height = usize::try_from(height).expect("positive height must fit usize");

    let size = width
        .checked_mul(height)
        .and_then(|size| size.checked_mul(bytes_per_pixel))
        .expect("texture byte size overflowed usize");
    assert!(
        size <= i32::MAX as usize,
        "{caller} texture byte size must fit Dear ImGui's signed int allocation path"
    );
    size
}

pub(super) fn checked_texture_byte_len_if_valid(
    caller: &str,
    width: i32,
    height: i32,
    bytes_per_pixel: i32,
) -> Option<usize> {
    if width <= 0 || height <= 0 || bytes_per_pixel <= 0 {
        return None;
    }
    let width = u32::try_from(width).ok()?;
    let height = u32::try_from(height).ok()?;
    let bytes_per_pixel = usize::try_from(bytes_per_pixel).ok()?;
    Some(checked_texture_byte_len(
        caller,
        width,
        height,
        bytes_per_pixel,
    ))
}

pub(super) fn non_negative_texture_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}
