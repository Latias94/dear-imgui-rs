//! Utility functions for ImGuizmo
//!
//! This module provides various utility functions for string handling,
//! coordinate conversions, and common operations.

use crate::types::{Color, Vec2, Vec3};
use crate::{GuizmoError, GuizmoResult};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Convert a Rust string to a C-compatible string
///
/// This function safely converts a Rust string to a CString, handling
/// potential null bytes by removing them.
pub fn to_cstring(s: &str) -> CString {
    // Remove null bytes to prevent CString creation errors
    let cleaned = s.replace('\0', "");
    CString::new(cleaned).unwrap_or_else(|_| CString::new("").unwrap())
}

/// Convert an optional string to a C string pointer
///
/// Returns a null pointer if the input is None, otherwise returns
/// a pointer to the C string along with the CString to keep it alive.
///
/// # Safety
/// The returned pointer is only valid as long as the returned CString is alive.
pub fn to_cstring_ptr(s: Option<&str>) -> (*const c_char, Option<std::ffi::CString>) {
    match s {
        Some(string) => {
            let cstring = to_cstring(string);
            let ptr = cstring.as_ptr();
            (ptr, Some(cstring))
        }
        None => (std::ptr::null(), None),
    }
}

/// Convert a C string pointer to a Rust string
///
/// # Safety
/// The caller must ensure that the pointer is valid and points to a null-terminated string.
pub unsafe fn from_cstring_ptr(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_owned())
    }
}

/// Convert screen coordinates to normalized device coordinates (NDC)
pub fn screen_to_ndc(screen_pos: Vec2, viewport_size: Vec2) -> Vec2 {
    Vec2::new(
        (screen_pos.x / viewport_size.x) * 2.0 - 1.0,
        1.0 - (screen_pos.y / viewport_size.y) * 2.0,
    )
}

/// Convert normalized device coordinates to screen coordinates
pub fn ndc_to_screen(ndc: Vec2, viewport_size: Vec2) -> Vec2 {
    Vec2::new(
        (ndc.x + 1.0) * 0.5 * viewport_size.x,
        (1.0 - ndc.y) * 0.5 * viewport_size.y,
    )
}

/// Clamp a value between min and max
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Clamp a vector between min and max values
pub fn clamp_vec3(value: Vec3, min: Vec3, max: Vec3) -> Vec3 {
    Vec3::new(
        clamp(value.x, min.x, max.x),
        clamp(value.y, min.y, max.y),
        clamp(value.z, min.z, max.z),
    )
}

/// Check if a value is approximately equal to another within epsilon
pub fn approx_equal(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() < epsilon
}

/// Check if two vectors are approximately equal
pub fn approx_equal_vec3(a: Vec3, b: Vec3, epsilon: f32) -> bool {
    approx_equal(a.x, b.x, epsilon)
        && approx_equal(a.y, b.y, epsilon)
        && approx_equal(a.z, b.z, epsilon)
}

/// Calculate the signed angle between two 2D vectors
pub fn signed_angle_2d(from: Vec2, to: Vec2) -> f32 {
    let cross = from.x * to.y - from.y * to.x;
    let dot = from.dot(to);
    cross.atan2(dot)
}

/// Calculate the angle between two 3D vectors
pub fn angle_between_vectors(a: Vec3, b: Vec3) -> f32 {
    let dot = a.normalize().dot(b.normalize());
    dot.clamp(-1.0, 1.0).acos()
}

/// Generate a color from HSV values
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r + m, g + m, b + m, 1.0]
}

/// Convert RGB color to HSV
pub fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = color[0];
    let g = color[1];
    let b = color[2];

    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let s = if max == 0.0 { 0.0 } else { delta / max };
    let v = max;

    (h, s, v)
}

/// Blend two colors using alpha blending
pub fn blend_colors(foreground: Color, background: Color) -> Color {
    let alpha = foreground[3];
    let inv_alpha = 1.0 - alpha;

    [
        foreground[0] * alpha + background[0] * inv_alpha,
        foreground[1] * alpha + background[1] * inv_alpha,
        foreground[2] * alpha + background[2] * inv_alpha,
        foreground[3] + background[3] * inv_alpha,
    ]
}

/// Create a color with modified alpha
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    [color[0], color[1], color[2], alpha.clamp(0.0, 1.0)]
}

/// Darken a color by a factor
pub fn darken_color(color: Color, factor: f32) -> Color {
    let factor = factor.clamp(0.0, 1.0);
    [
        color[0] * factor,
        color[1] * factor,
        color[2] * factor,
        color[3],
    ]
}

/// Lighten a color by a factor
pub fn lighten_color(color: Color, factor: f32) -> Color {
    let factor = factor.clamp(0.0, 1.0);
    [
        color[0] + (1.0 - color[0]) * factor,
        color[1] + (1.0 - color[1]) * factor,
        color[2] + (1.0 - color[2]) * factor,
        color[3],
    ]
}

/// Check if a point is inside a circle
pub fn point_in_circle(point: Vec2, center: Vec2, radius: f32) -> bool {
    (point - center).length_squared() <= radius * radius
}

/// Check if a point is inside a rectangle
pub fn point_in_rect(point: Vec2, rect_min: Vec2, rect_max: Vec2) -> bool {
    point.x >= rect_min.x && point.x <= rect_max.x && point.y >= rect_min.y && point.y <= rect_max.y
}

/// Calculate the closest point on a line segment to a given point
pub fn closest_point_on_line_segment(point: Vec3, line_start: Vec3, line_end: Vec3) -> Vec3 {
    let line_vec = line_end - line_start;
    let point_vec = point - line_start;

    let line_length_sq = line_vec.length_squared();
    if line_length_sq < f32::EPSILON {
        return line_start;
    }

    let t = (point_vec.dot(line_vec) / line_length_sq).clamp(0.0, 1.0);
    line_start + line_vec * t
}

/// Generate a unique ID from a string (simple hash)
pub fn string_to_id(s: &str) -> u32 {
    let mut hash = 0u32;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    hash
}

/// Format a float with a specific number of decimal places
pub fn format_float(value: f32, decimals: usize) -> String {
    format!("{:.1$}", value, decimals)
}

/// Validate that a viewport has positive dimensions
pub fn validate_viewport(_x: f32, _y: f32, width: f32, height: f32) -> GuizmoResult<()> {
    if width <= 0.0 {
        return Err(GuizmoError::invalid_viewport(format!(
            "Width must be positive, got {}",
            width
        )));
    }
    if height <= 0.0 {
        return Err(GuizmoError::invalid_viewport(format!(
            "Height must be positive, got {}",
            height
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_cstring_conversion() {
        let s = "test string";
        let cstring = to_cstring(s);
        assert_eq!(cstring.to_str().unwrap(), s);

        // Test with null bytes
        let s_with_null = "test\0string";
        let cstring = to_cstring(s_with_null);
        assert_eq!(cstring.to_str().unwrap(), "teststring");
    }

    #[test]
    fn test_coordinate_conversion() {
        let screen = Vec2::new(400.0, 300.0);
        let viewport = Vec2::new(800.0, 600.0);

        let ndc = screen_to_ndc(screen, viewport);
        assert_relative_eq!(ndc, Vec2::new(0.0, 0.0), epsilon = 1e-6);

        let back_to_screen = ndc_to_screen(ndc, viewport);
        assert_relative_eq!(back_to_screen, screen, epsilon = 1e-6);
    }

    #[test]
    fn test_angle_calculations() {
        let v1 = Vec2::new(1.0, 0.0);
        let v2 = Vec2::new(0.0, 1.0);

        let angle = signed_angle_2d(v1, v2);
        assert_relative_eq!(angle, std::f32::consts::PI / 2.0, epsilon = 1e-6);
    }

    #[test]
    fn test_color_operations() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 0.5];

        let blended = blend_colors(blue, red);
        assert_eq!(blended[0], 0.5); // Red component
        assert_eq!(blended[2], 0.5); // Blue component
    }

    #[test]
    fn test_hsv_conversion() {
        let red_rgb = [1.0, 0.0, 0.0, 1.0];
        let (h, s, v) = rgb_to_hsv(red_rgb);
        let back_to_rgb = hsv_to_rgb(h, s, v);

        assert_relative_eq!(back_to_rgb[0], red_rgb[0], epsilon = 1e-6);
        assert_relative_eq!(back_to_rgb[1], red_rgb[1], epsilon = 1e-6);
        assert_relative_eq!(back_to_rgb[2], red_rgb[2], epsilon = 1e-6);
    }

    #[test]
    fn test_point_in_shapes() {
        let center = Vec2::new(0.0, 0.0);
        let point_inside = Vec2::new(0.5, 0.5);
        let point_outside = Vec2::new(2.0, 2.0);

        assert!(point_in_circle(point_inside, center, 1.0));
        assert!(!point_in_circle(point_outside, center, 1.0));

        let rect_min = Vec2::new(-1.0, -1.0);
        let rect_max = Vec2::new(1.0, 1.0);

        assert!(point_in_rect(point_inside, rect_min, rect_max));
        assert!(!point_in_rect(point_outside, rect_min, rect_max));
    }

    #[test]
    fn test_viewport_validation() {
        assert!(validate_viewport(0.0, 0.0, 800.0, 600.0).is_ok());
        assert!(validate_viewport(0.0, 0.0, -800.0, 600.0).is_err());
        assert!(validate_viewport(0.0, 0.0, 800.0, 0.0).is_err());
    }
}
