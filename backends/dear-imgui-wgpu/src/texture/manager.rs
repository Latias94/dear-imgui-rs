use super::resource::ManagedWgpuTexture;
use super::*;

/// Texture manager for WGPU renderer
///
/// This manages the mapping between Dear ImGui texture IDs and WGPU textures,
/// similar to the ImageBindGroups storage in the C++ implementation.
#[derive(Debug)]
pub struct WgpuTextureManager {
    /// Application-owned textures addressed through the legacy TextureId path.
    pub(super) textures: HashMap<TextureId, WgpuTexture>,
    /// Context-owned textures addressed by the pointer-free snapshot protocol.
    pub(super) managed_textures: HashMap<SnapshotTextureId, ManagedWgpuTexture>,
    /// Renderer IDs written back to draw commands, mapped to their managed owners.
    pub(super) managed_by_texture_id: HashMap<TextureId, SnapshotTextureId>,
    /// Managed identities sealed by Destroy, paired with their latest request epoch.
    pub(super) destroyed_managed_textures: HashMap<SnapshotTextureId, u64>,
    /// Next available texture ID
    pub(super) next_id: u64,
    /// Custom samplers registered for external textures (sampler_id -> sampler)
    pub(super) custom_samplers: HashMap<u64, Sampler>,
    /// Mapping from texture_id -> sampler_id for per-texture custom sampling
    pub(super) custom_sampler_by_texture: HashMap<TextureId, u64>,
    /// Cached common bind groups (uniform buffer + sampler) per sampler_id
    pub(super) common_bind_groups: HashMap<u64, BindGroup>,
    /// Next available sampler ID
    pub(super) next_sampler_id: u64,
}

impl Default for WgpuTextureManager {
    fn default() -> Self {
        Self::new()
    }
}
