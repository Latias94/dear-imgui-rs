use super::*;
use dear_imgui_backend_support::{DirtyRectNormalizer, ManagedTextureTable};
use dear_imgui_rs::render::TextureUploadIdentity;
use std::borrow::Cow;

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
    fn create(
        device: &Device,
        allocator: &mut RendererAllocator,
        resources: &VulkanRendererResources,
        width: u32,
        height: u32,
    ) -> RendererResult<Self> {
        let (image, image_mem) =
            allocator.create_image(device, width, height, MANAGED_TEXTURE_FORMAT)?;
        let image_view = match create_texture_image_view(device, image) {
            Ok(view) => view,
            Err(error) => {
                let _ = allocator.destroy_image(device, image, image_mem);
                return Err(error);
            }
        };
        let descriptor_set = match create_vulkan_sampled_image_descriptor_set(
            device,
            resources.sampled_image_set_layout,
            resources.descriptor_pool,
            image_view,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ) {
            Ok(set) => set,
            Err(error) => {
                unsafe { device.destroy_image_view(image_view, None) };
                let _ = allocator.destroy_image(device, image, image_mem);
                return Err(error);
            }
        };
        Ok(Self {
            image,
            image_mem,
            image_view,
            descriptor_set,
            width,
            height,
        })
    }

    pub(super) fn destroy(
        self,
        device: &Device,
        allocator: &mut RendererAllocator,
        pool: vk::DescriptorPool,
    ) -> RendererResult<()> {
        let descriptor_result = unsafe {
            let result = device.free_descriptor_sets(pool, &[self.descriptor_set]);
            device.destroy_image_view(self.image_view, None);
            result.map_err(RendererError::from)
        };
        descriptor_result.and(allocator.destroy_image(device, self.image, self.image_mem))
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
    pub(super) managed_textures: ManagedTextureTable<ManagedVulkanTexture>,
    pub(super) managed_ids: HashMap<u64, SnapshotTextureId>,
    pub(super) retiring_textures:
        RetirementQueue<ManagedTextureRetirementKey, RetiredManagedVulkanTexture>,
    /// Destination images of a submission that reported terminal device loss.
    pub(super) unconfirmed_uploads: Vec<VulkanTexture>,
    pub(super) external_textures: HashMap<u64, ExternalTextureBinding>,
    pub(super) next_id: u64,
}

#[derive(Debug)]
pub(super) struct ManagedVulkanTexture {
    pub(super) texture_id: TextureId,
    pub(super) texture: VulkanTexture,
    upload_identity: TextureUploadIdentity,
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

struct StagedManagedUpload<'pixels> {
    snapshot_id: SnapshotTextureId,
    managed: ManagedVulkanTexture,
    replacement: Option<RetirementReservation>,
    previous_image: Option<vk::Image>,
    pixels: Vec<ImageUploadPixels<'pixels>>,
}

impl TextureManager {
    pub(super) fn new() -> Self {
        Self {
            textures: HashMap::new(),
            managed_textures: ManagedTextureTable::default(),
            managed_ids: HashMap::new(),
            retiring_textures: RetirementQueue::new(),
            unconfirmed_uploads: Vec::new(),
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
                .get(*snapshot_id)
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
    ) {
        let texture_id = replacement.texture_id;
        let key = ManagedTextureRetirementKey::Superseded(reservation.batch());
        let previous = self
            .managed_textures
            .insert(texture, replacement)
            .expect("a staged replacement is not sealed")
            .expect("managed replacement requires an active texture");
        debug_assert_eq!(previous.texture_id, texture_id);
        let _ = self
            .retiring_textures
            .commit(reservation, key, previous.retire());
    }

    fn request_managed_retirement(
        &mut self,
        texture: SnapshotTextureId,
    ) -> Result<RetirementRequest, ()> {
        let key = ManagedTextureRetirementKey::Destroyed(texture);
        if self.retiring_textures.contains_key(&key) {
            return Ok(RetirementRequest::Pending);
        }
        if self.managed_textures.get(texture).is_none() {
            return Ok(RetirementRequest::Retired);
        }
        let Some(reservation) = self.retiring_textures.reserve() else {
            return Err(());
        };
        let managed = self
            .managed_textures
            .remove(texture)
            .expect("managed texture existence was checked before retirement");
        let batch = self
            .retiring_textures
            .commit(reservation, key, managed.retire());
        Ok(RetirementRequest::Queued(batch))
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
    /// Associate the returned token with GPU completion covering all Ash uploads (including
    /// copies from superseded images), main-viewport draws, and secondary-viewport draws that can
    /// still reference a texture in the batch. If those operations span multiple queues, every
    /// relevant queue must complete before notifying this renderer.
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
        let mut result = Ok(());
        for managed in retired {
            result = result.and(managed.texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            ));
        }
        result.map(|()| count)
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
    /// This is a legacy escape hatch for an unregistered `TextureData`. Apply the returned
    /// transition to that exact texture through unsafe `TextureUpdateResult::apply_to_texture`.
    /// Safe image widgets do not accept borrowed `TextureData`; normal applications should
    /// register `OwnedTextureData` with the Context and let [`Self::prepare_frame`] reconcile
    /// request-bound feedback instead.
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
    /// completion or queue ordering for the upload submitted by this method. An upload returns
    /// `RendererError::Vulkan(vk::Result::NOT_READY)` when all staging slots are occupied.
    pub unsafe fn update_texture_unchecked(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
        self.ensure_operational()?;
        self.reap_completed_uploads()?;

        let status = texture_data.status();
        match status {
            TextureStatus::WantCreate => self.create_legacy_texture(texture_data),
            TextureStatus::WantUpdates => {
                let id = texture_data.tex_id().id();
                let Some(existing) = self.textures.textures.get(&id) else {
                    return self.create_legacy_texture(texture_data);
                };
                let (x, y, width, height) =
                    clamp_rect(texture_data.update_rect(), existing.width, existing.height);
                if width == 0 || height == 0 {
                    return Ok(TextureUpdateResult::Updated);
                }
                let Some(pixels) = texture_data_to_rgba_subrect(texture_data, x, y, width, height)
                else {
                    return Ok(TextureUpdateResult::Failed);
                };
                let pixels = ImageUploadPixels {
                    region: ImageUploadRegion {
                        x,
                        y,
                        width,
                        height,
                    },
                    rgba: Cow::Owned(pixels),
                };
                let copy = ImageUpload::subrect(existing.image, &pixels);
                self.submit_texture_uploads(&[copy])?;
                Ok(TextureUpdateResult::Updated)
            }
            TextureStatus::WantDestroy => {
                let id = texture_data.tex_id().id();
                if self.textures.textures.contains_key(&id) {
                    self.wait_for_pending_uploads()?;
                }
                if let Some(texture) = self.textures.textures.remove(&id) {
                    self.release_texture(texture)?;
                }
                Ok(TextureUpdateResult::Destroyed)
            }
            TextureStatus::OK | TextureStatus::Destroyed => Ok(TextureUpdateResult::NoAction),
        }
    }

    fn create_legacy_texture(&mut self, data: &TextureData) -> RendererResult<TextureUpdateResult> {
        let (width, height) = (data.width(), data.height());
        if width == 0 || height == 0 {
            return Ok(TextureUpdateResult::Failed);
        }
        let Some(pixels) = texture_data_to_rgba_full(data) else {
            return Ok(TextureUpdateResult::Failed);
        };
        let old_id = data.tex_id().id();
        let id = if self.textures.textures.contains_key(&old_id) {
            self.wait_for_pending_uploads()?;
            old_id
        } else {
            self.textures.allocate_id()
        };
        self.uploads.ensure_available()?;
        let texture = self.allocate_texture(width, height)?;
        let pixels = ImageUploadPixels::full_image(width, height, Cow::Owned(pixels));
        let copy = ImageUpload::new_image(texture.image, std::slice::from_ref(&pixels));
        if let Err(error) = self.submit_texture_uploads(&[copy]) {
            self.rollback_upload_texture(texture);
            return Err(error);
        }
        if let Some(old) = self.textures.textures.insert(id, texture) {
            self.release_texture(old)?;
        }
        Ok(TextureUpdateResult::Created {
            texture_id: id.into(),
        })
    }

    fn allocate_texture(&mut self, width: u32, height: u32) -> RendererResult<VulkanTexture> {
        VulkanTexture::create(
            &self.device,
            &mut self.allocator,
            &self.resources,
            width,
            height,
        )
    }

    fn release_texture(&mut self, texture: VulkanTexture) -> RendererResult<()> {
        texture.destroy(
            &self.device,
            &mut self.allocator,
            self.resources.descriptor_pool,
        )
    }

    fn rollback_upload_texture(&mut self, texture: VulkanTexture) {
        if self.uploads.has_unconfirmed_submission() {
            // Keep the destination until the same teardown proof as its command buffer and source.
            self.textures.unconfirmed_uploads.push(texture);
        } else {
            let _ = self.release_texture(texture);
        }
    }

    pub(super) fn create_default_texture(&mut self) -> RendererResult<u64> {
        let texture = self.allocate_texture(1, 1)?;
        let id = self.textures.allocate_id();
        self.textures.textures.insert(id, texture);
        self.default_texture_pending = true;
        Ok(id)
    }
}

impl AshRenderer {
    pub(super) fn process_texture_requests(
        &mut self,
        frame: &PendingFrame<'_>,
    ) -> RendererResult<Vec<TextureFeedback>> {
        let mut staged = Vec::new();
        let prepared = self
            .stage_texture_requests(frame, &mut staged)
            .and_then(|feedback| {
                self.submit_frame_uploads(&staged)?;
                Ok(feedback)
            });
        let feedback = match prepared {
            Ok(feedback) => feedback,
            Err(error) => {
                if matches!(&error, RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST)) {
                    self.uploads.remember_device_loss();
                }
                for upload in staged {
                    self.rollback_upload_texture(upload.managed.texture);
                }
                return Err(error);
            }
        };

        // The upload barriers make these images available to later draws on the same queue.
        // Only successful submissions enter the active map or supersede a previous image.
        // Core supplies one request per texture, so a staged identity cannot have been sealed.
        for StagedManagedUpload {
            snapshot_id,
            managed,
            replacement,
            ..
        } in staged
        {
            if let Some(reservation) = replacement {
                self.textures
                    .install_managed_replacement(snapshot_id, managed, reservation);
            } else {
                let texture_id = managed.texture_id;
                self.textures
                    .managed_textures
                    .insert(snapshot_id, managed)
                    .expect("a staged create is not sealed");
                self.textures
                    .managed_ids
                    .insert(texture_id.id(), snapshot_id);
            }
        }
        Ok(feedback)
    }

    fn submit_frame_uploads(&mut self, staged: &[StagedManagedUpload<'_>]) -> RendererResult<()> {
        const WHITE_PIXEL: [u8; 4] = [255; 4];
        let white = ImageUploadPixels::full_image(1, 1, Cow::Borrowed(&WHITE_PIXEL));
        let mut copies = Vec::new();
        if self.default_texture_pending {
            let texture = &self.textures.textures[&self.default_texture_id];
            copies.push(ImageUpload::new_image(
                texture.image,
                std::slice::from_ref(&white),
            ));
        }
        copies.extend(staged.iter().map(|upload| {
            let texture = &upload.managed.texture;
            match upload.previous_image {
                Some(previous) => ImageUpload::replacement(
                    texture.image,
                    previous,
                    texture.width,
                    texture.height,
                    &upload.pixels,
                ),
                None => ImageUpload::new_image(texture.image, &upload.pixels),
            }
        }));
        self.submit_texture_uploads(&copies)?;
        self.default_texture_pending = false;
        Ok(())
    }

    fn stage_texture_requests<'pixels>(
        &mut self,
        frame: &'pixels PendingFrame<'_>,
        staged: &mut Vec<StagedManagedUpload<'pixels>>,
    ) -> RendererResult<Vec<TextureFeedback>> {
        let requests = frame.texture_requests();
        let request_epoch = frame.epoch().sequence();
        let mut feedback = Vec::with_capacity(requests.len());
        for request in requests {
            let snapshot_id = request.texture();
            let outcome = match request.operation() {
                TextureOp::Destroy => {
                    self.textures
                        .managed_textures
                        .seal_destroy(snapshot_id, request_epoch);
                    match self.textures.request_managed_retirement(snapshot_id) {
                        Ok(RetirementRequest::Queued(_) | RetirementRequest::Pending) => {
                            request.retry()
                        }
                        Ok(RetirementRequest::Retired) => {
                            let destroyed = request.destroyed()?;
                            self.textures
                                .managed_textures
                                .acknowledge_destroy(snapshot_id, request_epoch);
                            destroyed
                        }
                        Err(()) => return Err(retirement_exhausted()),
                    }
                }
                _ if self.textures.managed_textures.is_destroyed(snapshot_id) => {
                    request.superseded()
                }
                TextureOp::Create { .. } | TextureOp::Update { .. } => {
                    request.uploaded(self.stage_managed_upload(request, staged)?)?
                }
            };
            feedback.push(outcome);
        }
        Ok(feedback)
    }

    fn stage_managed_upload<'pixels>(
        &mut self,
        request: &'pixels TextureRequest,
        staged: &mut Vec<StagedManagedUpload<'pixels>>,
    ) -> RendererResult<TextureId> {
        let snapshot_id = request.texture();
        let upload_identity = request.upload_identity().expect("only uploads are staged");
        let mut existing = self.textures.managed_textures.get_mut(snapshot_id);
        if let Some(existing) = &existing
            && existing.upload_identity == upload_identity
        {
            return Ok(existing.texture_id);
        }

        let (width, height, previous_image, pixels) = match request.operation() {
            TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => {
                if self
                    .textures
                    .retiring_textures
                    .contains_key(&ManagedTextureRetirementKey::Destroyed(snapshot_id))
                {
                    return Err(RendererError::InvalidRenderState(format!(
                        "managed texture {snapshot_id:?} was recreated while retirement is pending"
                    )));
                }
                if existing.as_ref().is_some_and(|existing| {
                    existing.texture.width != *width || existing.texture.height != *height
                }) {
                    return Err(RendererError::InvalidRenderState(format!(
                        "managed texture {snapshot_id:?} create dimensions changed during retry"
                    )));
                }
                self.uploads.ensure_available()?;
                let rgba = texture_upload_to_rgba(*format, *width, *height, *row_pitch, pixels)
                    .ok_or_else(|| {
                        RendererError::InvalidRenderState(format!(
                            "managed texture {snapshot_id:?} has an invalid create upload layout"
                        ))
                    })?;
                let pixels = vec![ImageUploadPixels::full_image(*width, *height, rgba)];
                (*width, *height, None, pixels)
            }
            TextureOp::Update {
                format,
                width,
                height,
                rects,
            } => {
                let Some(existing) = existing.as_mut() else {
                    return Err(RendererError::InvalidRenderState(format!(
                        "managed texture {snapshot_id:?} received an update before create"
                    )));
                };
                if existing.texture.width != *width || existing.texture.height != *height {
                    return Err(RendererError::InvalidRenderState(format!(
                        "managed texture {snapshot_id:?} update dimensions changed without create"
                    )));
                }
                let pixels = managed_update_pixels(snapshot_id, *format, [*width, *height], rects)?;
                if pixels.is_empty() {
                    existing.upload_identity = upload_identity;
                    return Ok(existing.texture_id);
                }
                self.uploads.ensure_available()?;
                (*width, *height, Some(existing.texture.image), pixels)
            }
            TextureOp::Destroy => unreachable!("only uploads are staged"),
        };

        let (texture_id, replacement) = if let Some(existing) = existing {
            let texture_id = existing.texture_id;
            let reservation = self
                .textures
                .reserve_superseded_retirement()
                .ok_or_else(retirement_exhausted)?;
            (texture_id, Some(reservation))
        } else {
            (self.textures.allocate_id().into(), None)
        };
        let texture = self.allocate_texture(width, height)?;
        staged.push(StagedManagedUpload {
            snapshot_id,
            managed: ManagedVulkanTexture {
                texture_id,
                texture,
                upload_identity,
            },
            replacement,
            previous_image,
            pixels,
        });
        Ok(texture_id)
    }
}

fn managed_update_pixels(
    snapshot_id: SnapshotTextureId,
    format: ImGuiTextureFormat,
    [width, height]: [u32; 2],
    rects: &[TextureUploadRect],
) -> RendererResult<Vec<ImageUploadPixels<'_>>> {
    let rects_in_bounds = DirtyRectNormalizer::new([width, height]);
    let mut pixels = Vec::with_capacity(rects.len());
    for upload in rects {
        let rect = upload.rect;
        if rect.w == 0 || rect.h == 0 {
            continue;
        }
        if rects_in_bounds.validate(rect).is_err() {
            return Err(RendererError::InvalidRenderState(format!(
                "managed texture {snapshot_id:?} update rectangle is out of bounds"
            )));
        }
        let region = ImageUploadRegion {
            x: u32::from(rect.x),
            y: u32::from(rect.y),
            width: u32::from(rect.w),
            height: u32::from(rect.h),
        };
        let rgba = texture_upload_to_rgba(
            format,
            region.width,
            region.height,
            upload.row_pitch,
            &upload.data,
        )
        .ok_or_else(|| {
            RendererError::InvalidRenderState(format!(
                "managed texture {snapshot_id:?} has an invalid update upload layout"
            ))
        })?;
        pixels.push(ImageUploadPixels { region, rgba });
    }
    Ok(pixels)
}

fn retirement_exhausted() -> RendererError {
    RendererError::InvalidRenderState("managed texture retirement batch space is exhausted".into())
}

pub(super) fn texture_upload_to_rgba(
    format: ImGuiTextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &[u8],
) -> Option<Cow<'_, [u8]>> {
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
    if format == ImGuiTextureFormat::RGBA32 && row_pitch == packed_row {
        return Some(Cow::Borrowed(&pixels[..required]));
    }

    let mut rgba = vec![0_u8; width.checked_mul(height)?.checked_mul(4)?];
    for row in 0..height {
        let source = &pixels[row * row_pitch..row * row_pitch + packed_row];
        let destination = &mut rgba[row * width * 4..(row + 1) * width * 4];
        match format {
            ImGuiTextureFormat::RGBA32 => destination.copy_from_slice(source),
            ImGuiTextureFormat::Alpha8 => {
                for (pixel, alpha) in destination.as_chunks_mut::<4>().0.iter_mut().zip(source) {
                    pixel.copy_from_slice(&[255, 255, 255, *alpha]);
                }
            }
        }
    }
    Some(Cow::Owned(rgba))
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

#[cfg(test)]
mod operational_gate_tests {
    use super::*;
    use dear_imgui_rs::FramePrepareOptions;
    use dear_imgui_rs::texture::OwnedTextureData;

    fn destroyed_renderer(context: &Context) -> AshRenderer {
        let device = unsafe { Device::load_with(|_| std::ptr::null(), vk::Device::null()) };
        let mut allocator = RendererAllocator::new(Allocator::new_default(
            vk::PhysicalDeviceMemoryProperties::default(),
        ));
        allocator.release().unwrap();
        AshRenderer {
            device,
            allocator,
            queue: vk::Queue::null(),
            command_pool: vk::CommandPool::null(),
            resources: VulkanRendererResources::empty(),
            textures: TextureManager::new(),
            consumer: None,
            context_state: std::rc::Rc::new(RendererContextState::prepare(context).unwrap()),
            publication_attachment: None,
            default_texture_id: 0,
            default_texture_pending: false,
            options: Options::default(),
            frames: Frames::new(0),
            destroyed: true,
            uploads: UploadRing::new(0),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            render_mode: super::lifecycle::test_render_mode(),
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
        let texture_data =
            OwnedTextureData::from_pixels(ImGuiTextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();

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

        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        context.prepare_frame(
            FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0).renderer_has_textures(),
        );
        context.frame();
        let frame = context.render(&consumer);
        assert_destroyed(renderer.prepare_frame(frame));
        assert_destroyed(renderer.shutdown(&mut context));

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        assert_destroyed(renderer.viewport_pipeline(vk::Format::R8G8B8A8_UNORM));
    }
}

#[cfg(test)]
mod managed_lifecycle_tests {
    use super::*;
    use ash::vk::Handle;
    use dear_imgui_rs::FramePrepareOptions;
    use dear_imgui_rs::texture::{
        OwnedTextureData, TextureRect, TextureRegion, TextureSubresource,
    };

    fn managed_texture(
        texture_id: TextureId,
        image: u64,
        descriptor_set: u64,
        [width, height]: [u32; 2],
        upload_identity: TextureUploadIdentity,
    ) -> ManagedVulkanTexture {
        ManagedVulkanTexture {
            texture_id,
            texture: VulkanTexture {
                image: vk::Image::from_raw(image),
                image_mem: Memory::Default(vk::DeviceMemory::from_raw(image + 100)),
                image_view: vk::ImageView::from_raw(image + 200),
                descriptor_set: vk::DescriptorSet::from_raw(descriptor_set),
                width,
                height,
            },
            upload_identity,
        }
    }

    #[test]
    fn managed_update_packs_only_nonempty_rectangles_in_request_order() {
        let context = Context::create();
        let snapshot_id = SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 7,
            generation: 3,
        };
        let rects = [
            TextureUploadRect {
                rect: TextureRect {
                    x: 2,
                    y: 1,
                    w: 1,
                    h: 2,
                },
                row_pitch: 2,
                data: vec![10, 99, 20],
            },
            TextureUploadRect {
                rect: TextureRect {
                    x: 2,
                    y: 2,
                    w: 1,
                    h: 0,
                },
                row_pitch: 0,
                data: vec![],
            },
            TextureUploadRect {
                rect: TextureRect {
                    x: 2,
                    y: 2,
                    w: 1,
                    h: 1,
                },
                row_pitch: 1,
                data: vec![30],
            },
        ];
        let pixels =
            managed_update_pixels(snapshot_id, ImGuiTextureFormat::Alpha8, [3, 3], &rects).unwrap();
        assert_eq!(pixels.len(), 2);
        assert_eq!(
            pixels[0].region,
            ImageUploadRegion {
                x: 2,
                y: 1,
                width: 1,
                height: 2
            }
        );
        assert_eq!(
            pixels[0].rgba.as_ref(),
            &[255, 255, 255, 10, 255, 255, 255, 20]
        );
        assert_eq!(
            pixels[1].region,
            ImageUploadRegion {
                x: 2,
                y: 2,
                width: 1,
                height: 1
            }
        );
        assert_eq!(pixels[1].rgba.as_ref(), &[255, 255, 255, 30]);
        assert_eq!(
            pixels.iter().map(|patch| patch.rgba.len()).sum::<usize>(),
            12
        );
        assert!(
            managed_update_pixels(
                snapshot_id,
                ImGuiTextureFormat::Alpha8,
                [3, 3],
                &rects[1..2]
            )
            .unwrap()
            .is_empty()
        );

        assert!(matches!(
            managed_update_pixels(snapshot_id, ImGuiTextureFormat::Alpha8, [2, 3], &rects),
            Err(RendererError::InvalidRenderState(message)) if message.contains("out of bounds")
        ));
        let truncated = [TextureUploadRect {
            data: vec![10, 99],
            ..rects[0].clone()
        }];
        assert!(matches!(
            managed_update_pixels(snapshot_id, ImGuiTextureFormat::Alpha8, [3, 3], &truncated),
            Err(RendererError::InvalidRenderState(message)) if message.contains("invalid update upload layout")
        ));
    }

    #[test]
    fn managed_update_retries_only_until_its_revision_is_submitted() {
        use crate::renderer::lifecycle::renderer_for_test;

        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let owned =
            OwnedTextureData::from_pixels(ImGuiTextureFormat::RGBA32, 4, 4, &[0; 64]).unwrap();
        let managed_id = context.register_texture(owned);
        let snapshot_id = SnapshotTextureId::User(managed_id);
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let initial = context
            .begin_frame()
            .render(renderer.renderer_consumer().unwrap());
        for request in initial.texture_requests() {
            let TextureOp::Create { width, height, .. } = request.operation() else {
                panic!("expected the initial create requests");
            };
            let id = renderer.textures.allocate_id();
            let texture = managed_texture(
                id.into(),
                id,
                id,
                [*width, *height],
                request.upload_identity().unwrap(),
            );
            renderer.textures.managed_ids.insert(id, request.texture());
            renderer
                .textures
                .managed_textures
                .insert(request.texture(), texture)
                .unwrap();
        }
        let created = renderer
            .textures
            .managed_textures
            .get(snapshot_id)
            .unwrap()
            .upload_identity;
        let (frame, retirement) = renderer.prepare_frame(initial).unwrap();
        assert_eq!(retirement, None);
        drop(frame);

        context
            .try_with_texture_mut(managed_id, |mut texture| {
                texture.update_subresource(TextureSubresource::new(
                    TextureRegion::new(2, 1, 1, 1).unwrap(),
                    4,
                    &[17, 18, 19, 20],
                ))
            })
            .unwrap();
        let pending = context
            .begin_frame()
            .render(renderer.renderer_consumer().unwrap());
        let update = pending
            .texture_requests()
            .iter()
            .find(|request| request.texture() == snapshot_id)
            .unwrap();
        let TextureOp::Update { rects, .. } = update.operation() else {
            panic!("expected an update after the initial feedback");
        };
        assert_eq!(rects.len(), 1);
        assert_eq!(
            rects[0].rect,
            TextureRect {
                x: 2,
                y: 1,
                w: 1,
                h: 1
            }
        );
        let updated = update.upload_identity().unwrap();
        assert_ne!(created, updated);
        assert!(matches!(
            renderer.prepare_frame(pending),
            Err(RendererError::Vulkan(vk::Result::NOT_READY))
        ));
        assert_eq!(
            renderer
                .textures
                .managed_textures
                .get(snapshot_id)
                .unwrap()
                .upload_identity,
            created
        );
        assert!(
            renderer
                .textures
                .retiring_textures
                .pending_batch()
                .is_none()
        );

        let retry = context
            .begin_frame()
            .render(renderer.renderer_consumer().unwrap());
        let update = retry
            .texture_requests()
            .iter()
            .find(|request| request.texture() == snapshot_id)
            .unwrap();
        assert_eq!(update.upload_identity(), Some(updated));
        // Model a successful submission whose feedback was abandoned, with no available slot.
        renderer
            .textures
            .managed_textures
            .get_mut(snapshot_id)
            .unwrap()
            .upload_identity = updated;
        let (frame, retirement) = renderer.prepare_frame(retry).unwrap();
        assert_eq!(retirement, None);
        drop(frame);
        // All handles were synthesized for the retry test.
        renderer.textures.managed_textures = ManagedTextureTable::default();
    }

    #[test]
    fn superseded_then_destroyed_texture_preserves_mapping_until_final_completion() {
        let mut context = Context::create();
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame().render(&consumer);
        let request = &frame.texture_requests()[0];
        let snapshot_id = request.texture();
        let upload_identity = request.upload_identity().unwrap();
        let texture_id = TextureId::from(41_u64);
        let mut manager = TextureManager::new();
        manager.managed_ids.insert(texture_id.id(), snapshot_id);
        manager
            .managed_textures
            .insert(
                snapshot_id,
                managed_texture(texture_id, 1, 11, [1, 1], upload_identity),
            )
            .unwrap();

        let reservation = manager.reserve_superseded_retirement().unwrap();
        let superseded_batch = reservation.batch();
        manager.install_managed_replacement(
            snapshot_id,
            managed_texture(texture_id, 2, 22, [1, 1], upload_identity),
            reservation,
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
            manager
                .managed_textures
                .get(snapshot_id)
                .unwrap()
                .texture
                .image
                .as_raw(),
            2
        );

        let destroy_epoch = 9;
        manager
            .managed_textures
            .seal_destroy(snapshot_id, destroy_epoch);
        let RetirementRequest::Queued(destroy_batch) = manager
            .request_managed_retirement(snapshot_id)
            .expect("active replacement should enter destroy retirement")
        else {
            panic!("active replacement was acknowledged before retirement");
        };
        assert!(manager.managed_textures.get(snapshot_id).is_none());
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
        assert!(manager.managed_textures.is_destroyed(snapshot_id));

        manager
            .managed_textures
            .acknowledge_destroy(snapshot_id, destroy_epoch);
        manager
            .managed_textures
            .prune_tombstones(|_| Some(destroy_epoch - 1));
        assert!(manager.managed_textures.is_destroyed(snapshot_id));
        manager
            .managed_textures
            .prune_tombstones(|_| Some(destroy_epoch));
        assert!(!manager.managed_textures.is_destroyed(snapshot_id));
    }
}
