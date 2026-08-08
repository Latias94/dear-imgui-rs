use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_texture_id() -> RendererResult<TextureId> {
    let id = NEXT_TEXTURE_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| RendererError::TextureIdExhausted)?;
    Ok(TextureId::new(id))
}

impl WgpuTextureManager {
    pub(crate) fn new() -> Self {
        Self {
            external_views: HashMap::new(),
            managed_textures: HashMap::new(),
            managed_by_texture_id: HashMap::new(),
            destroyed_managed_textures: HashMap::new(),
        }
    }

    pub(crate) fn register_external_view(
        &mut self,
        view: &TextureView,
    ) -> RendererResult<ExternalTextureId> {
        let id = allocate_texture_id()?;
        self.external_views.insert(id, view.clone());
        Ok(ExternalTextureId::new(id))
    }

    pub(crate) fn update_external_view(
        &mut self,
        texture: ExternalTextureId,
        view: &TextureView,
    ) -> RendererResult<()> {
        let texture_id = texture.texture_id();
        let registered = self
            .external_views
            .get_mut(&texture_id)
            .ok_or(RendererError::ExternalTextureNotFound(texture_id))?;
        *registered = view.clone();
        Ok(())
    }

    pub(crate) fn remove_external_view(
        &mut self,
        texture: ExternalTextureId,
    ) -> RendererResult<()> {
        let texture_id = texture.texture_id();
        self.external_views
            .remove(&texture_id)
            .map(drop)
            .ok_or(RendererError::ExternalTextureNotFound(texture_id))
    }

    pub(super) fn allocate_managed_texture_id(&self) -> RendererResult<TextureId> {
        allocate_texture_id()
    }

    pub(crate) fn texture_view(&self, id: TextureId) -> Option<&TextureView> {
        self.external_views.get(&id).or_else(|| {
            let managed = self.managed_by_texture_id.get(&id)?;
            self.managed_textures
                .get(managed)
                .map(|entry| entry.resource.view())
        })
    }

    #[cfg(test)]
    pub(crate) fn contains_texture(&self, id: TextureId) -> bool {
        self.texture_view(id).is_some()
    }

    pub(crate) fn clear_external_views(&mut self) {
        self.external_views.clear();
    }

    pub(crate) fn clear_managed_textures(&mut self) {
        self.managed_textures.clear();
        self.managed_by_texture_id.clear();
    }

    pub(super) fn managed_texture_id(&self, id: SnapshotTextureId) -> Option<TextureId> {
        self.managed_textures.get(&id).map(|entry| entry.texture_id)
    }

    pub(crate) fn clear_destroyed_managed_textures(&mut self) {
        self.destroyed_managed_textures.clear();
    }

    pub(crate) fn prune_destroyed_managed_textures(&mut self, completion_watermark: u64) {
        self.destroyed_managed_textures
            .retain(|_, destroy_epoch| *destroy_epoch > completion_watermark);
    }

    #[cfg(test)]
    pub(super) fn managed_texture_count(&self) -> usize {
        self.managed_textures.len()
    }

    #[cfg(test)]
    pub(super) fn texture_count(&self) -> usize {
        self.external_views.len() + self.managed_textures.len()
    }

    #[cfg(test)]
    pub(super) fn destroyed_managed_texture_count(&self) -> usize {
        self.destroyed_managed_textures.len()
    }
}
