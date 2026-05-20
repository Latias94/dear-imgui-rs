use super::result::mark_texture_destroyed;
use super::*;

impl WgpuTextureManager {
    /// Destroy a texture by ID
    pub fn destroy_texture_by_id(&mut self, id: TextureId) {
        self.remove_texture(id);
    }

    /// Destroy a texture
    pub fn destroy_texture(&mut self, texture_id: TextureId) {
        self.remove_texture(texture_id);
        // WGPU textures are automatically cleaned up when dropped
    }

    /// Handle texture updates from Dear ImGui draw data
    ///
    /// This iterates `DrawData::textures_mut()` and applies create/update/destroy requests.
    /// For `WantCreate`, we create the GPU texture, then write the generated id back into
    /// the `ImTextureData` via `set_tex_id()` and mark status `OK` (matching C++ backend).
    /// For `WantUpdates`, if a valid id is not yet assigned (first use), we create now and
    /// assign the id; otherwise we update in place. When textures are recreated or destroyed,
    /// the corresponding cached bind groups in `RenderResources` are invalidated so that
    /// subsequent draws will see the updated views.
    pub fn handle_texture_updates(
        &mut self,
        draw_data: &mut dear_imgui_rs::render::DrawData,
        device: &Device,
        queue: &Queue,
        render_resources: &mut RenderResources,
    ) {
        let mut textures = draw_data.textures_mut();
        while let Some(mut texture_data) = textures.next() {
            let status = texture_data.status();
            let current_tex_id = texture_data.tex_id();

            match status {
                TextureStatus::WantCreate => {
                    // Create and upload new texture to graphics system
                    // Following the official imgui_impl_wgpu.cpp implementation

                    // If ImGui already had a TexID associated, drop any stale bind group
                    // so that a new one is created the first time we render with it.
                    if !current_tex_id.is_null() {
                        render_resources.remove_image_bind_group(current_tex_id);
                    }

                    match self.create_texture_from_data(device, queue, &*texture_data) {
                        Ok(wgpu_texture_id) => {
                            // CRITICAL: Set the texture ID back to Dear ImGui
                            // In the C++ implementation, they use the TextureView pointer as ImTextureID.
                            // In Rust, we can't get the raw pointer, so we use our internal texture ID.
                            // This works because our renderer will map the texture ID to the WGPU texture.
                            texture_data.set_tex_id(wgpu_texture_id);

                            // Mark texture as ready
                            texture_data.set_status(TextureStatus::OK);
                        }
                        Err(e) => {
                            println!(
                                "Failed to create texture for ID: {:?}, error: {}",
                                current_tex_id, e
                            );
                        }
                    }
                }
                TextureStatus::WantUpdates => {
                    let imgui_tex_id = texture_data.tex_id();
                    let internal_id = imgui_tex_id;

                    // If we don't have a valid texture id yet (first update) or the
                    // id isn't registered, create it now and write back the TexID,
                    // so this frame (or the next one) can bind the correct texture.
                    if internal_id.is_null() || !self.contains_texture(internal_id) {
                        match self.create_texture_from_data(device, queue, &*texture_data) {
                            Ok(new_id) => {
                                texture_data.set_tex_id(new_id);
                                texture_data.set_status(TextureStatus::OK);
                            }
                            Err(_e) => {
                                // Leave it destroyed to avoid retry storm; user can request create again
                                texture_data.set_status(TextureStatus::Destroyed);
                            }
                        }
                    } else {
                        // We are about to update/recreate an existing texture. Invalidate
                        // any cached bind group so it will be rebuilt with the new view.
                        render_resources.remove_image_bind_group(internal_id);

                        // Try in-place sub-rect updates first
                        if self
                            .apply_subrect_updates(queue, &*texture_data, internal_id)
                            .unwrap_or(false)
                        {
                            texture_data.set_status(TextureStatus::OK);
                        } else if self
                            .update_texture_from_data_with_id(
                                device,
                                queue,
                                &*texture_data,
                                internal_id,
                            )
                            .is_err()
                        {
                            // If update fails, keep the existing GPU texture and mark OK to avoid a retry storm.
                            // We cannot clear TexID here because draw commands in this frame may still reference it.
                            texture_data.set_status(TextureStatus::OK);
                        } else {
                            texture_data.set_status(TextureStatus::OK);
                        }
                    }
                }
                TextureStatus::WantDestroy => {
                    // Only destroy when unused frames > 0 (align with official backend behavior)
                    let mut can_destroy = true;
                    unsafe {
                        let raw = texture_data.as_raw();
                        if !raw.is_null() {
                            // If field not present in bindings on some versions, default true
                            #[allow(unused_unsafe)]
                            {
                                // Access UnusedFrames if available
                                // SAFETY: reading a plain field from raw C struct
                                can_destroy = (*raw).UnusedFrames > 0;
                            }
                        }
                    }
                    if can_destroy {
                        let imgui_tex_id = texture_data.tex_id();
                        let internal_id = imgui_tex_id;
                        // Remove from texture cache and any associated bind groups
                        self.remove_texture(internal_id);
                        self.clear_custom_sampler_for_texture(internal_id);
                        render_resources.remove_image_bind_group(internal_id);
                        mark_texture_destroyed(&mut texture_data);
                    }
                }
                TextureStatus::OK | TextureStatus::Destroyed => {
                    // No action needed
                }
            }
        }
    }
}
