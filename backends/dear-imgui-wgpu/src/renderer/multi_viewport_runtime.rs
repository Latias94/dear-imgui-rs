//! Shared owning WGPU multi-viewport renderer runtime.

mod callbacks;
mod registry;
mod runtime;
mod surface;

#[cfg(test)]
mod tests;

pub(crate) use runtime::OwningViewportRuntime;
pub use runtime::{WgpuViewportAttachError, WgpuViewportError};

fn logical_size_to_framebuffer(size: [f32; 2], scale: [f32; 2]) -> [u32; 2] {
    [
        physical_dimension(size[0], valid_scale(scale[0])),
        physical_dimension(size[1], valid_scale(scale[1])),
    ]
}

fn valid_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn physical_dimension(logical: f32, scale: f32) -> u32 {
    if !logical.is_finite() {
        return 1;
    }
    (logical * scale).max(1.0).round().min(u32::MAX as f32) as u32
}

#[cfg(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
compile_error!(
    "Features `multi-viewport-winit` and `multi-viewport-sdl3` are mutually exclusive; enable only one."
);

#[cfg(all(not(feature = "multi-viewport-winit"), feature = "multi-viewport-sdl3"))]
use super::multi_viewport_sdl3_adapter as platform_adapter;
#[cfg(feature = "multi-viewport-winit")]
use super::multi_viewport_winit_adapter as platform_adapter;
