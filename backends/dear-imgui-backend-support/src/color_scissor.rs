use crate::BackendError;

/// Highest encoded sRGB channel value in the linear segment of IEC 61966-2-1 decoding.
pub const SRGB_DECODE_THRESHOLD: f32 = 0.04045;
/// Divisor for the linear segment of sRGB decoding.
pub const SRGB_LINEAR_DIVISOR: f32 = 12.92;
/// Offset added before the power segment of sRGB decoding.
pub const SRGB_OFFSET: f32 = 0.055;
/// Divisor used before the power segment of sRGB decoding.
pub const SRGB_SCALE: f32 = 1.055;
/// Exponent of the power segment of sRGB decoding.
pub const SRGB_EXPONENT: f32 = 2.4;

/// Project a clip rectangle to a top-left-origin framebuffer scissor `[x, y, width, height]`.
///
/// `clip_rect` holds `[left, top, right, bottom]` in draw coordinates. Subtract `clip_off`
/// before applying `clip_scale`. Round minima down and maxima up, then clamp to `fb_size`.
/// Returns `None` for an empty, inverted, offscreen, or non-finite projection.
#[must_use]
pub fn compute_scissor_rect(
    clip_rect: [f32; 4],
    clip_off: [f32; 2],
    clip_scale: [f32; 2],
    fb_size: [u32; 2],
) -> Option<[u32; 4]> {
    try_compute_scissor_rect(clip_rect, clip_off, clip_scale, fb_size)
        .ok()
        .flatten()
}

/// Project a clip rectangle while distinguishing invalid coordinates from an empty scissor.
///
/// See [`compute_scissor_rect`] for the projection. Non-positive scales yield an empty scissor.
///
/// # Errors
///
/// Returns [`BackendError::NonFiniteClipRect`] if any of the projected edges are non-finite.
pub fn try_compute_scissor_rect(
    clip_rect: [f32; 4],
    clip_off: [f32; 2],
    clip_scale: [f32; 2],
    fb_size: [u32; 2],
) -> Result<Option<[u32; 4]>, BackendError> {
    let [scale_x, scale_y] = clip_scale;
    let [off_x, off_y] = clip_off;
    let [left, top, right, bottom] = clip_rect;
    let projected = [
        (left - off_x) * scale_x,
        (top - off_y) * scale_y,
        (right - off_x) * scale_x,
        (bottom - off_y) * scale_y,
    ];
    if !projected.into_iter().all(f32::is_finite) {
        return Err(BackendError::NonFiniteClipRect(clip_rect));
    }
    let [left, top, right, bottom] = projected;
    if scale_x <= 0.0 || scale_y <= 0.0 || left >= right || top >= bottom {
        return Ok(None);
    }

    let x = left.floor().max(0.0) as u32;
    let y = top.floor().max(0.0) as u32;
    // Clamp the rounded endpoints in integer space to keep the exact framebuffer bounds.
    let right = (right.ceil() as u32).min(fb_size[0]);
    let bottom = (bottom.ceil() as u32).min(fb_size[1]);
    if x >= right || y >= bottom {
        return Ok(None);
    }
    Ok(Some([x, y, right - x, bottom - y]))
}

/// Decode one sRGB channel to linear light using the IEC 61966-2-1 piecewise function.
///
/// Negative inputs are clamped to zero. Positive inputs above one retain their extended range.
#[must_use]
pub fn srgb_to_linear_channel(channel: f32) -> f32 {
    let channel = channel.max(0.0);
    if channel <= SRGB_DECODE_THRESHOLD {
        channel / SRGB_LINEAR_DIVISOR
    } else {
        ((channel + SRGB_OFFSET) / SRGB_SCALE).powf(SRGB_EXPONENT)
    }
}

/// CPU reference for decoding Dear ImGui vertex RGBA after interpolation.
///
/// In sRGB compositing mode, interpolate the encoded RGB through rasterization, decode it in the
/// fragment stage, and multiply the resulting linear RGBA by the linear texture sample. Use an
/// sRGB texture view or one explicit texture decode for sRGB-encoded textures. Keep vertex alpha
/// unchanged by the decode.
#[must_use]
pub fn srgb_vertex_color_to_linear([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    [
        srgb_to_linear_channel(r),
        srgb_to_linear_channel(g),
        srgb_to_linear_channel(b),
        a,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scissor_applies_offset_and_scale_with_outward_rounding() {
        assert_eq!(
            compute_scissor_rect([9.9, 20.25, 12.1, 21.01], [10.0, 20.0], [2.0, 4.0], [4, 3]),
            Some([0, 1, 4, 2]),
        );
        assert_eq!(
            compute_scissor_rect([0.2, 1.2, 0.3, 1.4], [0.0; 2], [1.0; 2], [8, 8]),
            Some([0, 1, 1, 1]),
        );
    }

    #[test]
    fn scissor_skips_empty_projections_before_and_after_rounding() {
        for clip in [
            [0.2, 0.0, 0.2, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 8.1, 1.0, 8.2],
            [-1.0, 0.0, -0.1, 1.0],
        ] {
            assert_eq!(compute_scissor_rect(clip, [0.0; 2], [1.0; 2], [8, 8]), None);
        }
        let clip = [0.0, 0.0, 1.0, 1.0];
        assert_eq!(compute_scissor_rect(clip, [0.0; 2], [1.0; 2], [8, 0]), None);
        assert_eq!(
            compute_scissor_rect(clip, [0.0; 2], [-1.0, 1.0], [8, 8]),
            None
        );
    }

    #[test]
    fn scissor_detects_non_finite_inputs_and_projections() {
        let clip = [0.0, 0.0, f32::MAX, 1.0];
        assert!(matches!(
            try_compute_scissor_rect(clip, [0.0; 2], [2.0, 1.0], [8, 8]),
            Err(BackendError::NonFiniteClipRect(rect)) if rect == clip
        ));
        assert_eq!(
            compute_scissor_rect(clip, [0.0; 2], [2.0, 1.0], [8, 8]),
            None
        );
        assert_eq!(
            compute_scissor_rect(clip, [0.0, f32::NAN], [1.0; 2], [8, 8]),
            None
        );
    }

    #[test]
    fn scissor_respects_framebuffer_sizes_between_adjacent_f32_values() {
        let size = [16_777_219, u32::MAX];
        assert_eq!(
            compute_scissor_rect([0.0, 0.0, f32::MAX, f32::MAX], [0.0; 2], [1.0; 2], size),
            Some([0, 0, size[0], size[1]]),
        );
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-7,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn srgb_decode_matches_the_piecewise_reference() {
        assert_eq!(srgb_to_linear_channel(-0.2), 0.0);
        assert_eq!(srgb_to_linear_channel(0.0), 0.0);
        assert_eq!(srgb_to_linear_channel(0.04045), 0.04045 / 12.92);
        assert_close(srgb_to_linear_channel(0.04046), 0.003_131_594_5);
        assert_close(srgb_to_linear_channel(0.5), 0.214_041_14);
        assert_eq!(srgb_to_linear_channel(1.0), 1.0);
    }

    #[test]
    fn interpolated_vertex_rgb_is_decoded_with_alpha_unchanged() {
        let [r, g, b, a] = srgb_vertex_color_to_linear([0.5, 0.25, 0.02, 0.25]);
        assert_close(r, 0.214_041_14);
        assert_close(g, 0.050_876_09);
        assert_close(b, 0.001_547_987_6);
        assert_eq!(a, 0.25);
    }
}
