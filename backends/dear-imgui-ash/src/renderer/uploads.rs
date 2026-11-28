use super::*;

/// Staging is read by Ash's upload submissions only. Main and secondary draws own their fences.
pub(super) struct UploadRing {
    slots: Vec<UploadSlot>,
    next_slot: usize,
    bytes: Vec<u8>,
    device_state: UploadDeviceState,
}

#[derive(PartialEq, Eq)]
enum UploadDeviceState {
    Ready,
    Lost,
    LostDuringSubmit,
}

#[derive(Default)]
struct UploadSlot {
    fence: vk::Fence,
    /// A non-null command buffer is pending or belongs to a terminal device-loss submission.
    command_buffer: vk::CommandBuffer,
    staging: Option<StagingBuffer>,
}

struct StagingBuffer {
    buffer: vk::Buffer,
    memory: Memory,
    capacity: usize,
}

pub(super) fn grown_buffer_capacity(current: usize, required: usize) -> usize {
    current
        .checked_add(current / 2)
        .unwrap_or(required)
        .max(required)
}

impl UploadRing {
    pub(super) fn new(count: usize) -> Self {
        Self {
            slots: (0..count).map(|_| UploadSlot::default()).collect(),
            next_slot: 0,
            bytes: Vec::new(),
            device_state: UploadDeviceState::Ready,
        }
    }

    pub(super) fn has_pending(&self) -> bool {
        self.slots.iter().any(UploadSlot::is_pending)
    }

    pub(super) fn check_device(&self) -> RendererResult<()> {
        if self.device_state != UploadDeviceState::Ready {
            return Err(vk::Result::ERROR_DEVICE_LOST.into());
        }
        Ok(())
    }

    pub(super) fn has_unconfirmed_submission(&self) -> bool {
        self.device_state == UploadDeviceState::LostDuringSubmit
    }

    pub(super) fn remember_device_loss(&mut self) {
        if self.device_state == UploadDeviceState::Ready {
            self.device_state = UploadDeviceState::Lost;
        }
    }

    fn next_free_slot(&self) -> RendererResult<usize> {
        let (earlier, later) = self.slots.split_at(self.next_slot);
        later
            .iter()
            .chain(earlier)
            .position(|slot| !slot.is_pending())
            .map(|offset| (self.next_slot + offset) % self.slots.len())
            .ok_or_else(|| vk::Result::NOT_READY.into())
    }

    pub(super) fn ensure_available(&self) -> RendererResult<()> {
        self.check_device()?;
        self.next_free_slot().map(|_| ())
    }

    fn reap(&mut self, device: &Device, command_pool: vk::CommandPool) -> RendererResult<()> {
        self.check_device()?;
        for slot in &mut self.slots {
            if !slot.is_pending() {
                continue;
            }
            match unsafe { device.get_fence_status(slot.fence) } {
                Ok(true) => slot.release_commands(device, command_pool),
                Ok(false) => {}
                Err(error) => {
                    if error == vk::Result::ERROR_DEVICE_LOST {
                        self.device_state = UploadDeviceState::Lost;
                    }
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }

    fn submit(
        &mut self,
        device: &Device,
        allocator: &mut RendererAllocator,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        copies: &[ImageUpload<'_>],
    ) -> RendererResult<()> {
        if copies.is_empty() {
            return Ok(());
        }
        self.check_device()?;
        let index = self.next_free_slot()?;
        let mut chunks = copies.iter().flat_map(|copy| copy.pixel_chunks());
        let first = chunks.next().expect("every upload contains pixel data");
        let bytes = if chunks.clone().next().is_none() {
            first
        } else {
            let bytes = &mut self.bytes;
            bytes.clear();
            let chunks = std::iter::once(first).chain(chunks);
            let size = chunks.clone().try_fold(0_usize, |size, chunk| {
                size.checked_add(chunk.len()).ok_or_else(|| {
                    RendererError::Allocator("combined texture upload size overflow".into())
                })
            })?;
            bytes.try_reserve(size).map_err(|error| {
                RendererError::Allocator(format!("could not reserve texture upload bytes: {error}"))
            })?;
            for chunk in chunks {
                bytes.extend_from_slice(chunk);
            }
            bytes.as_slice()
        };

        let slot = &mut self.slots[index];
        let result = slot
            .write_staging(device, allocator, bytes)
            .and_then(|buffer| {
                slot.submit(device, queue, command_pool, |command_buffer| {
                    let mut offset = 0;
                    for copy in copies {
                        copy.record(device, command_buffer, buffer, &mut offset);
                    }
                })
            });
        if matches!(
            &result,
            Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
        ) {
            self.device_state = if slot.is_pending() {
                UploadDeviceState::LostDuringSubmit
            } else {
                UploadDeviceState::Lost
            };
        }
        result?;
        self.next_slot = (index + 1) % self.slots.len();
        Ok(())
    }

    /// Caller has established device idle or terminal device loss.
    pub(super) fn destroy(
        &mut self,
        device: &Device,
        allocator: &mut RendererAllocator,
        command_pool: vk::CommandPool,
    ) -> RendererResult<()> {
        let mut result = Ok(());
        for mut slot in self.slots.drain(..) {
            slot.release_commands(device, command_pool);
            if slot.fence != vk::Fence::null() {
                unsafe { device.destroy_fence(slot.fence, None) };
            }
            if let Some(staging) = slot.staging {
                result =
                    result.and(allocator.destroy_buffer(device, staging.buffer, staging.memory));
            }
        }
        self.bytes = Vec::new();
        result
    }
}

impl UploadSlot {
    fn is_pending(&self) -> bool {
        self.command_buffer != vk::CommandBuffer::null()
    }

    fn release_commands(&mut self, device: &Device, command_pool: vk::CommandPool) {
        if self.is_pending() {
            unsafe { device.free_command_buffers(command_pool, &[self.command_buffer]) };
            self.command_buffer = vk::CommandBuffer::null();
        }
    }

    fn write_staging(
        &mut self,
        device: &Device,
        allocator: &mut RendererAllocator,
        bytes: &[u8],
    ) -> RendererResult<vk::Buffer> {
        let capacity = self.staging.as_ref().map_or(0, |staging| staging.capacity);
        if capacity < bytes.len() {
            let capacity = grown_buffer_capacity(capacity, bytes.len());
            let (buffer, memory) =
                allocator.create_buffer(device, capacity, vk::BufferUsageFlags::TRANSFER_SRC)?;
            if let Some(old) = self.staging.replace(StagingBuffer {
                buffer,
                memory,
                capacity,
            }) {
                allocator.destroy_buffer(device, old.buffer, old.memory)?;
            }
        }
        let staging = self
            .staging
            .as_mut()
            .expect("nonempty upload has staging storage");
        allocator.update_buffer(device, &mut staging.memory, bytes)?;
        Ok(staging.buffer)
    }

    fn submit(
        &mut self,
        device: &Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        record: impl FnOnce(vk::CommandBuffer),
    ) -> RendererResult<()> {
        if self.fence == vk::Fence::null() {
            self.fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None)? };
        }
        // The caller's pool need not support individual command buffer resets.
        let allocate = vk::CommandBufferAllocateInfo::default()
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool)
            .command_buffer_count(1);
        let command_buffer = unsafe { device.allocate_command_buffers(&allocate)?[0] };
        let result = (|| unsafe {
            let begin = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            device.begin_command_buffer(command_buffer, &begin)?;
            record(command_buffer);
            device.end_command_buffer(command_buffer)?;
            device.reset_fences(&[self.fence])?;
            let submit =
                vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
            let result = device.queue_submit(queue, &[submit], self.fence);
            // Vulkan treats device loss from submission like success for pending-resource checks.
            if matches!(result, Ok(()) | Err(vk::Result::ERROR_DEVICE_LOST)) {
                self.command_buffer = command_buffer;
            }
            result
        })();
        if !self.is_pending() {
            unsafe { device.free_command_buffers(command_pool, &[command_buffer]) };
        }
        result.map_err(Into::into)
    }
}

impl AshRenderer {
    pub(super) fn submit_texture_uploads(
        &mut self,
        copies: &[ImageUpload<'_>],
    ) -> RendererResult<()> {
        self.uploads.submit(
            &self.device,
            &mut self.allocator,
            self.queue,
            self.command_pool,
            copies,
        )
    }

    pub(super) fn reap_completed_uploads(&mut self) -> RendererResult<()> {
        self.uploads.reap(&self.device, self.command_pool)
    }

    pub(super) fn wait_for_pending_uploads(&mut self) -> RendererResult<()> {
        self.uploads.check_device()?;
        for slot in self.uploads.slots.iter().filter(|slot| slot.is_pending()) {
            let result = unsafe { self.device.wait_for_fences(&[slot.fence], true, u64::MAX) };
            if let Err(error) = result {
                if error == vk::Result::ERROR_DEVICE_LOST {
                    self.uploads.device_state = UploadDeviceState::Lost;
                }
                return Err(error.into());
            }
        }
        self.reap_completed_uploads()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ash::vk::Handle;

    #[test]
    fn an_unsubmitted_slot_is_reusable_even_if_its_fence_was_reset() {
        let mut ring = UploadRing::new(2);
        ring.slots[0].command_buffer = vk::CommandBuffer::from_raw(1);
        ring.slots[1].fence = vk::Fence::from_raw(2);
        assert_eq!(ring.next_free_slot().unwrap(), 1);

        ring.slots[1].command_buffer = vk::CommandBuffer::from_raw(3);
        assert!(matches!(
            ring.next_free_slot(),
            Err(RendererError::Vulkan(vk::Result::NOT_READY))
        ));

        ring.next_slot = 1;
        ring.slots[0].command_buffer = vk::CommandBuffer::null();
        assert_eq!(ring.next_free_slot().unwrap(), 0);
    }

    #[test]
    fn only_device_loss_during_submission_keeps_commands_pending() {
        use std::ffi::c_void;

        unsafe extern "system" fn allocate(
            _: vk::Device,
            _: *const vk::CommandBufferAllocateInfo<'_>,
            output: *mut vk::CommandBuffer,
        ) -> vk::Result {
            // SAFETY: the test submits with command_buffer_count equal to one.
            unsafe { output.write(vk::CommandBuffer::from_raw(1)) };
            vk::Result::SUCCESS
        }
        unsafe extern "system" fn begin(
            _: vk::CommandBuffer,
            _: *const vk::CommandBufferBeginInfo<'_>,
        ) -> vk::Result {
            vk::Result::SUCCESS
        }
        unsafe extern "system" fn end(_: vk::CommandBuffer) -> vk::Result {
            vk::Result::SUCCESS
        }
        unsafe extern "system" fn reset(_: vk::Device, _: u32, _: *const vk::Fence) -> vk::Result {
            vk::Result::SUCCESS
        }
        unsafe extern "system" fn reset_lost(
            _: vk::Device,
            _: u32,
            _: *const vk::Fence,
        ) -> vk::Result {
            vk::Result::ERROR_DEVICE_LOST
        }
        unsafe extern "system" fn submit_lost(
            _: vk::Queue,
            _: u32,
            _: *const vk::SubmitInfo<'_>,
            _: vk::Fence,
        ) -> vk::Result {
            vk::Result::ERROR_DEVICE_LOST
        }
        unsafe extern "system" fn free(
            _: vk::Device,
            _: vk::CommandPool,
            _: u32,
            _: *const vk::CommandBuffer,
        ) {
        }

        for lost_before_submission in [false, true] {
            // SAFETY: each exercised device entry point uses a matching Vulkan ABI stub.
            let device = unsafe {
                Device::load_with(
                    |name| match name.to_bytes() {
                        b"vkAllocateCommandBuffers" => allocate as *const c_void,
                        b"vkBeginCommandBuffer" => begin as *const c_void,
                        b"vkEndCommandBuffer" => end as *const c_void,
                        b"vkResetFences" if lost_before_submission => reset_lost as *const c_void,
                        b"vkResetFences" => reset as *const c_void,
                        b"vkQueueSubmit" => submit_lost as *const c_void,
                        b"vkFreeCommandBuffers" => free as *const c_void,
                        _ => std::ptr::null(),
                    },
                    vk::Device::null(),
                )
            };
            let pool = vk::CommandPool::null();
            let mut slot = UploadSlot {
                fence: vk::Fence::from_raw(1),
                ..Default::default()
            };
            assert!(matches!(
                slot.submit(&device, vk::Queue::null(), pool, |_| {}),
                Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
            ));
            assert_eq!(slot.is_pending(), !lost_before_submission);
            slot.release_commands(&device, pool);
        }
    }

    #[test]
    fn grows_capacity_by_half_or_to_the_required_size() {
        assert_eq!(grown_buffer_capacity(10, 11), 15);
        assert_eq!(grown_buffer_capacity(10, 24), 24);
        assert_eq!(grown_buffer_capacity(usize::MAX, usize::MAX), usize::MAX);
    }
}
