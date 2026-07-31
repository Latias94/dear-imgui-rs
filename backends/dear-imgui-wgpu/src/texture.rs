//! Texture management for the WGPU renderer
//!
//! Application-owned texture views use opaque [`ExternalTextureId`] handles. Context-owned
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
use std::collections::HashMap;
use wgpu::*;

pub(crate) use manager::WgpuTextureManager;
pub(crate) use resource::OwnedWgpuTexture;

/// Opaque handle for an application-owned WGPU texture view registered with a renderer.
///
/// The handle can be passed to Dear ImGui through [`Self::texture_id`], but cannot be forged from
/// an arbitrary [`TextureId`]. Registration owns a clone of the WGPU view handle; the application
/// remains responsible for the texture contents and must not explicitly destroy the underlying
/// GPU resource while it is registered.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct ExternalTextureId(TextureId);

impl ExternalTextureId {
    pub(super) const fn new(texture_id: TextureId) -> Self {
        Self(texture_id)
    }

    /// Returns the Dear ImGui texture identifier used by image widgets and draw-list commands.
    #[must_use]
    pub const fn texture_id(self) -> TextureId {
        self.0
    }
}

impl From<ExternalTextureId> for TextureId {
    fn from(texture: ExternalTextureId) -> Self {
        texture.texture_id()
    }
}
