use super::*;
use std::sync::Arc;

/// Renderer-owned WGPU texture resource.
#[derive(Debug)]
pub(crate) struct OwnedWgpuTexture {
    pub(super) texture: Texture,
    pub(super) texture_view: TextureView,
    recorded_reads: Arc<()>,
}

impl OwnedWgpuTexture {
    pub(crate) fn new(texture: Texture, texture_view: TextureView) -> Self {
        Self {
            texture,
            texture_view,
            recorded_reads: Arc::new(()),
        }
    }

    pub(crate) fn view(&self) -> &TextureView {
        &self.texture_view
    }

    pub(crate) fn texture(&self) -> &Texture {
        &self.texture
    }

    pub(super) fn recording_lease(&self) -> Arc<()> {
        Arc::clone(&self.recorded_reads)
    }

    pub(super) fn can_write_in_place(&mut self) -> bool {
        Arc::get_mut(&mut self.recorded_reads).is_some()
    }
}

#[derive(Debug)]
pub(super) struct ManagedWgpuTexture {
    pub(super) texture_id: TextureId,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) resource: OwnedWgpuTexture,
    pub(super) rgba_pixels: Vec<u8>,
}
