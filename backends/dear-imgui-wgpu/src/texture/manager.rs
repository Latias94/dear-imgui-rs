use super::resource::ManagedWgpuTexture;
use super::*;

/// Renderer-private mapping between Dear ImGui identifiers and WGPU resources.
#[derive(Debug)]
pub(crate) struct WgpuTextureManager {
    /// Application-owned texture views. The application retains texture ownership.
    pub(super) external_views: HashMap<TextureId, TextureView>,
    /// Context-owned textures addressed by the pointer-free snapshot protocol.
    pub(super) managed_textures: HashMap<SnapshotTextureId, ManagedWgpuTexture>,
    /// Renderer IDs written back to draw commands, mapped to their managed owners.
    pub(super) managed_by_texture_id: HashMap<TextureId, SnapshotTextureId>,
    /// Managed identities sealed by Destroy, paired with their latest request epoch.
    pub(super) destroyed_managed_textures: HashMap<SnapshotTextureId, u64>,
}

impl Default for WgpuTextureManager {
    fn default() -> Self {
        Self::new()
    }
}
