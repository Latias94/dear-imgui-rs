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
use std::{collections::HashMap, num::NonZeroU64};
use wgpu::*;

pub(crate) use cleanup::ManagedRequestOutcome;
pub(crate) use manager::WgpuTextureManager;
pub(crate) use resource::OwnedWgpuTexture;
#[cfg(test)]
pub(crate) use tests::{external_test_texture, request_test_device};

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

/// One version of the WGPU view bound to an external texture registration.
///
/// Registration starts at revision one. Every successful view replacement advances the revision,
/// including replacement with the same view handle. A binding is owned metadata: retaining it
/// neither keeps a registration alive nor owns a GPU resource. The revision describes the view
/// mapping, not the application's texture contents or engine frame number.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExternalTextureBinding {
    /// The external registration used by Dear ImGui draw commands.
    pub texture: ExternalTextureId,
    /// The nonzero view-mapping revision observed when the binding was captured.
    pub revision: NonZeroU64,
}

/// External texture dependencies observed while preparing executable draw commands.
///
/// Bindings appear once, in first-use order, after index validation, framebuffer clipping, and
/// image-bind-group preparation succeed. Zero-count and fully clipped commands contribute no
/// bindings. Managed textures and the renderer's default texture are not external dependencies.
/// This metadata describes prepared commands; recording, submission, and presentation must be
/// tracked separately.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WgpuDrawDependencies {
    /// External view mappings used by prepared element commands.
    pub external_bindings: Vec<ExternalTextureBinding>,
    /// Whether a non-standard raw callback may use additional, unobservable GPU resources.
    ///
    /// Standard reset and sampler-selection callbacks do not make the dependency list incomplete.
    pub contains_unknown_callback_dependencies: bool,
}
