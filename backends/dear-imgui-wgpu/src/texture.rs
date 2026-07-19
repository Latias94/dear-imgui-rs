//! Texture management for the WGPU renderer
//!
//! Application-owned textures retain their legacy [`TextureId`] lookup path. Context-owned
//! textures are keyed by pointer-free [`SnapshotTextureId`] values and are only changed by owned
//! renderer requests.

mod cache;
mod cleanup;
mod manager;
mod resource;
#[cfg(test)]
mod tests;
mod upload;

use crate::{RenderResources, RendererError, RendererResult};
use dear_imgui_rs::{
    TextureId,
    render::{SnapshotTextureId, TextureFeedback, TextureOp, TextureRequest, TextureUploadRect},
    texture::{TextureFormat as ImGuiTextureFormat, TextureRect},
};
use std::collections::{HashMap, HashSet};
use wgpu::*;

pub use resource::WgpuTexture;

pub use manager::WgpuTextureManager;
