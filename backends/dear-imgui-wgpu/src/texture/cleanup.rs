use super::resource::ManagedWgpuTexture;
use super::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum ManagedRequestOutcome {
    Uploaded(TextureId),
    Destroyed,
    IgnoredRetired,
}

impl WgpuTextureManager {
    /// Destroy a texture by its renderer-facing ID.
    pub fn destroy_texture_by_id(&mut self, id: TextureId) {
        self.remove_texture(id);
    }

    /// Destroy a texture by its renderer-facing ID.
    pub fn destroy_texture(&mut self, texture_id: TextureId) {
        self.remove_texture(texture_id);
    }

    pub(crate) fn handle_texture_requests(
        &mut self,
        requests: &[TextureRequest],
        device: &Device,
        queue: &Queue,
        render_resources: &mut RenderResources,
    ) -> RendererResult<Vec<TextureFeedback>> {
        let mut feedback = Vec::with_capacity(requests.len());
        for request in requests {
            match self.apply_managed_request(
                request.texture(),
                request.operation(),
                device,
                queue,
                render_resources,
            )? {
                ManagedRequestOutcome::Uploaded(texture_id) => {
                    feedback.push(request.uploaded(texture_id)?);
                }
                ManagedRequestOutcome::Destroyed => feedback.push(request.destroyed()?),
                ManagedRequestOutcome::IgnoredRetired => {}
            }
        }
        Ok(feedback)
    }

    pub(super) fn apply_managed_request(
        &mut self,
        id: SnapshotTextureId,
        operation: &TextureOp,
        device: &Device,
        queue: &Queue,
        render_resources: &mut RenderResources,
    ) -> RendererResult<ManagedRequestOutcome> {
        match operation {
            TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => {
                if self.destroyed_managed_textures.contains(&id) {
                    // A delayed create must not resurrect a resource after its destroy request.
                    return Ok(ManagedRequestOutcome::IgnoredRetired);
                }
                let texture_id = if let Some(existing) = self.managed_textures.get(&id) {
                    let texture_id = existing.texture_id;
                    self.upload_managed_texture_contents(
                        queue,
                        id,
                        *format,
                        [*width, *height],
                        *row_pitch,
                        pixels,
                    )?;
                    texture_id
                } else {
                    let resource = Self::create_managed_texture_resource(
                        device, queue, *format, *width, *height, *row_pitch, pixels,
                    )?;
                    let texture_id = self.allocate_texture_id();
                    self.managed_textures.insert(
                        id,
                        ManagedWgpuTexture {
                            texture_id,
                            width: *width,
                            height: *height,
                            resource,
                        },
                    );
                    self.managed_by_texture_id.insert(texture_id, id);
                    texture_id
                };
                Ok(ManagedRequestOutcome::Uploaded(texture_id))
            }
            TextureOp::Update {
                format,
                width,
                height,
                rects,
            } => {
                if self.destroyed_managed_textures.contains(&id) {
                    return Ok(ManagedRequestOutcome::IgnoredRetired);
                }
                self.update_managed_texture(queue, id, *format, *width, *height, rects)?;
                let texture_id = self
                    .managed_texture_id(id)
                    .ok_or(RendererError::ManagedTextureMissing(id))?;
                Ok(ManagedRequestOutcome::Uploaded(texture_id))
            }
            TextureOp::Destroy => {
                self.destroyed_managed_textures.insert(id);
                if let Some(entry) = self.managed_textures.remove(&id) {
                    self.managed_by_texture_id.remove(&entry.texture_id);
                    self.clear_custom_sampler_for_texture(entry.texture_id);
                    render_resources.remove_image_bind_group(entry.texture_id);
                }
                // Repeated destroy requests are intentionally acknowledged even after the GPU
                // resource is gone. The Context may be retrying feedback from an abandoned epoch.
                Ok(ManagedRequestOutcome::Destroyed)
            }
        }
    }
}
