use super::*;

/// Texture manager for WGPU renderer
///
/// This manages the mapping between Dear ImGui texture IDs and WGPU textures,
/// similar to the ImageBindGroups storage in the C++ implementation.
#[derive(Debug)]
pub struct WgpuTextureManager {
    /// Map from texture ID to WGPU texture
    pub(super) textures: HashMap<TextureId, WgpuTexture>,
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
