use super::*;

/// Renderer-owned WGPU texture resource.
#[derive(Debug)]
pub(crate) struct OwnedWgpuTexture {
    pub(super) texture: Texture,
    pub(super) texture_view: TextureView,
}

impl OwnedWgpuTexture {
    pub(crate) fn new(texture: Texture, texture_view: TextureView) -> Self {
        Self {
            texture,
            texture_view,
        }
    }

    pub(crate) fn view(&self) -> &TextureView {
        &self.texture_view
    }

    pub(crate) fn texture(&self) -> &Texture {
        &self.texture
    }
}

#[derive(Debug)]
pub(super) struct ManagedWgpuTexture {
    pub(super) texture_id: TextureId,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) resource: OwnedWgpuTexture,
}
