use dear_imgui_backend_support::ManagedTextureTable;

use super::resource::ManagedWgpuTexture;
use super::*;

/// Renderer-private mapping between Dear ImGui identifiers and WGPU resources.
#[derive(Debug)]
pub(crate) struct WgpuTextureManager {
    /// Application-owned texture views. The application retains texture ownership.
    pub(super) external_views: HashMap<TextureId, ExternalTextureView>,
    /// Context-owned textures addressed by the pointer-free snapshot protocol.
    pub(super) managed_textures: ManagedTextureTable<ManagedWgpuTexture>,
    /// Renderer IDs written back to draw commands, mapped to their managed owners.
    pub(super) managed_by_texture_id: HashMap<TextureId, SnapshotTextureId>,
}

#[derive(Debug)]
pub(super) struct ExternalTextureView {
    pub(super) view: TextureView,
    pub(super) revision: NonZeroU64,
}

impl Default for WgpuTextureManager {
    fn default() -> Self {
        Self::new()
    }
}
