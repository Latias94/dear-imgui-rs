//! WGPU backend for Dear ImGui
//!
//! This crate provides a WGPU-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the WGPU graphics API.
//!
//! # Features
//!
//! - **Graphics API**: WGPU 30 on native targets and WebAssembly
//! - **Diagnostics**: `tracing` emits renderer debug and warning events and is off by default
//! - **Managed textures**: pointer-free create/update/destroy requests owned by rendered frames
//! - **External textures**: Register application-owned `wgpu::TextureView` handles for UI display
//! - **Gamma correction**: Automatic sRGB format detection and gamma correction
//! - **Isolated uploads**: Vertex, index, and uniform buffers are reused only after their
//!   command buffers finish or are discarded
//! - **Device object management**: Invalidate and rebuild renderer objects on the same live device;
//!   actual device loss requires a replacement renderer
//! - **Multi-viewport support**: Support for multiple windows (feature-gated via `multi-viewport-winit` for winit or `multi-viewport-sdl3` for SDL3 on native targets)
//!
//! # Example
//!
//! ```rust,no_run
//! use dear_imgui_rs::Context;
//! use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer, wgpu};
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
//! // let ui = imgui.frame();
//! // ... build your UI ...
//! // let frame = imgui.render(renderer.renderer_consumer()?);
//! // let extent = FramebufferExtent::from_texture(&surface_texture.texture);
//! // renderer.render(frame, &mut render_pass, extent)?;
//! # Ok(())
//! # }
//! ```

pub extern crate wgpu;

// Module declarations
mod data;
mod error;
mod frame_resources;
mod observation;
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
pub use observation::{
    WgpuDrawRecording, WgpuFrameOutcome, WgpuFrameScope, WgpuFrameSkipReason, WgpuFrameStage,
    WgpuViewportObservation,
};
pub use renderer::{FramebufferExtent, WgpuRenderer};
pub use renderer::{
    WgpuPreparedSnapshot, WgpuSnapshotError, WgpuSnapshotExecutor, WgpuSnapshotRenderer,
    WgpuSnapshotReport, WgpuSnapshotSubmission, WgpuSnapshotTarget, WgpuSnapshotTargetRecording,
};
pub use texture::{ExternalTextureBinding, ExternalTextureId, WgpuDrawDependencies};

pub(crate) use data::{WgpuBackendData, WgpuRenderStateStorage};
pub(crate) use frame_resources::{FrameResourceArena, FrameResources};
pub(crate) use render_resources::RenderResources;
pub(crate) use shaders::ShaderManager;
pub(crate) use texture::WgpuTextureManager;
pub(crate) use uniforms::{UniformBuffer, Uniforms};

// Re-export multi-viewport helpers when enabled
#[cfg(feature = "multi-viewport-winit")]
pub use renderer::multi_viewport;
#[cfg(feature = "multi-viewport-sdl3")]
pub use renderer::multi_viewport_sdl3;

/// Gamma correction mode for the WGPU renderer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GammaMode {
    /// Decode vertex RGB for sRGB render targets and pass it through for linear targets.
    Auto,
    /// Force linear output (gamma = 1.0)
    Linear,
    /// Apply a power of 2.2 to interpolated vertex RGB.
    Gamma22,
    /// Decode interpolated vertex RGB with the IEC sRGB curve before texture modulation.
    /// This mode applies to any target format. Texture views determine texture sample decoding.
    Srgb,
}
