//! Texture management for the WGPU renderer
//!
//! This module handles texture creation, updates, and management,
//! integrating with Dear ImGui's modern texture system.

mod cache;
mod cleanup;
mod manager;
mod resource;
mod result;
#[cfg(test)]
mod tests;
mod upload;

use crate::{RenderResources, RendererError, RendererResult};
use dear_imgui_rs::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::collections::HashMap;
use wgpu::*;

pub use resource::WgpuTexture;
pub use result::TextureUpdateResult;

pub use manager::WgpuTextureManager;
