use super::*;

pub(super) struct InFlightUpload {
    pub(super) fence: vk::Fence,
    pub(super) command_buffer: vk::CommandBuffer,
    pub(super) staging: Vec<(vk::Buffer, Memory)>,
    pub(super) managed_texture: Option<SnapshotTextureId>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum UploadSignature {
    Create {
        format: ImGuiTextureFormat,
        width: u32,
        height: u32,
        row_pitch: usize,
        pixels: Vec<u8>,
    },
    Update {
        format: ImGuiTextureFormat,
        width: u32,
        height: u32,
        rects: Vec<UploadRectSignature>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct UploadRectSignature {
    rect: dear_imgui_rs::texture::TextureRect,
    row_pitch: usize,
    data: Vec<u8>,
}

impl UploadSignature {
    pub(super) fn from_operation(operation: &TextureOp) -> Option<Self> {
        match operation {
            TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => Some(Self::Create {
                format: *format,
                width: *width,
                height: *height,
                row_pitch: *row_pitch,
                pixels: pixels.clone(),
            }),
            TextureOp::Update {
                format,
                width,
                height,
                rects,
            } => Some(Self::Update {
                format: *format,
                width: *width,
                height: *height,
                rects: rects
                    .iter()
                    .map(|upload| UploadRectSignature {
                        rect: upload.rect,
                        row_pitch: upload.row_pitch,
                        data: upload.data.clone(),
                    })
                    .collect(),
            }),
            TextureOp::Destroy => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UploadState {
    Pending,
    Complete,
}

#[derive(Debug)]
struct ManagedUpload {
    signature: UploadSignature,
    texture_id: TextureId,
    state: UploadState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ManagedUploadDecision {
    Submit,
    Wait,
    Ready(TextureId),
}

#[derive(Debug)]
pub(super) struct ManagedUploadTracker<K = SnapshotTextureId> {
    uploads: HashMap<K, ManagedUpload>,
}

impl<K> Default for ManagedUploadTracker<K> {
    fn default() -> Self {
        Self {
            uploads: HashMap::new(),
        }
    }
}

impl<K> ManagedUploadTracker<K>
where
    K: Copy + Eq + std::hash::Hash,
{
    pub(super) fn decide(
        &mut self,
        texture: K,
        signature: &UploadSignature,
    ) -> ManagedUploadDecision {
        let Some(upload) = self.uploads.get(&texture) else {
            return ManagedUploadDecision::Submit;
        };
        if upload.signature == *signature {
            return match upload.state {
                UploadState::Pending => ManagedUploadDecision::Wait,
                UploadState::Complete => {
                    let texture_id = upload.texture_id;
                    self.uploads.remove(&texture);
                    ManagedUploadDecision::Ready(texture_id)
                }
            };
        }
        if upload.state == UploadState::Pending {
            ManagedUploadDecision::Wait
        } else {
            self.uploads.remove(&texture);
            ManagedUploadDecision::Submit
        }
    }

    pub(super) fn submitted(
        &mut self,
        texture: K,
        signature: UploadSignature,
        texture_id: TextureId,
    ) {
        let previous = self.uploads.insert(
            texture,
            ManagedUpload {
                signature,
                texture_id,
                state: UploadState::Pending,
            },
        );
        debug_assert!(previous.is_none());
    }

    pub(super) fn completed(&mut self, texture: K) {
        if let Some(upload) = self.uploads.get_mut(&texture) {
            upload.state = UploadState::Complete;
        }
    }

    pub(super) fn take_completed(&mut self, texture: K) -> Option<TextureId> {
        let upload = self.uploads.get(&texture)?;
        if upload.state != UploadState::Complete {
            return None;
        }
        self.uploads
            .remove(&texture)
            .map(|upload| upload.texture_id)
    }

    pub(super) fn cancel(&mut self, texture: K) {
        self.uploads.remove(&texture);
    }

    pub(super) fn is_pending(&self, texture: K) -> bool {
        self.uploads
            .get(&texture)
            .is_some_and(|upload| upload.state == UploadState::Pending)
    }
}

pub(super) fn finish_managed_upload_gate<K, E>(
    tracker: &mut ManagedUploadTracker<K>,
    texture: K,
    wait_result: Result<(), E>,
) -> Result<Option<TextureId>, E>
where
    K: Copy + Eq + std::hash::Hash,
{
    wait_result?;
    Ok(tracker.take_completed(texture))
}

pub(super) fn finish_destroy_upload_gate<K, E>(
    tracker: &mut ManagedUploadTracker<K>,
    texture: K,
    wait_result: Result<(), E>,
) -> Result<(), E>
where
    K: Copy + Eq + std::hash::Hash,
{
    wait_result?;
    tracker.cancel(texture);
    Ok(())
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

    pub(super) fn discard_pending_texture_update(&mut self, mut pending: PendingTextureUpdate) {
        if let Some(staging_mem) = pending.staging_mem.take() {
            let _ =
                self.allocator
                    .destroy_buffer(&self.device, pending.staging_buffer, staging_mem);
        }
    }

    pub(super) fn reap_completed_uploads(&mut self) -> RendererResult<()> {
        while let Some(front) = self.in_flight_uploads.front() {
            let done = unsafe { self.device.get_fence_status(front.fence)? };
            if !done {
                break;
            }

            let upload = self.in_flight_uploads.pop_front().expect("front exists");
            let mut cleanup_error = None;
            for (buffer, mem) in upload.staging {
                if let Err(error) = self.allocator.destroy_buffer(&self.device, buffer, mem)
                    && cleanup_error.is_none()
                {
                    cleanup_error = Some(error);
                }
            }
            unsafe {
                self.device
                    .free_command_buffers(self.command_pool, &[upload.command_buffer]);
                self.device.destroy_fence(upload.fence, None);
            }
            if let Some(texture) = upload.managed_texture {
                self.managed_uploads.completed(texture);
            }
            if let Some(error) = cleanup_error {
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) fn reap_all_uploads(&mut self) -> RendererResult<()> {
        let mut first_error = None;
        while let Some(upload) = self.in_flight_uploads.pop_front() {
            for (buffer, mem) in upload.staging {
                if let Err(error) = self.allocator.destroy_buffer(&self.device, buffer, mem)
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
            }
            unsafe {
                self.device
                    .free_command_buffers(self.command_pool, &[upload.command_buffer]);
                self.device.destroy_fence(upload.fence, None);
            }
            if let Some(texture) = upload.managed_texture {
                self.managed_uploads.completed(texture);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(super) fn wait_for_managed_upload(
        &mut self,
        texture: SnapshotTextureId,
    ) -> RendererResult<()> {
        let fence = self
            .in_flight_uploads
            .iter()
            .find(|upload| upload.managed_texture == Some(texture))
            .map(|upload| upload.fence)
            .ok_or_else(|| {
                RendererError::InvalidRenderState(format!(
                    "managed texture {texture:?} has pending feedback without an upload fence"
                ))
            })?;
        unsafe {
            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
        }
        self.reap_completed_uploads()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(byte: u8) -> UploadSignature {
        UploadSignature::Create {
            format: ImGuiTextureFormat::RGBA32,
            width: 1,
            height: 1,
            row_pitch: 4,
            pixels: vec![byte; 4],
        }
    }

    #[test]
    fn pending_upload_cannot_produce_feedback_until_completion() {
        let mut tracker = ManagedUploadTracker::<u32>::default();
        let texture = 3;
        let retry_signature = signature(1);
        let texture_id = TextureId::from(42_u64);

        assert_eq!(
            tracker.decide(texture, &retry_signature),
            ManagedUploadDecision::Submit
        );
        tracker.submitted(texture, signature(1), texture_id);
        assert_eq!(
            tracker.decide(texture, &retry_signature),
            ManagedUploadDecision::Wait
        );
        tracker.completed(texture);
        assert_eq!(
            tracker.decide(texture, &retry_signature),
            ManagedUploadDecision::Ready(texture_id)
        );
        assert_eq!(
            tracker.decide(texture, &retry_signature),
            ManagedUploadDecision::Submit
        );
    }

    #[test]
    fn changed_request_waits_and_old_completion_cannot_ack_it() {
        let mut tracker = ManagedUploadTracker::<u32>::default();
        let texture = 4;
        let first = signature(1);
        let changed = signature(2);

        tracker.submitted(texture, first, TextureId::from(9_u64));
        assert_eq!(
            tracker.decide(texture, &changed),
            ManagedUploadDecision::Wait
        );
        tracker.completed(texture);
        assert_eq!(
            tracker.decide(texture, &changed),
            ManagedUploadDecision::Submit
        );
    }

    #[test]
    fn completed_upload_can_be_taken_without_rechecking_its_signature() {
        let mut tracker = ManagedUploadTracker::<u32>::default();
        let texture = 5;
        let texture_id = TextureId::from(10_u64);
        tracker.submitted(texture, signature(1), texture_id);

        assert_eq!(tracker.take_completed(texture), None);
        assert!(tracker.is_pending(texture));

        tracker.completed(texture);
        assert_eq!(tracker.take_completed(texture), Some(texture_id));
        assert_eq!(
            tracker.decide(texture, &signature(2)),
            ManagedUploadDecision::Submit
        );
    }

    #[test]
    fn recoverable_wait_failure_keeps_managed_upload_owned() {
        let mut tracker = ManagedUploadTracker::<u32>::default();
        let texture = 6;
        let texture_id = TextureId::from(12_u64);
        tracker.submitted(texture, signature(2), texture_id);

        let result = finish_managed_upload_gate(&mut tracker, texture, Err("wait failed"));

        assert_eq!(result, Err("wait failed"));
        assert!(tracker.is_pending(texture));
        tracker.completed(texture);
        assert_eq!(
            finish_managed_upload_gate::<_, &str>(&mut tracker, texture, Ok(())),
            Ok(Some(texture_id))
        );
        assert!(!tracker.is_pending(texture));
    }

    #[test]
    fn recoverable_wait_failure_keeps_destroy_upload_owned() {
        let mut tracker = ManagedUploadTracker::<u32>::default();
        let texture = 8;
        tracker.submitted(texture, signature(3), TextureId::from(11_u64));

        let result = finish_destroy_upload_gate(&mut tracker, texture, Err("wait failed"));

        assert_eq!(result, Err("wait failed"));
        assert!(tracker.is_pending(texture));
        tracker.completed(texture);
        finish_destroy_upload_gate::<_, &str>(&mut tracker, texture, Ok(())).unwrap();
        assert!(!tracker.is_pending(texture));
    }
}
