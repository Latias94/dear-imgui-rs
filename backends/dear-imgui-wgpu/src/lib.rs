//! WGPU backend for Dear ImGui
//!
//! This crate provides a WGPU-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the WGPU graphics API.
//!
//! # Features
//!
//! - **WGPU version selection**: choose exactly one of:
//!   - `wgpu-28` (default)
//!   - `wgpu-27` (for ecosystems pinned to wgpu 27.x, e.g. some Bevy version trains)
//! - **Modern texture management**: Full integration with Dear ImGui's ImTextureData system
//! - **External textures**: Register existing `wgpu::Texture` resources for UI display,
//!   with optional per-texture custom samplers.
//! - **Gamma correction**: Automatic sRGB format detection and gamma correction
//! - **Multi-frame buffering**: Support for multiple frames in flight
//! - **Device object management**: Helpers to recreate device objects (pipelines/buffers/textures) after loss
//! - **Multi-viewport support**: Support for multiple windows (feature-gated via `multi-viewport-winit` for winit or `multi-viewport-sdl3` for SDL3 on native targets)
//!
//! # Example
//!
//! ```rust,no_run
//! use dear_imgui_rs::Context;
//! use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
//! use wgpu::*;
//!
//! // Initialize WGPU device and queue
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let instance = Instance::new(&InstanceDescriptor::default());
//! let adapter = instance.request_adapter(&RequestAdapterOptions::default()).await.unwrap();
//! let (device, queue) = adapter.request_device(&DeviceDescriptor::default()).await?;
//!
//! // Create Dear ImGui context
//! let mut imgui = Context::create();
//!
//! // Create renderer (recommended path)
//! let init_info = WgpuInitInfo::new(device, queue, TextureFormat::Bgra8UnormSrgb);
//! let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;
//!
//! // In your render loop:
//! // imgui.new_frame();
//! // ... build your UI ...
//! // let draw_data = imgui.render();
//! // renderer.render_draw_data(&draw_data, &mut render_pass)?;
//! # Ok(())
//! # }
//! ```

// Select a single wgpu version via features (default: wgpu-28).
//
// We keep the public API surface using `wgpu::*` types, but allow downstream crates to opt into a
// specific major version to better match their ecosystem (e.g. Bevy).
#[cfg(all(feature = "wgpu-27", feature = "wgpu-28"))]
compile_error!("Features `wgpu-27` and `wgpu-28` are mutually exclusive; enable only one.");
#[cfg(not(any(feature = "wgpu-27", feature = "wgpu-28")))]
compile_error!("Either feature `wgpu-27` or `wgpu-28` must be enabled for dear-imgui-wgpu.");

#[cfg(all(feature = "wgpu-27", feature = "webgl"))]
compile_error!(
    "Feature `webgl` selects the wgpu-28 WebGL route; use `webgl-wgpu27` with `wgpu-27`."
);
#[cfg(all(feature = "wgpu-27", feature = "webgpu"))]
compile_error!(
    "Feature `webgpu` selects the wgpu-28 WebGPU route; use `webgpu-wgpu27` with `wgpu-27`."
);
#[cfg(all(feature = "wgpu-28", feature = "webgl-wgpu27"))]
compile_error!(
    "Feature `webgl-wgpu27` is incompatible with `wgpu-28` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-28", feature = "webgpu-wgpu27"))]
compile_error!(
    "Feature `webgpu-wgpu27` is incompatible with `wgpu-28` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-27", feature = "webgl-wgpu28"))]
compile_error!(
    "Feature `webgl-wgpu28` is incompatible with `wgpu-27` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-27", feature = "webgpu-wgpu28"))]
compile_error!(
    "Feature `webgpu-wgpu28` is incompatible with `wgpu-27` (would pull multiple wgpu majors)."
);

#[cfg(feature = "wgpu-27")]
extern crate wgpu27 as wgpu;
#[cfg(feature = "wgpu-28")]
extern crate wgpu28 as wgpu;

// Module declarations
mod data;
mod error;
mod frame_resources;
mod render_resources;
mod renderer;
mod shaders;
mod texture;
mod uniforms;

// Re-exports
pub use data::*;
pub use error::*;
pub use frame_resources::*;
pub use render_resources::*;
pub use renderer::*;
pub use shaders::*;
pub use texture::*;
pub use uniforms::*;

// Re-export multi-viewport helpers when enabled
#[cfg(feature = "multi-viewport-winit")]
pub use renderer::multi_viewport;
#[cfg(feature = "multi-viewport-sdl3")]
pub use renderer::multi_viewport_sdl3;

/// Gamma correction mode for the WGPU renderer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GammaMode {
    /// Automatically pick gamma based on render target format (default)
    Auto,
    /// Force linear output (gamma = 1.0)
    Linear,
    /// Force gamma 2.2 curve (gamma = 2.2)
    Gamma22,
}
