//! WGPU backend for Dear ImGui
//!
//! This crate provides a WGPU-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the WGPU graphics API.
//!
//! # Features
//!
//! - **WGPU version selection**: choose exactly one of:
//!   - `wgpu-30` (default)
//!   - `wgpu-29`
//!   - `wgpu-28`
//!   - `wgpu-27` (for ecosystems pinned to wgpu 27.x, e.g. some Bevy version trains)
//! - **Diagnostics**: `tracing` emits renderer debug and warning events and is off by default
//! - **Managed textures**: pointer-free create/update/destroy requests owned by rendered frames
//! - **External textures**: Register application-owned `wgpu::TextureView` handles for UI display
//! - **Gamma correction**: Automatic sRGB format detection and gamma correction
//! - **Epoch-isolated uploads**: Vertex, index, and uniform buffers are never reused across
//!   render epochs
//! - **Device object management**: Helpers to recreate device objects (pipelines/buffers/textures) after loss
//! - **Multi-viewport support**: Support for multiple windows (feature-gated via `multi-viewport-winit` for winit or `multi-viewport-sdl3` for SDL3 on native targets)
//!
//! # Example
//!
//! ```rust,no_run
//! use dear_imgui_rs::Context;
//! use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo, wgpu};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let (device, queue) = todo!("initialize a WGPU Device/Queue");
//!
//! // Create Dear ImGui context
//! let mut imgui = Context::create();
//!
//! // Create renderer (recommended path)
//! let init_info = WgpuInitInfo::new(device, queue, wgpu::TextureFormat::Bgra8UnormSrgb);
//! let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;
//!
//! // In your render loop:
//! // imgui.new_frame();
//! // ... build your UI ...
//! // let frame = imgui.render();
//! // renderer.render(frame, &mut render_pass)?;
//! # Ok(())
//! # }
//! ```

// Select a single wgpu version via features (default: wgpu-30).
//
// We keep the public API surface using `wgpu::*` types, but allow downstream crates to opt into a
// specific major version to better match their ecosystem (e.g. Bevy).
#[cfg(all(
    feature = "wgpu-27",
    any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30")
))]
compile_error!(
    "Features `wgpu-27`, `wgpu-28`, `wgpu-29`, and `wgpu-30` are mutually exclusive; enable only one."
);
#[cfg(all(feature = "wgpu-28", any(feature = "wgpu-29", feature = "wgpu-30")))]
compile_error!(
    "Features `wgpu-27`, `wgpu-28`, `wgpu-29`, and `wgpu-30` are mutually exclusive; enable only one."
);
#[cfg(all(feature = "wgpu-29", feature = "wgpu-30"))]
compile_error!(
    "Features `wgpu-27`, `wgpu-28`, `wgpu-29`, and `wgpu-30` are mutually exclusive; enable only one."
);
#[cfg(not(any(
    feature = "wgpu-27",
    feature = "wgpu-28",
    feature = "wgpu-29",
    feature = "wgpu-30"
)))]
compile_error!(
    "Either feature `wgpu-27`, `wgpu-28`, `wgpu-29`, or `wgpu-30` must be enabled for dear-imgui-wgpu."
);

#[cfg(all(feature = "wgpu-27", feature = "webgl"))]
compile_error!(
    "Feature `webgl` selects the wgpu-30 WebGL route; use `webgl-wgpu27` with `wgpu-27`."
);
#[cfg(all(feature = "wgpu-27", feature = "webgpu"))]
compile_error!(
    "Feature `webgpu` selects the wgpu-30 WebGPU route; use `webgpu-wgpu27` with `wgpu-27`."
);
#[cfg(all(feature = "wgpu-28", feature = "webgl"))]
compile_error!(
    "Feature `webgl` selects the wgpu-30 WebGL route; use `webgl-wgpu28` with `wgpu-28`."
);
#[cfg(all(feature = "wgpu-28", feature = "webgpu"))]
compile_error!(
    "Feature `webgpu` selects the wgpu-30 WebGPU route; use `webgpu-wgpu28` with `wgpu-28`."
);
#[cfg(all(feature = "wgpu-29", feature = "webgl"))]
compile_error!(
    "Feature `webgl` selects the wgpu-30 WebGL route; use `webgl-wgpu29` with `wgpu-29`."
);
#[cfg(all(feature = "wgpu-29", feature = "webgpu"))]
compile_error!(
    "Feature `webgpu` selects the wgpu-30 WebGPU route; use `webgpu-wgpu29` with `wgpu-29`."
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
#[cfg(all(feature = "wgpu-29", feature = "webgl-wgpu27"))]
compile_error!(
    "Feature `webgl-wgpu27` is incompatible with `wgpu-29` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-29", feature = "webgpu-wgpu27"))]
compile_error!(
    "Feature `webgpu-wgpu27` is incompatible with `wgpu-29` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-29", feature = "webgl-wgpu28"))]
compile_error!(
    "Feature `webgl-wgpu28` is incompatible with `wgpu-29` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-29", feature = "webgpu-wgpu28"))]
compile_error!(
    "Feature `webgpu-wgpu28` is incompatible with `wgpu-29` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-30", feature = "webgl-wgpu27"))]
compile_error!(
    "Feature `webgl-wgpu27` is incompatible with `wgpu-30` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-30", feature = "webgpu-wgpu27"))]
compile_error!(
    "Feature `webgpu-wgpu27` is incompatible with `wgpu-30` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-30", feature = "webgl-wgpu28"))]
compile_error!(
    "Feature `webgl-wgpu28` is incompatible with `wgpu-30` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-30", feature = "webgpu-wgpu28"))]
compile_error!(
    "Feature `webgpu-wgpu28` is incompatible with `wgpu-30` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-30", feature = "webgl-wgpu29"))]
compile_error!(
    "Feature `webgl-wgpu29` is incompatible with `wgpu-30` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-30", feature = "webgpu-wgpu29"))]
compile_error!(
    "Feature `webgpu-wgpu29` is incompatible with `wgpu-30` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-27", feature = "webgl-wgpu29"))]
compile_error!(
    "Feature `webgl-wgpu29` is incompatible with `wgpu-27` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-27", feature = "webgpu-wgpu29"))]
compile_error!(
    "Feature `webgpu-wgpu29` is incompatible with `wgpu-27` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-28", feature = "webgl-wgpu29"))]
compile_error!(
    "Feature `webgl-wgpu29` is incompatible with `wgpu-28` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-28", feature = "webgpu-wgpu29"))]
compile_error!(
    "Feature `webgpu-wgpu29` is incompatible with `wgpu-28` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-27", feature = "webgl-wgpu30"))]
compile_error!(
    "Feature `webgl-wgpu30` is incompatible with `wgpu-27` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-27", feature = "webgpu-wgpu30"))]
compile_error!(
    "Feature `webgpu-wgpu30` is incompatible with `wgpu-27` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-28", feature = "webgl-wgpu30"))]
compile_error!(
    "Feature `webgl-wgpu30` is incompatible with `wgpu-28` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-28", feature = "webgpu-wgpu30"))]
compile_error!(
    "Feature `webgpu-wgpu30` is incompatible with `wgpu-28` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-29", feature = "webgl-wgpu30"))]
compile_error!(
    "Feature `webgl-wgpu30` is incompatible with `wgpu-29` (would pull multiple wgpu majors)."
);
#[cfg(all(feature = "wgpu-29", feature = "webgpu-wgpu30"))]
compile_error!(
    "Feature `webgpu-wgpu30` is incompatible with `wgpu-29` (would pull multiple wgpu majors)."
);

#[cfg(feature = "wgpu-27")]
pub extern crate wgpu27 as wgpu;
#[cfg(feature = "wgpu-28")]
pub extern crate wgpu28 as wgpu;
#[cfg(feature = "wgpu-29")]
pub extern crate wgpu29 as wgpu;
#[cfg(feature = "wgpu-30")]
pub extern crate wgpu30 as wgpu;

#[cfg(feature = "tracing")]
macro_rules! backend_debug {
    ($($arg:tt)*) => { tracing::debug!($($arg)*); };
}

#[cfg(not(feature = "tracing"))]
macro_rules! backend_debug {
    ($($arg:tt)*) => {};
}

// Module declarations
mod data;
mod error;
mod frame_resources;
mod render_resources;
mod renderer;
mod shaders;
mod texture;
mod uniforms;

#[cfg(doctest)]
mod removed_public_contracts {
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::empty;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::new_without_font_atlas;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::init_with_context;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::default();
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::is_initialized;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::texture_manager;
    /// ```
    struct TwoPhaseRendererInitialization;

    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::new_frame;
    /// ```
    struct ManualFramePreparation;

    /// ```compile_fail
    /// use dear_imgui_wgpu::FrameResources;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::RenderResources;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::ShaderManager;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::UniformBuffer;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::Uniforms;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuTextureManager;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuTexture;
    /// ```
    struct RendererInternals;

    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::register_external_texture_with_sampler;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::update_external_texture_sampler;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::update_external_texture_view;
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_wgpu::WgpuRenderer;
    /// let _ = WgpuRenderer::unregister_texture;
    /// ```
    struct PerTextureSamplers;
}

pub use data::{
    WgpuInitInfo, WgpuRenderState, WgpuRenderStateAccessError, WgpuViewportSurfaceConfig,
};
pub use error::{RendererError, RendererResult};
pub use renderer::WgpuRenderer;
pub use texture::ExternalTextureId;

pub(crate) use data::{WgpuBackendData, WgpuRenderStateStorage};
pub(crate) use frame_resources::{FrameResourceArena, FrameResources};
pub(crate) use render_resources::RenderResources;
pub(crate) use shaders::ShaderManager;
pub(crate) use texture::{OwnedWgpuTexture, WgpuTextureManager};
pub(crate) use uniforms::{UniformBuffer, Uniforms};

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
