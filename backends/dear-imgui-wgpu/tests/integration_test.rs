//! Integration tests for the WGPU backend improvements
//!
//! This test verifies that our improvements maintain compatibility with the C++ implementation

use dear_imgui_wgpu::{Uniforms, WgpuInitInfo, WgpuRenderer};
use wgpu::*;

/// Test that gamma correction values match the C++ implementation
#[test]
fn test_gamma_correction_formats() {
    // Test sRGB formats that should have gamma = 2.2
    let srgb_formats = vec![
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Bc1RgbaUnormSrgb,
        TextureFormat::Bc2RgbaUnormSrgb,
        TextureFormat::Bc3RgbaUnormSrgb,
        TextureFormat::Bc7RgbaUnormSrgb,
        TextureFormat::Etc2Rgb8UnormSrgb,
        TextureFormat::Etc2Rgb8A1UnormSrgb,
        TextureFormat::Etc2Rgba8UnormSrgb,
    ];

    for format in srgb_formats {
        let gamma = Uniforms::gamma_for_format(format);
        assert_eq!(gamma, 2.2, "sRGB format {:?} should have gamma 2.2", format);
    }

    // Test ASTC sRGB formats
    let astc_srgb_formats = vec![
        TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::UnormSrgb,
        },
    ];

    for format in astc_srgb_formats {
        let gamma = Uniforms::gamma_for_format(format);
        assert_eq!(
            gamma, 2.2,
            "ASTC sRGB format {:?} should have gamma 2.2",
            format
        );
    }

    // Test linear formats that should have gamma = 1.0
    let linear_formats = vec![
        TextureFormat::Rgba8Unorm,
        TextureFormat::Bgra8Unorm,
        TextureFormat::R8Unorm,
        TextureFormat::Rg8Unorm,
    ];

    for format in linear_formats {
        let gamma = Uniforms::gamma_for_format(format);
        assert_eq!(
            gamma, 1.0,
            "Linear format {:?} should have gamma 1.0",
            format
        );
    }
}

/// Test that orthographic matrix calculation matches C++ implementation
#[test]
fn test_orthographic_matrix() {
    let display_pos = [10.0, 20.0];
    let display_size = [800.0, 600.0];

    let matrix = Uniforms::create_orthographic_matrix(display_pos, display_size);

    // Expected values based on C++ implementation:
    // L = 10.0, R = 810.0, T = 20.0, B = 620.0
    let l = display_pos[0];
    let r = display_pos[0] + display_size[0];
    let t = display_pos[1];
    let b = display_pos[1] + display_size[1];

    // Check matrix values match C++ calculation
    assert_eq!(matrix[0][0], 2.0 / (r - l));
    assert_eq!(matrix[1][1], 2.0 / (t - b));
    assert_eq!(matrix[2][2], 0.5);
    assert_eq!(matrix[3][0], (r + l) / (l - r));
    assert_eq!(matrix[3][1], (t + b) / (b - t));
    assert_eq!(matrix[3][2], 0.5);
    assert_eq!(matrix[3][3], 1.0);
}

/// Test that renderer can be created with default values
#[test]
fn test_renderer_creation() {
    let renderer = WgpuRenderer::new();
    assert!(!renderer.is_initialized());

    // Test default creation
    let default_renderer = WgpuRenderer::default();
    assert!(!default_renderer.is_initialized());
}

/// Test uniforms structure size and alignment
#[test]
fn test_uniforms_layout() {
    use std::mem::{align_of, size_of};

    // Ensure uniforms structure has proper size and alignment for GPU usage
    assert_eq!(size_of::<Uniforms>(), 80); // 4x4 f32 matrix (64 bytes) + f32 gamma (4 bytes) + padding (12 bytes)
    assert_eq!(align_of::<Uniforms>(), 4); // f32 alignment

    let uniforms = Uniforms::new();

    // Test default values
    assert_eq!(uniforms.gamma, 1.0);
    assert_eq!(uniforms.mvp[0][0], 1.0); // Identity matrix
    assert_eq!(uniforms.mvp[1][1], 1.0);
    assert_eq!(uniforms.mvp[2][2], 1.0);
    assert_eq!(uniforms.mvp[3][3], 1.0);
}
