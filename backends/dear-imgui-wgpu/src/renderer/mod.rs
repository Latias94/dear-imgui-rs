//! Main WGPU renderer implementation
//!
//! This module contains the main WgpuRenderer struct and its implementation,
//! following the pattern from imgui_impl_wgpu.cpp
//!
//! Managed texture flow (Dear ImGui 1.92+):
//! - `Context::render()` returns a Context-borrowed `RenderedFrame` with owned texture requests.
//! - WGPU stores managed GPU resources by pointer-free `SnapshotTextureId`.
//! - The renderer reconciles request-bound feedback before reading draw commands.
//! - Legacy application textures continue to use `TextureId` without entering this protocol.

mod callbacks;
mod core;
mod init;
mod lifecycle;
mod render;
mod texture_api;

mod draw;
mod external_textures;
mod font_atlas;
#[cfg(feature = "multi-viewport-winit")]
pub mod multi_viewport;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
mod multi_viewport_runtime;
#[cfg(feature = "multi-viewport-sdl3")]
pub mod multi_viewport_sdl3;
#[cfg(feature = "multi-viewport-sdl3")]
mod multi_viewport_sdl3_adapter;
#[cfg(feature = "multi-viewport-winit")]
mod multi_viewport_winit_adapter;
mod pipeline;
#[cfg(feature = "multi-viewport-sdl3")]
mod sdl3_raw_window_handle;

use crate::{RendererError, RendererResult, Uniforms, WgpuBackendData, WgpuTextureManager};
pub use core::WgpuRenderer;
use dear_imgui_rs::render::{RendererRenderStateGuard, RendererRenderStateGuardError};

pub(super) fn map_renderer_render_state_error(
    error: RendererRenderStateGuardError,
) -> RendererError {
    match error {
        RendererRenderStateGuardError::MissingPlatformIo => RendererError::InvalidRenderState(
            "PlatformIO not available for renderer render state".to_owned(),
        ),
        RendererRenderStateGuardError::AlreadyOccupied => RendererError::InvalidRenderState(
            "PlatformIO Renderer_RenderState is already occupied".to_owned(),
        ),
        RendererRenderStateGuardError::Drift => RendererError::RendererStateDrift {
            field: "Renderer_RenderState",
        },
    }
}
