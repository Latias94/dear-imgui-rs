//! Main WGPU renderer implementation
//!
//! This module contains the main WgpuRenderer struct and its implementation,
//! following the pattern from imgui_impl_wgpu.cpp
//!
//! Texture Updates Flow (ImGui 1.92+)
//! - During `Context::render()`, Dear ImGui emits a list of textures to be processed with
//!   `DrawData::textures_mut()` (see `dear_imgui_rs::render::DrawData::textures_mut`). Each item is
//!   an `ImTextureData*` with a `Status` field:
//!   - `WantCreate`: create a GPU texture, upload all pixels, set `TexID`, then set status `OK`.
//!   - `WantUpdates`: upload `UpdateRect` (and any queued rects) then set `OK`.
//!   - `WantDestroy`: schedule/destroy GPU texture; if unused for some frames, set `Destroyed`.
//! - This backend honors these transitions in its texture module; users can simply pass
//!   `&mut TextureData` to UI/draw calls and let the backend handle the rest.

mod callbacks;
mod core;
mod init;
mod lifecycle;
mod render;
mod state;
mod texture_api;

mod draw;
mod external_textures;
mod font_atlas;
#[cfg(feature = "multi-viewport-winit")]
pub mod multi_viewport;
#[cfg(feature = "multi-viewport-sdl3")]
pub mod multi_viewport_sdl3;
mod pipeline;
#[cfg(feature = "multi-viewport-sdl3")]
mod sdl3_raw_window_handle;

use crate::{RendererError, RendererResult, Uniforms, WgpuBackendData, WgpuTextureManager};
pub use core::WgpuRenderer;
use state::{ActiveSampler, RendererRenderStateGuard};
