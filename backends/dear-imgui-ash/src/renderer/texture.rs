use super::*;

#[derive(Debug)]
pub(super) struct VulkanTexture {
    pub(super) image: vk::Image,
    pub(super) image_mem: Memory,
    pub(super) image_view: vk::ImageView,
    pub(super) sampler: vk::Sampler,
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
            device.destroy_sampler(self.sampler, None);
            device.destroy_image_view(self.image_view, None);
            let _ = device.free_descriptor_sets(pool, &[self.descriptor_set]);
        }
        let _ = allocator.destroy_image(device, self.image, self.image_mem);
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct ExternalTextureBinding {
    pub(super) descriptor_set: vk::DescriptorSet,
    pub(super) image_view: Option<vk::ImageView>,
    pub(super) sampler: Option<vk::Sampler>,
    pub(super) free_descriptor_set: bool,
}

impl ExternalTextureBinding {
    fn borrowed_descriptor_set(descriptor_set: vk::DescriptorSet) -> Self {
        Self {
            descriptor_set,
            image_view: None,
            sampler: None,
            free_descriptor_set: false,
        }
    }

    fn owned_descriptor_set(
        descriptor_set: vk::DescriptorSet,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> Self {
        Self {
            descriptor_set,
            image_view: Some(image_view),
            sampler: Some(sampler),
            free_descriptor_set: true,
        }
    }
}

#[derive(Debug)]
pub(super) struct TextureManager {
    pub(super) textures: HashMap<u64, VulkanTexture>,
    pub(super) external_textures: HashMap<u64, ExternalTextureBinding>,
    pub(super) next_id: u64,
}

impl TextureManager {
    pub(super) fn new() -> Self {
        Self {
            textures: HashMap::new(),
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
        } else {
            self.external_textures
                .get(&texture_id)
                .map(|b| b.descriptor_set)
        }
    }

    pub(super) fn register_external_descriptor_set(&mut self, set: vk::DescriptorSet) -> u64 {
        let id = self.allocate_id();
        self.external_textures
            .insert(id, ExternalTextureBinding::borrowed_descriptor_set(set));
        id
    }

    pub(super) fn register_external_texture(
        &mut self,
        set: vk::DescriptorSet,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> u64 {
        let id = self.allocate_id();
        self.external_textures.insert(
            id,
            ExternalTextureBinding::owned_descriptor_set(set, image_view, sampler),
        );
        id
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct TextureWriteback {
    pub(super) texture: *mut dear_imgui_rs::sys::ImTextureData,
    pub(super) tex_id: Option<TextureId>,
    pub(super) status: TextureStatus,
}

impl TextureWriteback {
    pub(super) fn apply(self) {
        if self.texture.is_null() {
            return;
        }

        unsafe {
            if let Some(tex_id) = self.tex_id {
                dear_imgui_rs::sys::ImTextureData_SetTexID(self.texture, tex_id.id() as _);
            }
            if self.status == TextureStatus::Destroyed {
                (*self.texture).WantDestroyNextFrame = true;
            }
            dear_imgui_rs::sys::ImTextureData_SetStatus(self.texture, self.status.into());
        }
    }
}

pub(super) struct PendingTextureCreate {
    pub(super) id: u64,
    pub(super) texture: Texture,
    pub(super) descriptor_set: vk::DescriptorSet,
    pub(super) staging_buffer: vk::Buffer,
    pub(super) staging_mem: Option<Memory>,
    pub(super) w: u32,
    pub(super) h: u32,
}

impl PendingTextureCreate {
    pub(super) fn into_vulkan_texture(mut self) -> (u64, VulkanTexture, vk::Buffer, Memory) {
        let staging_mem = self
            .staging_mem
            .take()
            .expect("pending create staging memory must be consumed exactly once");
        let Texture {
            image,
            image_mem,
            image_view,
            sampler,
        } = self.texture;
        (
            self.id,
            VulkanTexture {
                image,
                image_mem,
                image_view,
                sampler,
                descriptor_set: self.descriptor_set,
                width: self.w,
                height: self.h,
            },
            self.staging_buffer,
            staging_mem,
        )
    }
}

pub(super) struct PendingTextureUpdate {
    pub(super) image: vk::Image,
    pub(super) staging_buffer: vk::Buffer,
    pub(super) staging_mem: Option<Memory>,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) w: u32,
    pub(super) h: u32,
}

impl PendingTextureUpdate {
    pub(super) fn into_staging(mut self) -> (vk::Buffer, Memory) {
        let staging_mem = self
            .staging_mem
            .take()
            .expect("pending update staging memory must be consumed exactly once");
        (self.staging_buffer, staging_mem)
    }
}

impl AshRenderer {
    pub fn register_texture_descriptor_set(&mut self, set: vk::DescriptorSet) -> TextureId {
        TextureId::from(self.textures.register_external_descriptor_set(set))
    }

    /// Remove a previously registered external texture descriptor set.
    pub fn remove_texture_descriptor_set(&mut self, id: TextureId) {
        self.unregister_texture(id);
    }

    /// Register an external `vk::ImageView` + `vk::Sampler` as a legacy `TextureId`.
    ///
    /// This is the Vulkan equivalent of `dear-imgui-wgpu::WgpuRenderer::register_external_texture_with_sampler()`.
    /// The returned `TextureId` can be passed to `ui.image(tex_id, size)`.
    ///
    /// Note: this only allocates a descriptor set; the image and sampler are owned by the caller
    /// and must outlive rendering that references the returned id.
    pub fn register_external_texture_with_sampler(
        &mut self,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> RendererResult<TextureId> {
        let set = create_vulkan_descriptor_set(
            &self.device,
            self.descriptor_set_layout,
            self.descriptor_pool,
            image_view,
            sampler,
        )?;
        Ok(TextureId::from(
            self.textures
                .register_external_texture(set, image_view, sampler),
        ))
    }

    /// Update the view for an already-registered external texture.
    ///
    /// Returns false if the texture id is not an external texture registered via
    /// `register_external_texture_with_sampler()`.
    pub fn update_external_texture_view(
        &mut self,
        texture_id: TextureId,
        image_view: vk::ImageView,
    ) -> bool {
        let id = texture_id.id();
        let Some(binding) = self.textures.external_textures.get_mut(&id) else {
            return false;
        };
        if !binding.free_descriptor_set {
            return false;
        }
        let Some(sampler) = binding.sampler else {
            return false;
        };

        binding.image_view = Some(image_view);
        unsafe {
            let image_info = [vk::DescriptorImageInfo {
                sampler,
                image_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let write_desc_sets = [vk::WriteDescriptorSet::default()
                .dst_set(binding.descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info)];
            self.device.update_descriptor_sets(&write_desc_sets, &[]);
        }
        true
    }

    /// Update (or set) a custom sampler for an already-registered external texture.
    ///
    /// Returns false if the texture id is not an external texture registered via
    /// `register_external_texture_with_sampler()`.
    pub fn update_external_texture_sampler(
        &mut self,
        texture_id: TextureId,
        sampler: vk::Sampler,
    ) -> bool {
        let id = texture_id.id();
        let Some(binding) = self.textures.external_textures.get_mut(&id) else {
            return false;
        };
        if !binding.free_descriptor_set {
            return false;
        }
        let Some(image_view) = binding.image_view else {
            return false;
        };

        binding.sampler = Some(sampler);
        unsafe {
            let image_info = [vk::DescriptorImageInfo {
                sampler,
                image_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let write_desc_sets = [vk::WriteDescriptorSet::default()
                .dst_set(binding.descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info)];
            self.device.update_descriptor_sets(&write_desc_sets, &[]);
        }
        true
    }

    /// Unregister a texture id.
    ///
    /// For external textures registered via `register_external_texture_with_sampler()`, this also
    /// frees the underlying descriptor set from the pool. For descriptor sets registered via
    /// `register_texture_descriptor_set()`, this simply forgets the id (the descriptor set remains
    /// owned by the caller).
    pub fn unregister_texture(&mut self, texture_id: TextureId) {
        let id = texture_id.id();
        if let Some(binding) = self.textures.external_textures.remove(&id) {
            if binding.free_descriptor_set {
                unsafe {
                    let _ = self
                        .device
                        .free_descriptor_sets(self.descriptor_pool, &[binding.descriptor_set]);
                }
            }
        }
    }

    /// Update a single texture manually.
    ///
    /// This mirrors the `dear-imgui-wgpu` API and is useful when the texture is not registered
    /// in ImGui's `PlatformIO.Textures[]` list (e.g. user-created `ImTextureData` that isn't
    /// registered via ImGui's experimental `RegisterUserTexture()` API).
    ///
    /// Call this before rendering if you pass `&mut TextureData` to widgets (e.g. `ui.image()`),
    /// otherwise `ImDrawCmd_GetTexID()` may assert if `TexID` is still invalid.
    pub fn update_texture(
        &mut self,
        texture_data: &TextureData,
    ) -> RendererResult<TextureUpdateResult> {
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

                let (texture, staging_buffer, staging_mem) = Texture::create(
                    &self.device,
                    &mut self.allocator,
                    w,
                    h,
                    self.options.texture_format,
                    &pixels,
                )?;

                let descriptor_set = match create_vulkan_descriptor_set(
                    &self.device,
                    self.descriptor_set_layout,
                    self.descriptor_pool,
                    texture.image_view,
                    texture.sampler,
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
                            let _ = self
                                .device
                                .free_descriptor_sets(self.descriptor_pool, &[descriptor_set]);
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
                });

                if let Some(old) = self.textures.textures.remove(&id) {
                    old.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
                }
                self.textures.textures.insert(
                    id,
                    VulkanTexture {
                        image: texture.image,
                        image_mem: texture.image_mem,
                        image_view: texture.image_view,
                        sampler: texture.sampler,
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
                        x,
                        y,
                        w,
                        h,
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
                });

                Ok(TextureUpdateResult::Updated)
            }
            TextureStatus::WantDestroy => {
                let id = texture_data.tex_id().id();
                if self.textures.textures.contains_key(&id) {
                    self.wait_for_pending_uploads()?;
                }
                if let Some(tex) = self.textures.textures.remove(&id) {
                    tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
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

        let (texture, staging_buffer, staging_mem) = Texture::create(
            &self.device,
            &mut self.allocator,
            w,
            h,
            self.options.texture_format,
            &pixels,
        )?;

        let descriptor_set = match create_vulkan_descriptor_set(
            &self.device,
            self.descriptor_set_layout,
            self.descriptor_pool,
            texture.image_view,
            texture.sampler,
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
                        .free_descriptor_sets(self.descriptor_pool, &[descriptor_set]);
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
        });

        if let Some(old) = self.textures.textures.remove(&id) {
            old.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }
        self.textures.textures.insert(
            id,
            VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                sampler: texture.sampler,
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

        let (texture, staging_buffer, staging_mem) = Texture::create(
            &self.device,
            &mut self.allocator,
            1,
            1,
            self.options.texture_format,
            &pixels,
        )?;

        if let Err(err) =
            execute_one_time_commands(&self.device, self.queue, self.command_pool, |cmd| {
                texture.upload(&self.device, cmd, staging_buffer, 1, 1);
            })
        {
            let _ = self
                .allocator
                .destroy_buffer(&self.device, staging_buffer, staging_mem);
            let _ = texture.destroy(&self.device, &mut self.allocator);
            return Err(err);
        }

        if let Err(err) = self
            .allocator
            .destroy_buffer(&self.device, staging_buffer, staging_mem)
        {
            let _ = texture.destroy(&self.device, &mut self.allocator);
            return Err(err);
        }

        let descriptor_set = match create_vulkan_descriptor_set(
            &self.device,
            self.descriptor_set_layout,
            self.descriptor_pool,
            texture.image_view,
            texture.sampler,
        ) {
            Ok(descriptor_set) => descriptor_set,
            Err(err) => {
                let _ = texture.destroy(&self.device, &mut self.allocator);
                return Err(err);
            }
        };

        self.textures.textures.insert(
            texture_id,
            VulkanTexture {
                image: texture.image,
                image_mem: texture.image_mem,
                image_view: texture.image_view,
                sampler: texture.sampler,
                descriptor_set,
                width: 1,
                height: 1,
            },
        );

        Ok(texture_id)
    }

    pub(super) fn process_texture_requests(
        &mut self,
        draw_data: &mut dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        let mut creates: Vec<PendingTextureCreate> = Vec::new();
        let mut updates: Vec<PendingTextureUpdate> = Vec::new();
        let mut writebacks: Vec<TextureWriteback> = Vec::new();

        let mut textures = draw_data.textures_mut();
        while let Some(mut td) = textures.next() {
            let status = td.status();
            let internal_id = td.tex_id().id();
            let needs_create = matches!(status, TextureStatus::WantCreate)
                || (matches!(status, TextureStatus::WantUpdates)
                    && (internal_id == 0 || !self.textures.textures.contains_key(&internal_id)));

            if needs_create {
                let id = if internal_id == 0 || !self.textures.textures.contains_key(&internal_id) {
                    self.textures.allocate_id()
                } else {
                    internal_id
                };
                let replacing_existing = self.textures.textures.contains_key(&id);
                if replacing_existing {
                    self.wait_for_pending_uploads()?;
                }

                let (w, h) = (td.width(), td.height());
                if w == 0 || h == 0 {
                    continue;
                }
                let Some(pixels) = texture_data_to_rgba_full(&td) else {
                    continue;
                };

                let (texture, staging_buffer, staging_mem) = match Texture::create(
                    &self.device,
                    &mut self.allocator,
                    w,
                    h,
                    self.options.texture_format,
                    &pixels,
                ) {
                    Ok(texture) => texture,
                    Err(err) => {
                        self.discard_pending_texture_work(creates, updates);
                        return Err(err);
                    }
                };

                let descriptor_set = match create_vulkan_descriptor_set(
                    &self.device,
                    self.descriptor_set_layout,
                    self.descriptor_pool,
                    texture.image_view,
                    texture.sampler,
                ) {
                    Ok(descriptor_set) => descriptor_set,
                    Err(err) => {
                        let _ = self.allocator.destroy_buffer(
                            &self.device,
                            staging_buffer,
                            staging_mem,
                        );
                        let _ = texture.destroy(&self.device, &mut self.allocator);
                        self.discard_pending_texture_work(creates, updates);
                        return Err(err);
                    }
                };

                creates.push(PendingTextureCreate {
                    id,
                    texture,
                    descriptor_set,
                    staging_buffer,
                    staging_mem: Some(staging_mem),
                    w,
                    h,
                });

                writebacks.push(TextureWriteback {
                    texture: td.as_raw_mut(),
                    tex_id: Some(TextureId::from(id)),
                    status: TextureStatus::OK,
                });
                continue;
            }

            match status {
                TextureStatus::WantCreate => {
                    // Handled by `needs_create` branch above.
                }
                TextureStatus::WantUpdates => {
                    let id = internal_id;
                    let Some(existing) = self.textures.textures.get(&id) else {
                        // If the backend lost its copy but ImGui still asks for updates, fall back
                        // to a full recreate in the next frame.
                        td.set_status(TextureStatus::WantCreate);
                        continue;
                    };

                    let (tw, th, image) = (existing.width, existing.height, existing.image);
                    if tw == 0 || th == 0 {
                        td.set_status(TextureStatus::OK);
                        continue;
                    }

                    let rect = td.update_rect();
                    let (x, y, w, h) = clamp_rect(rect, tw, th);
                    if w == 0 || h == 0 {
                        td.set_status(TextureStatus::OK);
                        continue;
                    }

                    let Some(pixels) = texture_data_to_rgba_subrect(&td, x, y, w, h) else {
                        td.set_status(TextureStatus::OK);
                        continue;
                    };
                    let (staging_buffer, staging_mem) = match create_and_fill_buffer(
                        &self.device,
                        &mut self.allocator,
                        &pixels,
                        vk::BufferUsageFlags::TRANSFER_SRC,
                    ) {
                        Ok(staging) => staging,
                        Err(err) => {
                            self.discard_pending_texture_work(creates, updates);
                            return Err(err);
                        }
                    };

                    updates.push(PendingTextureUpdate {
                        image,
                        staging_buffer,
                        staging_mem: Some(staging_mem),
                        x,
                        y,
                        w,
                        h,
                    });

                    writebacks.push(TextureWriteback {
                        texture: td.as_raw_mut(),
                        tex_id: None,
                        status: TextureStatus::OK,
                    });
                }
                TextureStatus::WantDestroy => {
                    let id = internal_id;
                    if self.textures.textures.contains_key(&id) {
                        if let Err(err) = self.wait_for_pending_uploads() {
                            self.discard_pending_texture_work(creates, updates);
                            return Err(err);
                        }
                    }
                    if let Some(tex) = self.textures.textures.remove(&id) {
                        tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
                    }
                    unsafe {
                        (*td.as_raw_mut()).WantDestroyNextFrame = true;
                    }
                    td.set_status(TextureStatus::Destroyed);
                }
                TextureStatus::OK | TextureStatus::Destroyed => {}
            }
        }
        drop(textures);

        if !creates.is_empty() || !updates.is_empty() {
            let (command_buffer, fence) = match self.submit_upload_commands(|cmd| {
                for c in &creates {
                    c.texture
                        .upload(&self.device, cmd, c.staging_buffer, c.w, c.h);
                }
                for u in &updates {
                    upload_rgba_subrect_to_image(
                        &self.device,
                        cmd,
                        u.staging_buffer,
                        u.image,
                        u.x,
                        u.y,
                        u.w,
                        u.h,
                    );
                }
            }) {
                Ok(upload) => upload,
                Err(err) => {
                    self.discard_pending_texture_work(creates, updates);
                    return Err(err);
                }
            };

            let mut staging: Vec<(vk::Buffer, Memory)> =
                Vec::with_capacity(creates.len() + updates.len());
            let mut created_textures: Vec<(u64, VulkanTexture)> = Vec::with_capacity(creates.len());
            for c in creates {
                let (id, texture, staging_buffer, staging_mem) = c.into_vulkan_texture();
                staging.push((staging_buffer, staging_mem));
                created_textures.push((id, texture));
            }
            for u in updates {
                staging.push(u.into_staging());
            }

            self.in_flight_uploads.push_back(InFlightUpload {
                fence,
                command_buffer,
                staging,
            });

            for (id, texture) in created_textures {
                if let Some(old) = self.textures.textures.remove(&id) {
                    old.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
                }
                self.textures.textures.insert(id, texture);
            }
        }

        for writeback in writebacks {
            writeback.apply();
        }

        Ok(())
    }
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
