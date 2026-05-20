use super::*;

pub(super) struct InFlightUpload {
    pub(super) fence: vk::Fence,
    pub(super) command_buffer: vk::CommandBuffer,
    pub(super) staging: Vec<(vk::Buffer, Memory)>,
}

impl AshRenderer {
    pub(super) fn submit_upload_commands<F>(
        &self,
        record: F,
    ) -> RendererResult<(vk::CommandBuffer, vk::Fence)>
    where
        F: FnOnce(vk::CommandBuffer),
    {
        let command_buffer = unsafe {
            let alloc_info = vk::CommandBufferAllocateInfo::default()
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_pool(self.command_pool)
                .command_buffer_count(1);
            self.device.allocate_command_buffers(&alloc_info)?[0]
        };

        unsafe {
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            if let Err(err) = self
                .device
                .begin_command_buffer(command_buffer, &begin_info)
            {
                self.device
                    .free_command_buffers(self.command_pool, &[command_buffer]);
                return Err(err.into());
            }
        }

        record(command_buffer);

        unsafe {
            if let Err(err) = self.device.end_command_buffer(command_buffer) {
                self.device
                    .free_command_buffers(self.command_pool, &[command_buffer]);
                return Err(err.into());
            }
        }

        let fence = unsafe {
            match self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
            {
                Ok(fence) => fence,
                Err(err) => {
                    self.device
                        .free_command_buffers(self.command_pool, &[command_buffer]);
                    return Err(err.into());
                }
            }
        };
        let submit_info =
            vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
        unsafe {
            if let Err(err) =
                self.device
                    .queue_submit(self.queue, std::slice::from_ref(&submit_info), fence)
            {
                self.device.destroy_fence(fence, None);
                self.device
                    .free_command_buffers(self.command_pool, &[command_buffer]);
                return Err(err.into());
            }
        }

        Ok((command_buffer, fence))
    }

    pub(super) fn discard_pending_texture_create(&mut self, mut pending: PendingTextureCreate) {
        unsafe {
            let _ = self
                .device
                .free_descriptor_sets(self.descriptor_pool, &[pending.descriptor_set]);
        }
        if let Some(staging_mem) = pending.staging_mem.take() {
            let _ =
                self.allocator
                    .destroy_buffer(&self.device, pending.staging_buffer, staging_mem);
        }
        let _ = pending.texture.destroy(&self.device, &mut self.allocator);
    }

    pub(super) fn discard_pending_texture_update(&mut self, mut pending: PendingTextureUpdate) {
        if let Some(staging_mem) = pending.staging_mem.take() {
            let _ =
                self.allocator
                    .destroy_buffer(&self.device, pending.staging_buffer, staging_mem);
        }
    }

    pub(super) fn discard_pending_texture_work(
        &mut self,
        creates: Vec<PendingTextureCreate>,
        updates: Vec<PendingTextureUpdate>,
    ) {
        for create in creates {
            self.discard_pending_texture_create(create);
        }
        for update in updates {
            self.discard_pending_texture_update(update);
        }
    }

    pub(super) fn reap_completed_uploads(&mut self) -> RendererResult<()> {
        while let Some(front) = self.in_flight_uploads.front() {
            let done = unsafe { self.device.get_fence_status(front.fence)? };
            if !done {
                break;
            }

            let upload = self.in_flight_uploads.pop_front().expect("front exists");
            for (buffer, mem) in upload.staging {
                self.allocator.destroy_buffer(&self.device, buffer, mem)?;
            }
            unsafe {
                self.device
                    .free_command_buffers(self.command_pool, &[upload.command_buffer]);
                self.device.destroy_fence(upload.fence, None);
            }
        }
        Ok(())
    }

    pub(super) fn reap_all_uploads(&mut self) -> RendererResult<()> {
        while let Some(upload) = self.in_flight_uploads.pop_front() {
            for (buffer, mem) in upload.staging {
                self.allocator.destroy_buffer(&self.device, buffer, mem)?;
            }
            unsafe {
                self.device
                    .free_command_buffers(self.command_pool, &[upload.command_buffer]);
                self.device.destroy_fence(upload.fence, None);
            }
        }
        Ok(())
    }

    pub(super) fn wait_for_pending_uploads(&mut self) -> RendererResult<()> {
        for upload in &self.in_flight_uploads {
            unsafe {
                self.device
                    .wait_for_fences(&[upload.fence], true, u64::MAX)?;
            }
        }
        self.reap_all_uploads()
    }
}
