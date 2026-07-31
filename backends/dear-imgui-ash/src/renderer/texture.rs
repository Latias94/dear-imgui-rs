use super::uploads::finish_managed_upload_gate;
use super::*;

#[derive(Debug)]
pub(super) struct VulkanTexture {
    pub(super) image: vk::Image,
    pub(super) image_mem: Memory,
    pub(super) image_view: vk::ImageView,
    pub(super) descriptor_set: vk::DescriptorSet,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl VulkanTexture {
    pub(super) fn destroy(
        self,
        device: &Device,
        allocator: &mut Allocator,
        pool: vk::DescriptorPool,
    ) {
        unsafe {
            let _ = device.free_descriptor_sets(pool, &[self.descriptor_set]);
            device.destroy_image_view(self.image_view, None);
        }
        let _ = allocator.destroy_image(device, self.image, self.image_mem);
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct ExternalTextureBinding {
    pub(super) descriptor_set: vk::DescriptorSet,
    pub(super) image_view: vk::ImageView,
    pub(super) image_layout: vk::ImageLayout,
}

impl ExternalTextureBinding {
    fn new(
        descriptor_set: vk::DescriptorSet,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Self {
        Self {
            descriptor_set,
            image_view,
            image_layout,
        }
    }
}

#[derive(Debug)]
pub(super) struct TextureManager {
    /// Legacy/default textures addressed directly by `TextureId`.
    pub(super) textures: HashMap<u64, VulkanTexture>,
    pub(super) managed_textures: HashMap<SnapshotTextureId, ManagedVulkanTexture>,
    pub(super) managed_ids: HashMap<u64, SnapshotTextureId>,
    /// Last Destroy epoch for identities that may still appear in older in-flight requests.
    pub(super) destroyed_managed_textures: HashMap<SnapshotTextureId, u64>,
    pub(super) retiring_textures:
        RetirementQueue<ManagedTextureRetirementKey, RetiredManagedVulkanTexture>,
    pub(super) external_textures: HashMap<u64, ExternalTextureBinding>,
    pub(super) next_id: u64,
}

#[derive(Debug)]
pub(super) struct ManagedVulkanTexture {
    pub(super) texture_id: TextureId,
    pub(super) texture: VulkanTexture,
    /// Complete pixels used to replace the image without reading GPU-owned state.
    rgba: Vec<u8>,
}

impl ManagedVulkanTexture {
    fn retire(self) -> RetiredManagedVulkanTexture {
        RetiredManagedVulkanTexture {
            texture_id: self.texture_id,
            texture: self.texture,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum ManagedTextureRetirementKey {
    Destroyed(SnapshotTextureId),
    Superseded(TextureRetirementBatch),
}

#[derive(Debug)]
pub(super) struct RetiredManagedVulkanTexture {
    pub(super) texture_id: TextureId,
    pub(super) texture: VulkanTexture,
}

impl TextureManager {
    pub(super) fn new() -> Self {
        Self {
            textures: HashMap::new(),
            managed_textures: HashMap::new(),
            managed_ids: HashMap::new(),
            destroyed_managed_textures: HashMap::new(),
            retiring_textures: RetirementQueue::new(),
            external_textures: HashMap::new(),
            next_id: 1,
        }
    }

    pub(super) fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        id
    }

    pub(super) fn get_descriptor_set(&self, texture_id: u64) -> Option<vk::DescriptorSet> {
        if let Some(tex) = self.textures.get(&texture_id) {
            Some(tex.descriptor_set)
        } else if let Some(snapshot_id) = self.managed_ids.get(&texture_id) {
            self.managed_textures
                .get(snapshot_id)
                .map(|texture| texture.texture.descriptor_set)
                .or_else(|| {
                    self.retiring_textures
                        .get(&ManagedTextureRetirementKey::Destroyed(*snapshot_id))
                        .map(|texture| texture.texture.descriptor_set)
                })
        } else {
            self.external_textures
                .get(&texture_id)
                .map(|b| b.descriptor_set)
        }
    }

    pub(super) fn register_external_texture(
        &mut self,
        set: vk::DescriptorSet,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> u64 {
        let id = self.allocate_id();
        self.external_textures.insert(
            id,
            ExternalTextureBinding::new(set, image_view, image_layout),
        );
        id
    }

    fn reserve_superseded_retirement(&mut self) -> Option<RetirementReservation> {
        self.retiring_textures.reserve()
    }

    fn install_managed_replacement(
        &mut self,
        texture: SnapshotTextureId,
        replacement: ManagedVulkanTexture,
        reservation: RetirementReservation,
    ) -> TextureId {
        let texture_id = replacement.texture_id;
        let key = ManagedTextureRetirementKey::Superseded(reservation.batch());
        let previous = self
            .managed_textures
            .insert(texture, replacement)
            .expect("managed replacement requires an active texture");
        debug_assert_eq!(previous.texture_id, texture_id);
        let _ = self
            .retiring_textures
            .commit(reservation, key, previous.retire());
        texture_id
    }

    fn request_managed_retirement(
        &mut self,
        texture: SnapshotTextureId,
    ) -> Result<RetirementRequest, ()> {
        let key = ManagedTextureRetirementKey::Destroyed(texture);
        if self.retiring_textures.contains_key(&key) {
            return Ok(RetirementRequest::Pending);
        }
        if !self.managed_textures.contains_key(&texture) {
            return Ok(RetirementRequest::Retired);
        }
        let Some(reservation) = self.retiring_textures.reserve() else {
            return Err(());
        };
        let managed = self
            .managed_textures
            .remove(&texture)
            .expect("managed texture existence was checked before retirement");
        let batch = self
            .retiring_textures
            .commit(reservation, key, managed.retire());
        Ok(RetirementRequest::Queued(batch))
    }

    fn seal_managed_texture_destroyed(&mut self, texture: SnapshotTextureId, destroy_epoch: u64) {
        self.destroyed_managed_textures
            .entry(texture)
            .and_modify(|epoch| *epoch = (*epoch).max(destroy_epoch))
            .or_insert(destroy_epoch);
    }

    fn managed_texture_is_destroyed(&self, texture: SnapshotTextureId) -> bool {
        self.destroyed_managed_textures.contains_key(&texture)
    }

    pub(super) fn clear_destroyed_managed_textures(&mut self) {
        self.destroyed_managed_textures.clear();
    }

    pub(super) fn prune_destroyed_managed_textures(&mut self, completion_watermark: u64) {
        self.destroyed_managed_textures
            .retain(|_, destroy_epoch| *destroy_epoch > completion_watermark);
    }

    fn complete_managed_retirements(
        &mut self,
        completed: TextureRetirementBatch,
    ) -> Option<Vec<RetiredManagedVulkanTexture>> {
        let retired = self.retiring_textures.complete_through(completed)?;
        let mut resources = Vec::with_capacity(retired.len());
        for (retirement, managed) in retired {
            if let ManagedTextureRetirementKey::Destroyed(snapshot_id) = retirement {
                let removed = self.managed_ids.remove(&managed.texture_id.id());
                debug_assert_eq!(removed, Some(snapshot_id));
            }
            resources.push(managed);
        }
        Some(resources)
    }
}

impl AshRenderer {
    /// Highest managed-texture resource retirement batch still waiting for GPU completion.
    ///
    /// Associate the returned token with GPU completion covering all Ash uploads, main-viewport
    /// draws, and secondary-viewport draws that can still reference a texture in the batch. If
    /// those operations span multiple queues, every relevant queue must complete before notifying
    /// this renderer.
    pub fn pending_texture_retirement(&self) -> RendererResult<Option<TextureRetirementBatch>> {
        self.ensure_operational()?;
        Ok(self.textures.retiring_textures.pending_batch())
    }

    /// Wait for the whole device and destroy managed texture resources through `completed`.
    ///
    /// Dear ImGui destroy requests are acknowledged on a later frame, after this method has
    /// actually released the corresponding Vulkan resources. Device loss is terminal: resources
    /// are reclaimed, then `ERROR_DEVICE_LOST` is returned. The returned count includes old Vulkan
    /// images superseded by managed updates as well as resources pending a Dear ImGui destroy.
    /// Any recorded but unsubmitted command buffer that references released resources becomes
    /// invalid and must never be submitted; this is part of [`Self::cmd_draw`]'s safety contract.
    pub fn wait_for_texture_retirements(
        &mut self,
        completed: TextureRetirementBatch,
    ) -> RendererResult<usize> {
        self.ensure_operational()?;
        let completion = match unsafe { self.device.device_wait_idle() } {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_DEVICE_LOST) => {
                Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
            }
            Err(error) => return Err(error.into()),
        };
        let count = self.complete_texture_retirements(completed)?;
        completion.map(|()| count)
    }

    /// Verify caller-provided fences and destroy managed texture resources through `completed`.
    ///
    /// Every fence is queried before any texture is destroyed. A pending or null fence leaves the
    /// retirement queue unchanged. The returned count includes old Vulkan images superseded by
    /// managed updates as well as resources pending a Dear ImGui destroy.
    ///
    /// # Safety
    ///
    /// Every fence must belong to this renderer's logical device and, together, cover every queue
    /// operation that can reference textures through `completed`, including uploads, main viewport
    /// draws, and secondary viewport draws. No recorded command buffer which references released
    /// resources may be submitted afterwards. Vulkan cannot validate foreign-device handles or
    /// future submissions.
    pub unsafe fn complete_texture_retirements_with_fences(
        &mut self,
        completed: TextureRetirementBatch,
        fences: &[vk::Fence],
    ) -> RendererResult<usize> {
        self.ensure_operational()?;
        if fences.is_empty() {
            return Err(RendererError::TextureRetirementFencesEmpty);
        }
        for (index, fence) in fences.iter().copied().enumerate() {
            if fence == vk::Fence::null() {
                return Err(RendererError::TextureRetirementFenceNull { index });
            }
            if !unsafe { self.device.get_fence_status(fence)? } {
                return Err(RendererError::TextureRetirementFencePending { index });
            }
        }
        self.complete_texture_retirements(completed)
    }

    fn complete_texture_retirements(
        &mut self,
        completed: TextureRetirementBatch,
    ) -> RendererResult<usize> {
        self.ensure_operational()?;
        let retired = self
            .textures
            .complete_managed_retirements(completed)
            .ok_or_else(|| {
                RendererError::InvalidRenderState(format!(
                    "texture retirement batch {} was not issued by this renderer",
                    completed.sequence()
                ))
            })?;
        let count = retired.len();
        for managed in retired {
            managed.texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            );
        }
        Ok(count)
    }

    /// Register an application-owned sampled image as a `TextureId`.
    ///
    /// Ash allocates and owns the set-0 sampled-image descriptor. Sampling is selected separately
    /// by Dear ImGui's standard linear/nearest callbacks and never mutates application sampler
    /// state.
    ///
    /// # Safety
    ///
    /// `image_view` must be a live sampled image view created from this renderer's logical device.
    /// It must remain alive, and its subresources must use `image_layout`, until the returned id is
    /// unregistered and all command buffers that referenced it have completed.
    pub unsafe fn register_external_texture(
        &mut self,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> RendererResult<TextureId> {
        self.ensure_operational()?;
        let set = create_vulkan_sampled_image_descriptor_set(
            &self.device,
            self.resources.sampled_image_set_layout,
            self.resources.descriptor_pool,
            image_view,
            image_layout,
        )?;
        Ok(TextureId::from(self.textures.register_external_texture(
            set,
            image_view,
            image_layout,
        )))
    }

    /// Update an external sampled image after waiting for the logical device to become idle.
    ///
    /// Returns `false` if the texture id is not an external texture registered by this renderer.
    ///
    /// # Safety
    ///
    /// The new view has the same device, lifetime, usage, and layout obligations as
    /// [`Self::register_external_texture`].
    pub unsafe fn update_external_texture(
        &mut self,
        texture_id: TextureId,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> RendererResult<bool> {
        self.ensure_operational()?;
        if !self
            .textures
            .external_textures
            .contains_key(&texture_id.id())
        {
            return Ok(false);
        }
        unsafe { self.device.device_wait_idle()? };
        unsafe { self.update_external_texture_unchecked(texture_id, image_view, image_layout) }
    }

    /// Update an external sampled image without waiting for earlier descriptor users.
    ///
    /// # Safety
    ///
    /// The caller must satisfy [`Self::update_external_texture`] and additionally prove that no
    /// submitted or recorded command can still access this texture's descriptor set. The renderer
    /// descriptor layout does not enable update-after-bind.
    pub unsafe fn update_external_texture_unchecked(
        &mut self,
        texture_id: TextureId,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> RendererResult<bool> {
        self.ensure_operational()?;
        let id = texture_id.id();
        let Some(binding) = self.textures.external_textures.get_mut(&id) else {
            return Ok(false);
        };

        binding.image_view = image_view;
        binding.image_layout = image_layout;
        unsafe {
            let image_info = [vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view,
                image_layout,
            }];
            let write_desc_sets = [vk::WriteDescriptorSet::default()
                .dst_set(binding.descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&image_info)];
            self.device.update_descriptor_sets(&write_desc_sets, &[]);
        }
        Ok(true)
    }

    /// Unregister a texture id after waiting for submitted device work.
    ///
    /// # Safety
    ///
    /// No recorded command buffer that references this texture may be submitted after this call.
    /// `device_wait_idle` covers submitted work only; it cannot discover application-owned command
    /// buffers that have been recorded but not submitted.
    pub unsafe fn unregister_texture(&mut self, texture_id: TextureId) -> RendererResult<()> {
        self.ensure_operational()?;
        if !self
            .textures
            .external_textures
            .contains_key(&texture_id.id())
        {
            return Ok(());
        }
        unsafe { self.device.device_wait_idle()? };
        unsafe { self.unregister_texture_unchecked(texture_id) }
    }

    /// Unregister a texture without waiting for submitted descriptor users.
    ///
    /// # Safety
    ///
    /// The caller must prove that no submitted or recorded command can still access this texture's
    /// descriptor set. Owned descriptor sets may be freed immediately.
    pub unsafe fn unregister_texture_unchecked(
        &mut self,
        texture_id: TextureId,
    ) -> RendererResult<()> {
        self.ensure_operational()?;
        let id = texture_id.id();
        let Some(binding) = self.textures.external_textures.get(&id).copied() else {
            return Ok(());
        };
        unsafe {
            self.device
                .free_descriptor_sets(self.resources.descriptor_pool, &[binding.descriptor_set])?;
        }
        self.textures.external_textures.remove(&id);
        Ok(())
    }

    /// Update a single texture manually.
    ///
    /// This mirrors the `dear-imgui-wgpu` API and is useful when the texture is not registered
    /// in ImGui's `PlatformIO.Textures[]` list (e.g. user-created `ImTextureData` that isn't
    /// registered via ImGui's experimental `RegisterUserTexture()` API).
    ///
    /// Call this before rendering if you pass `&mut TextureData` to widgets (e.g. `ui.image()`),
    /// otherwise `ImDrawCmd_GetTexID()` may assert if `TexID` is still invalid.
    ///
    /// This operation may replace or destroy Vulkan resources. Any recorded but unsubmitted
    /// command buffer that references those resources becomes invalid and must never be submitted;
    /// this is part of [`Self::cmd_draw`]'s safety contract.
    pub fn update_texture(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
        self.ensure_operational()?;
        unsafe { self.device.device_wait_idle()? };
        let result = unsafe { self.update_texture_unchecked(texture_data) }?;
        self.wait_for_pending_uploads()?;
        Ok(result)
    }

    /// Apply a legacy `TextureData` transition without synchronizing earlier or later GPU use.
    ///
    /// # Safety
    ///
    /// The caller must prove that earlier work no longer reads any texture that can be replaced,
    /// updated, or destroyed. Before using a created or updated texture, the caller must also prove
    /// completion or queue ordering for the upload submitted by this method.
    pub unsafe fn update_texture_unchecked(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
        self.ensure_operational()?;
        self.reap_completed_uploads()?;

        let status = texture_data.status();
        match status {
            TextureStatus::WantCreate => {
                let internal_id = texture_data.tex_id().id();
                let id = if internal_id != 0 && self.textures.textures.contains_key(&internal_id) {
                    internal_id
                } else {
                    self.textures.allocate_id()
                };
                let replacing_existing = self.textures.textures.contains_key(&id);
                if replacing_existing {
                    self.wait_for_pending_uploads()?;
                }

                let (w, h) = (texture_data.width(), texture_data.height());
                if w == 0 || h == 0 {
                    return Ok(TextureUpdateResult::Failed);
                }
                let Some(pixels) = texture_data_to_rgba_full(texture_data) else {
                    return Ok(TextureUpdateResult::Failed);
                };

                let (texture, staging_buffer, staging_mem) =
                    Texture::create(&self.device, &mut self.allocator, w, h, &pixels)?;

                let descriptor_set = match create_vulkan_sampled_image_descriptor_set(
                    &self.device,
                    self.resources.sampled_image_set_layout,
                    self.resources.descriptor_pool,
                    texture.image_view,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ) {
                    Ok(descriptor_set) => descriptor_set,
                    Err(err) => {
                        let _ = self.allocator.destroy_buffer(
                            &self.device,
                            staging_buffer,
                            staging_mem,
                        );
                        let _ = texture.destroy(&self.device, &mut self.allocator);
                        return Err(err);
                    }
                };

                let (command_buffer, fence) = match self.submit_upload_commands(|cmd| {
                    texture.upload(&self.device, cmd, staging_buffer, w, h);
                }) {
                    Ok(upload) => upload,
                    Err(err) => {
                        unsafe {
                            let _ = self.device.free_descriptor_sets(
                                self.resources.descriptor_pool,
                                &[descriptor_set],
                            );
                        }
                        let _ = self.allocator.destroy_buffer(
                            &self.device,
                            staging_buffer,
                            staging_mem,
                        );
                        let _ = texture.destroy(&self.device, &mut self.allocator);
                        return Err(err);
                    }
                };

                self.in_flight_uploads.push_back(InFlightUpload {
                    fence,
                    command_buffer,
                    staging: vec![(staging_buffer, staging_mem)],
                    managed_texture: None,
                });

                if let Some(old) = self.textures.textures.remove(&id) {
                    old.destroy(
                        &self.device,
                        &mut self.allocator,
                        self.resources.descriptor_pool,
                    );
                }
                self.textures.textures.insert(
                    id,
                    VulkanTexture {
                        image: texture.image,
                        image_mem: texture.image_mem,
                        image_view: texture.image_view,
                        descriptor_set,
                        width: w,
                        height: h,
                    },
                );

                Ok(TextureUpdateResult::Created {
                    texture_id: TextureId::from(id),
                })
            }
            TextureStatus::WantUpdates => {
                let internal_id = texture_data.tex_id().id();
                if internal_id == 0 || !self.textures.textures.contains_key(&internal_id) {
                    // Not created yet: treat updates as a full create.
                    return self.update_texture_with_forced_create(texture_data);
                }

                let Some(existing) = self.textures.textures.get(&internal_id) else {
                    return Ok(TextureUpdateResult::Failed);
                };

                let (tw, th) = (existing.width, existing.height);
                let rect = texture_data.update_rect();
                let (x, y, w, h) = clamp_rect(rect, tw, th);
                if w == 0 || h == 0 {
                    return Ok(TextureUpdateResult::Updated);
                }

                let Some(pixels) = texture_data_to_rgba_subrect(texture_data, x, y, w, h) else {
                    return Ok(TextureUpdateResult::Failed);
                };
                let (staging_buffer, staging_mem) = create_and_fill_buffer(
                    &self.device,
                    &mut self.allocator,
                    &pixels,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                )?;

                let (command_buffer, fence) = match self.submit_upload_commands(|cmd| {
                    upload_rgba_subrect_to_image(
                        &self.device,
                        cmd,
                        staging_buffer,
                        existing.image,
                        ImageUploadRegion {
                            x,
                            y,
                            width: w,
                            height: h,
                        },
                    );
                }) {
                    Ok(upload) => upload,
                    Err(err) => {
                        let _ = self.allocator.destroy_buffer(
                            &self.device,
                            staging_buffer,
                            staging_mem,
                        );
                        return Err(err);
                    }
                };

                self.in_flight_uploads.push_back(InFlightUpload {
                    fence,
                    command_buffer,
                    staging: vec![(staging_buffer, staging_mem)],
                    managed_texture: None,
                });

                Ok(TextureUpdateResult::Updated)
            }
            TextureStatus::WantDestroy => {
                let id = texture_data.tex_id().id();
                if self.textures.textures.contains_key(&id) {
                    self.wait_for_pending_uploads()?;
                }
                if let Some(tex) = self.textures.textures.remove(&id) {
                    tex.destroy(
                        &self.device,
                        &mut self.allocator,
                        self.resources.descriptor_pool,
                    );
                }
                Ok(TextureUpdateResult::Destroyed)
            }
            TextureStatus::OK | TextureStatus::Destroyed => Ok(TextureUpdateResult::NoAction),
        }
    }

    fn update_texture_with_forced_create(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
        // Force-create by temporarily treating it as WantCreate.
        // We don't mutate the passed-in TextureData here; the returned result will set TexID/Status.
        let internal_id = texture_data.tex_id().id();
        let id = if internal_id != 0 && self.textures.textures.contains_key(&internal_id) {
            internal_id
        } else {
            self.textures.allocate_id()
        };
        let replacing_existing = self.textures.textures.contains_key(&id);
        if replacing_existing {
            self.wait_for_pending_uploads()?;
        }

        let (w, h) = (texture_data.width(), texture_data.height());
        if w == 0 || h == 0 {
            return Ok(TextureUpdateResult::Failed);
        }
        let Some(pixels) = texture_data_to_rgba_full(texture_data) else {
            return Ok(TextureUpdateResult::Failed);
        };

        let (texture, staging_buffer, staging_mem) =
            Texture::create(&self.device, &mut self.allocator, w, h, &pixels)?;

        let descriptor_set = match create_vulkan_sampled_image_descriptor_set(
            &self.device,
            self.resources.sampled_image_set_layout,
            self.resources.descriptor_pool,
            texture.image_view,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ) {
            Ok(descriptor_set) => descriptor_set,
            Err(err) => {
                let _ = self
                    .allocator
                    .destroy_buffer(&self.device, staging_buffer, staging_mem);
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(err);
            }
        };

        let (command_buffer, fence) = match self.submit_upload_commands(|cmd| {
            texture.upload(&self.device, cmd, staging_buffer, w, h);
        }) {
            Ok(upload) => upload,
            Err(err) => {
                unsafe {
                    let _ = self
                        .device
                        .free_descriptor_sets(self.resources.descriptor_pool, &[descriptor_set]);
                }
                let _ = self
                    .allocator
                    .destroy_buffer(&self.device, staging_buffer, staging_mem);
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(err);
            }
        };

        self.in_flight_uploads.push_back(InFlightUpload {
            fence,
            command_buffer,
            staging: vec![(staging_buffer, staging_mem)],
            managed_texture: None,
        });

        if let Some(old) = self.textures.textures.remove(&id) {
            old.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            );
        }
        self.textures.textures.insert(
            id,
            VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                descriptor_set,
                width: w,
                height: h,
            },
        );

        Ok(TextureUpdateResult::Created {
            texture_id: TextureId::from(id),
        })
    }
}

impl AshRenderer {
    pub(super) fn create_default_texture(&mut self) -> RendererResult<u64> {
        // 1x1 white RGBA.
        let pixels = [255u8, 255u8, 255u8, 255u8];
        let texture_id = self.textures.allocate_id();

        let (texture, staging_buffer, staging_mem) =
            Texture::create(&self.device, &mut self.allocator, 1, 1, &pixels)?;

        let descriptor_set = match create_vulkan_sampled_image_descriptor_set(
            &self.device,
            self.resources.sampled_image_set_layout,
            self.resources.descriptor_pool,
            texture.image_view,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ) {
            Ok(descriptor_set) => descriptor_set,
            Err(err) => {
                let _ = self
                    .allocator
                    .destroy_buffer(&self.device, staging_buffer, staging_mem);
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(err);
            }
        };

        let (command_buffer, fence) = match self.submit_upload_commands(|command_buffer| {
            texture.upload(&self.device, command_buffer, staging_buffer, 1, 1);
        }) {
            Ok(upload) => upload,
            Err(error) => {
                unsafe {
                    let _ = self
                        .device
                        .free_descriptor_sets(self.resources.descriptor_pool, &[descriptor_set]);
                }
                let _ = self
                    .allocator
                    .destroy_buffer(&self.device, staging_buffer, staging_mem);
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(error);
            }
        };
        self.in_flight_uploads.push_back(InFlightUpload {
            fence,
            command_buffer,
            staging: vec![(staging_buffer, staging_mem)],
            managed_texture: None,
        });

        self.textures.textures.insert(
            texture_id,
            VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                descriptor_set,
                width: 1,
                height: 1,
            },
        );

        Ok(texture_id)
    }

    pub(super) fn process_texture_requests(
        &mut self,
        requests: &[TextureRequest],
        request_epoch: u64,
    ) -> RendererResult<Vec<TextureFeedback>> {
        let mut feedback = Vec::with_capacity(requests.len());

        for request in requests {
            let snapshot_id = request.texture();
            match request.operation() {
                TextureOp::Create { .. } | TextureOp::Update { .. } => {
                    if !self.textures.managed_texture_is_destroyed(snapshot_id) {
                        feedback.push(self.complete_managed_upload_request(request)?)
                    }
                }
                TextureOp::Destroy => {
                    self.textures
                        .seal_managed_texture_destroyed(snapshot_id, request_epoch);
                    let upload_wait = if self.managed_uploads.is_pending(snapshot_id) {
                        self.wait_for_managed_upload(snapshot_id)
                    } else {
                        Ok(())
                    };
                    finish_destroy_upload_gate(
                        &mut self.managed_uploads,
                        snapshot_id,
                        upload_wait,
                    )?;
                    match self.textures.request_managed_retirement(snapshot_id) {
                        Ok(RetirementRequest::Queued(_)) | Ok(RetirementRequest::Pending) => {}
                        Ok(RetirementRequest::Retired) => feedback.push(request.destroyed()?),
                        Err(()) => {
                            return Err(RendererError::InvalidRenderState(
                                "managed texture retirement batch space is exhausted".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(feedback)
    }

    fn complete_managed_upload_request(
        &mut self,
        request: &TextureRequest,
    ) -> RendererResult<TextureFeedback> {
        let snapshot_id = request.texture();
        let identity = request.upload_identity().ok_or_else(|| {
            RendererError::InvalidRenderState(
                "destroy request entered the managed upload path".to_string(),
            )
        })?;

        loop {
            match self.managed_uploads.decide(snapshot_id, identity) {
                ManagedUploadDecision::Ready(texture_id) => {
                    return Ok(request.uploaded(texture_id)?);
                }
                ManagedUploadDecision::Wait => {
                    self.wait_for_managed_upload(snapshot_id)?;
                }
                ManagedUploadDecision::Submit => {
                    let uploads_before = self.in_flight_uploads.len();
                    let texture_id = match request.operation() {
                        TextureOp::Create {
                            format,
                            width,
                            height,
                            row_pitch,
                            pixels,
                        } => self.create_managed_texture(
                            snapshot_id,
                            *format,
                            *width,
                            *height,
                            *row_pitch,
                            pixels,
                        )?,
                        TextureOp::Update {
                            format,
                            width,
                            height,
                            rects,
                        } => self.update_managed_texture(
                            snapshot_id,
                            *format,
                            *width,
                            *height,
                            rects,
                        )?,
                        TextureOp::Destroy => unreachable!("validated upload operation"),
                    };

                    if self.in_flight_uploads.len() == uploads_before {
                        return Ok(request.uploaded(texture_id)?);
                    }
                    let upload = self.in_flight_uploads.back_mut().ok_or_else(|| {
                        RendererError::InvalidRenderState(format!(
                            "managed texture {snapshot_id:?} submitted no trackable upload"
                        ))
                    })?;
                    if upload.managed_texture.is_some() {
                        return Err(RendererError::InvalidRenderState(format!(
                            "managed texture {snapshot_id:?} collided with another upload"
                        )));
                    }
                    upload.managed_texture = Some(snapshot_id);
                    self.managed_uploads
                        .submitted(snapshot_id, identity, texture_id);
                    let upload_wait = self.wait_for_managed_upload(snapshot_id);
                    let texture_id = finish_managed_upload_gate(
                        &mut self.managed_uploads,
                        snapshot_id,
                        upload_wait,
                    )?
                    .ok_or_else(|| {
                        RendererError::InvalidRenderState(format!(
                            "managed texture {snapshot_id:?} completed without tracked feedback"
                        ))
                    })?;
                    return Ok(request.uploaded(texture_id)?);
                }
            }
        }
    }

    fn create_managed_texture(
        &mut self,
        snapshot_id: SnapshotTextureId,
        format: ImGuiTextureFormat,
        width: u32,
        height: u32,
        row_pitch: usize,
        pixels: &[u8],
    ) -> RendererResult<TextureId> {
        if self
            .textures
            .retiring_textures
            .contains_key(&ManagedTextureRetirementKey::Destroyed(snapshot_id))
        {
            return Err(RendererError::InvalidRenderState(format!(
                "managed texture {snapshot_id:?} was recreated while retirement is pending"
            )));
        }
        if width == 0 || height == 0 {
            return Err(RendererError::InvalidRenderState(format!(
                "managed texture {snapshot_id:?} has zero dimensions"
            )));
        }
        let rgba =
            texture_upload_to_rgba(format, width, height, row_pitch, pixels).ok_or_else(|| {
                RendererError::InvalidRenderState(format!(
                    "managed texture {snapshot_id:?} has an invalid create upload layout"
                ))
            })?;
        if let Some(existing) = self.textures.managed_textures.get(&snapshot_id) {
            if existing.texture.width != width || existing.texture.height != height {
                return Err(RendererError::InvalidRenderState(format!(
                    "managed texture {snapshot_id:?} create dimensions changed during retry"
                )));
            }
            if existing.rgba == rgba {
                return Ok(existing.texture_id);
            }
            return self.replace_managed_texture_image(snapshot_id, width, height, rgba);
        }
        let raw_id = self.textures.allocate_id();
        let texture_id = TextureId::from(raw_id);
        let managed = self.create_managed_texture_image(texture_id, width, height, rgba)?;
        self.textures.managed_ids.insert(raw_id, snapshot_id);
        self.textures.managed_textures.insert(snapshot_id, managed);
        Ok(texture_id)
    }

    fn create_managed_texture_image(
        &mut self,
        texture_id: TextureId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> RendererResult<ManagedVulkanTexture> {
        let (texture, staging_buffer, staging_mem) =
            Texture::create(&self.device, &mut self.allocator, width, height, &rgba)?;
        let descriptor_set = match create_vulkan_sampled_image_descriptor_set(
            &self.device,
            self.resources.sampled_image_set_layout,
            self.resources.descriptor_pool,
            texture.image_view,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ) {
            Ok(descriptor_set) => descriptor_set,
            Err(error) => {
                let _ = self
                    .allocator
                    .destroy_buffer(&self.device, staging_buffer, staging_mem);
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(error);
            }
        };
        let (command_buffer, fence) = match self.submit_upload_commands(|command_buffer| {
            texture.upload(&self.device, command_buffer, staging_buffer, width, height);
        }) {
            Ok(upload) => upload,
            Err(error) => {
                unsafe {
                    let _ = self
                        .device
                        .free_descriptor_sets(self.resources.descriptor_pool, &[descriptor_set]);
                }
                let _ = self
                    .allocator
                    .destroy_buffer(&self.device, staging_buffer, staging_mem);
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(error);
            }
        };
        self.in_flight_uploads.push_back(InFlightUpload {
            fence,
            command_buffer,
            staging: vec![(staging_buffer, staging_mem)],
            managed_texture: None,
        });
        let managed = ManagedVulkanTexture {
            texture_id,
            texture: VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                descriptor_set,
                width,
                height,
            },
            rgba,
        };
        Ok(managed)
    }

    fn replace_managed_texture_image(
        &mut self,
        snapshot_id: SnapshotTextureId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> RendererResult<TextureId> {
        let texture_id = self
            .textures
            .managed_textures
            .get(&snapshot_id)
            .map(|managed| managed.texture_id)
            .ok_or_else(|| {
                RendererError::InvalidRenderState(format!(
                    "managed texture {snapshot_id:?} has no active image to replace"
                ))
            })?;
        // Reserve before Vulkan allocation so exhaustion cannot strand a submitted replacement.
        let reservation = self
            .textures
            .reserve_superseded_retirement()
            .ok_or_else(|| {
                RendererError::InvalidRenderState(
                    "managed texture retirement batch space is exhausted".to_string(),
                )
            })?;
        let replacement = self.create_managed_texture_image(texture_id, width, height, rgba)?;
        Ok(self
            .textures
            .install_managed_replacement(snapshot_id, replacement, reservation))
    }

    fn update_managed_texture(
        &mut self,
        snapshot_id: SnapshotTextureId,
        format: ImGuiTextureFormat,
        width: u32,
        height: u32,
        rects: &[TextureUploadRect],
    ) -> RendererResult<TextureId> {
        let Some(existing) = self.textures.managed_textures.get(&snapshot_id) else {
            return Err(RendererError::InvalidRenderState(format!(
                "managed texture {snapshot_id:?} received an update before create"
            )));
        };
        if existing.texture.width != width || existing.texture.height != height {
            return Err(RendererError::InvalidRenderState(format!(
                "managed texture {snapshot_id:?} update dimensions changed without create"
            )));
        }
        let texture_id = existing.texture_id;
        let mut replacement_rgba = existing.rgba.clone();
        for upload in rects {
            let rect = upload.rect;
            let (x, y, w, h) = (
                u32::from(rect.x),
                u32::from(rect.y),
                u32::from(rect.w),
                u32::from(rect.h),
            );
            if w == 0 || h == 0 {
                continue;
            }
            let valid_bounds = x.checked_add(w).is_some_and(|right| right <= width)
                && y.checked_add(h).is_some_and(|bottom| bottom <= height);
            if !valid_bounds {
                return Err(RendererError::InvalidRenderState(format!(
                    "managed texture {snapshot_id:?} update rectangle is out of bounds"
                )));
            }
            let Some(rgba) = texture_upload_to_rgba(format, w, h, upload.row_pitch, &upload.data)
            else {
                return Err(RendererError::InvalidRenderState(format!(
                    "managed texture {snapshot_id:?} has an invalid update upload layout"
                )));
            };
            if !apply_rgba_rect(
                &mut replacement_rgba,
                RgbaExtent { width, height },
                RgbaRect {
                    x,
                    y,
                    width: w,
                    height: h,
                },
                &rgba,
            ) {
                return Err(RendererError::InvalidRenderState(format!(
                    "managed texture {snapshot_id:?} has an invalid CPU shadow layout"
                )));
            }
        }
        if replacement_rgba == existing.rgba {
            return Ok(texture_id);
        }
        self.replace_managed_texture_image(snapshot_id, width, height, replacement_rgba)
    }
}

#[derive(Clone, Copy)]
pub(super) struct RgbaExtent {
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Clone, Copy)]
pub(super) struct RgbaRect {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn apply_rgba_rect(
    destination: &mut [u8],
    destination_extent: RgbaExtent,
    source_rect: RgbaRect,
    source: &[u8],
) -> bool {
    let Ok(destination_width) = usize::try_from(destination_extent.width) else {
        return false;
    };
    let Ok(destination_height) = usize::try_from(destination_extent.height) else {
        return false;
    };
    let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
        usize::try_from(source_rect.x),
        usize::try_from(source_rect.y),
        usize::try_from(source_rect.width),
        usize::try_from(source_rect.height),
    ) else {
        return false;
    };
    let Some(destination_len) = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    let Some(source_row_len) = width.checked_mul(4) else {
        return false;
    };
    let Some(source_len) = source_row_len.checked_mul(height) else {
        return false;
    };
    if destination.len() != destination_len
        || source.len() != source_len
        || x.checked_add(width)
            .is_none_or(|right| right > destination_width)
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > destination_height)
    {
        return false;
    }

    for row in 0..height {
        let destination_start = (y + row) * destination_width * 4 + x * 4;
        let source_start = row * source_row_len;
        destination[destination_start..destination_start + source_row_len]
            .copy_from_slice(&source[source_start..source_start + source_row_len]);
    }
    true
}

pub(super) fn texture_upload_to_rgba(
    format: ImGuiTextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &[u8],
) -> Option<Vec<u8>> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let source_bpp = match format {
        ImGuiTextureFormat::RGBA32 => 4,
        ImGuiTextureFormat::Alpha8 => 1,
    };
    let packed_row = width.checked_mul(source_bpp)?;
    if row_pitch < packed_row {
        return None;
    }
    let required = (height - 1)
        .checked_mul(row_pitch)?
        .checked_add(packed_row)?;
    if pixels.len() < required {
        return None;
    }

    let mut rgba = vec![0_u8; width.checked_mul(height)?.checked_mul(4)?];
    for row in 0..height {
        let source = &pixels[row * row_pitch..row * row_pitch + packed_row];
        let destination = &mut rgba[row * width * 4..(row + 1) * width * 4];
        match format {
            ImGuiTextureFormat::RGBA32 => destination.copy_from_slice(source),
            ImGuiTextureFormat::Alpha8 => {
                for (pixel, alpha) in destination.chunks_exact_mut(4).zip(source) {
                    pixel.copy_from_slice(&[255, 255, 255, *alpha]);
                }
            }
        }
    }
    Some(rgba)
}

pub(super) fn texture_data_to_rgba_full(td: &TextureData) -> Option<Vec<u8>> {
    let w = td.width();
    let h = td.height();
    if w == 0 || h == 0 {
        return None;
    }
    texture_data_to_rgba_subrect(td, 0, 0, w, h)
}

pub(super) fn texture_data_to_rgba_subrect(
    td: &TextureData,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Option<Vec<u8>> {
    let pixels = td.pixels()?;
    let tex_w = usize::try_from(td.width()).ok()?;
    let tex_h = usize::try_from(td.height()).ok()?;
    if tex_w == 0 || tex_h == 0 {
        return None;
    }

    let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
    if w == 0 || h == 0 || x >= tex_w || y >= tex_h {
        return None;
    }
    let w = w.min(tex_w.saturating_sub(x));
    let h = h.min(tex_h.saturating_sub(y));
    let bpp = td.bytes_per_pixel();

    let mut out = vec![0u8; w.checked_mul(h)?.checked_mul(4)?];
    match td.format() {
        ImGuiTextureFormat::RGBA32 => {
            for row in 0..h {
                let src_off = ((y + row) * tex_w + x) * bpp;
                let dst_off = row * w * 4;
                out[dst_off..dst_off + w * 4].copy_from_slice(&pixels[src_off..src_off + w * 4]);
            }
        }
        ImGuiTextureFormat::Alpha8 => {
            for row in 0..h {
                let src_off = ((y + row) * tex_w + x) * bpp;
                let dst_off = row * w * 4;
                for col in 0..w {
                    let a = pixels[src_off + col];
                    let o = dst_off + col * 4;
                    out[o..o + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
        }
    }

    Some(out)
}

pub(super) fn clamp_rect(
    rect: dear_imgui_rs::texture::TextureRect,
    tw: u32,
    th: u32,
) -> (u32, u32, u32, u32) {
    let x = u32::from(rect.x).min(tw);
    let y = u32::from(rect.y).min(th);
    let w = u32::from(rect.w);
    let h = u32::from(rect.h);
    if w == 0 || h == 0 || x >= tw || y >= th {
        return (x, y, 0, 0);
    }
    (x, y, w.min(tw - x), h.min(th - y))
}

#[cfg(all(test, not(any(feature = "gpu-allocator", feature = "vk-mem"))))]
mod operational_gate_tests {
    use super::*;
    use dear_imgui_rs::texture::OwnedTextureData;

    fn destroyed_renderer(context: &Context) -> AshRenderer {
        let device = unsafe { Device::load_with(|_| std::ptr::null(), vk::Device::null()) };
        AshRenderer {
            device,
            allocator: Allocator::new(vk::PhysicalDeviceMemoryProperties::default()),
            queue: vk::Queue::null(),
            command_pool: vk::CommandPool::null(),
            resources: VulkanRendererResources::empty(),
            textures: TextureManager::new(),
            consumer: None,
            context_state: RendererContextState::prepare(context).unwrap(),
            default_texture_id: 0,
            options: Options::default(),
            frames: Frames::new(0),
            destroyed: true,
            in_flight_uploads: VecDeque::new(),
            managed_uploads: ManagedUploadTracker::default(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_pipelines: HashMap::new(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn assert_destroyed<T>(result: RendererResult<T>) {
        assert!(matches!(result, Err(RendererError::RendererDestroyed)));
    }

    #[test]
    fn every_resource_entry_rejects_destroyed_renderer_before_vulkan() {
        let mut context = Context::create();
        let mut renderer = destroyed_renderer(&context);
        let batch = renderer
            .textures
            .retiring_textures
            .reserve()
            .unwrap()
            .batch();
        let texture = TextureId::from(1_u64);
        let texture_data = OwnedTextureData::new();

        assert_destroyed(renderer.pending_texture_retirement());
        assert_destroyed(renderer.wait_for_texture_retirements(batch));
        assert_destroyed(unsafe {
            renderer.complete_texture_retirements_with_fences(batch, &[vk::Fence::null()])
        });
        assert_destroyed(unsafe {
            renderer.register_external_texture(
                vk::ImageView::null(),
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
        });
        assert_destroyed(unsafe {
            renderer.update_external_texture(
                texture,
                vk::ImageView::null(),
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
        });
        assert_destroyed(unsafe {
            renderer.update_external_texture_unchecked(
                texture,
                vk::ImageView::null(),
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
        });
        assert_destroyed(unsafe { renderer.unregister_texture(texture) });
        assert_destroyed(unsafe { renderer.unregister_texture_unchecked(texture) });
        assert_destroyed(renderer.update_texture(&texture_data));
        assert_destroyed(unsafe { renderer.update_texture_unchecked(&texture_data) });

        context.io_mut().set_display_size([128.0, 128.0]);
        assert!(context.font_atlas().build());
        context.frame();
        let frame = context.render();
        assert_destroyed(unsafe { renderer.cmd_draw(vk::CommandBuffer::null(), frame) });
        assert_destroyed(renderer.shutdown(&mut context));

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        assert_destroyed(renderer.viewport_pipeline(vk::Format::R8G8B8A8_UNORM));
    }
}

#[cfg(all(test, not(any(feature = "gpu-allocator", feature = "vk-mem"))))]
mod managed_lifecycle_tests {
    use super::*;
    use ash::vk::Handle;

    fn managed_texture(
        texture_id: TextureId,
        image: u64,
        descriptor_set: u64,
        pixel: u8,
    ) -> ManagedVulkanTexture {
        ManagedVulkanTexture {
            texture_id,
            texture: VulkanTexture {
                image: vk::Image::from_raw(image),
                image_mem: vk::DeviceMemory::from_raw(image + 100),
                image_view: vk::ImageView::from_raw(image + 200),
                descriptor_set: vk::DescriptorSet::from_raw(descriptor_set),
                width: 1,
                height: 1,
            },
            rgba: vec![pixel; 4],
        }
    }

    #[test]
    fn superseded_then_destroyed_texture_preserves_mapping_until_final_completion() {
        let context = Context::create();
        let snapshot_id = SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 7,
            generation: 3,
        };
        let texture_id = TextureId::from(41_u64);
        let mut manager = TextureManager::new();
        manager.managed_ids.insert(texture_id.id(), snapshot_id);
        manager
            .managed_textures
            .insert(snapshot_id, managed_texture(texture_id, 1, 11, 1));

        let reservation = manager.reserve_superseded_retirement().unwrap();
        let superseded_batch = reservation.batch();
        assert_eq!(
            manager.install_managed_replacement(
                snapshot_id,
                managed_texture(texture_id, 2, 22, 2),
                reservation,
            ),
            texture_id
        );
        assert_eq!(
            manager.get_descriptor_set(texture_id.id()),
            Some(vk::DescriptorSet::from_raw(22))
        );

        let superseded = manager
            .complete_managed_retirements(superseded_batch)
            .unwrap();
        assert_eq!(superseded.len(), 1);
        assert_eq!(superseded[0].texture.image.as_raw(), 1);
        assert_eq!(
            manager.managed_ids.get(&texture_id.id()),
            Some(&snapshot_id)
        );
        assert_eq!(
            manager.managed_textures[&snapshot_id]
                .texture
                .image
                .as_raw(),
            2
        );

        let destroy_epoch = 9;
        manager.seal_managed_texture_destroyed(snapshot_id, destroy_epoch);
        let RetirementRequest::Queued(destroy_batch) = manager
            .request_managed_retirement(snapshot_id)
            .expect("active replacement should enter destroy retirement")
        else {
            panic!("active replacement was acknowledged before retirement");
        };
        assert!(!manager.managed_textures.contains_key(&snapshot_id));
        assert_eq!(
            manager.get_descriptor_set(texture_id.id()),
            Some(vk::DescriptorSet::from_raw(22))
        );

        let destroyed = manager.complete_managed_retirements(destroy_batch).unwrap();
        assert_eq!(destroyed.len(), 1);
        assert_eq!(destroyed[0].texture.image.as_raw(), 2);
        assert!(!manager.managed_ids.contains_key(&texture_id.id()));
        assert_eq!(manager.get_descriptor_set(texture_id.id()), None);
        assert_eq!(manager.retiring_textures.pending_batch(), None);
        assert!(manager.managed_texture_is_destroyed(snapshot_id));

        manager.prune_destroyed_managed_textures(destroy_epoch - 1);
        assert!(manager.managed_texture_is_destroyed(snapshot_id));
        manager.prune_destroyed_managed_textures(destroy_epoch);
        assert!(!manager.managed_texture_is_destroyed(snapshot_id));
    }

    #[test]
    fn destroy_tombstones_use_max_epoch_and_contiguous_watermark() {
        let context = Context::create();
        let snapshot_id = SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 11,
            generation: 4,
        };
        let mut manager = TextureManager::new();

        manager.seal_managed_texture_destroyed(snapshot_id, 8);
        manager.seal_managed_texture_destroyed(snapshot_id, 5);
        manager.prune_destroyed_managed_textures(7);
        assert!(manager.managed_texture_is_destroyed(snapshot_id));

        manager.seal_managed_texture_destroyed(snapshot_id, 10);
        manager.prune_destroyed_managed_textures(8);
        assert!(manager.managed_texture_is_destroyed(snapshot_id));
        manager.prune_destroyed_managed_textures(10);
        assert!(!manager.managed_texture_is_destroyed(snapshot_id));
    }

    #[test]
    fn advancing_watermark_keeps_destroy_tombstones_bounded() {
        let context = Context::create();
        let mut manager = TextureManager::new();

        for epoch in 1..=64 {
            let snapshot_id = SnapshotTextureId::FontAtlas {
                context: context.id(),
                stamp: 13,
                generation: epoch,
            };
            manager.seal_managed_texture_destroyed(snapshot_id, epoch);
            manager.prune_destroyed_managed_textures(epoch - 1);
            assert_eq!(manager.destroyed_managed_textures.len(), 1);
        }
        manager.prune_destroyed_managed_textures(64);
        assert!(manager.destroyed_managed_textures.is_empty());
    }
}
