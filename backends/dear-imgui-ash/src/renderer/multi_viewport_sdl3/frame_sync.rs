use super::*;

pub(super) struct FrameSync {
    pub(super) fence: vk::Fence,
    pub(super) command_buffer: vk::CommandBuffer,
    pub(super) image_available: vk::Semaphore,
    pub(super) render_finished: vk::Semaphore,
}

pub(super) fn create_command_pool(
    device: &Device,
    queue_family_index: u32,
) -> RendererResult<vk::CommandPool> {
    let info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    unsafe { Ok(device.create_command_pool(&info, None)?) }
}

pub(super) fn create_frame_sync(
    device: &Device,
    command_pool: vk::CommandPool,
) -> RendererResult<FrameSync> {
    let semaphore_info = vk::SemaphoreCreateInfo::default();
    let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

    let image_available = unsafe { device.create_semaphore(&semaphore_info, None)? };
    let render_finished = match unsafe { device.create_semaphore(&semaphore_info, None) } {
        Ok(render_finished) => render_finished,
        Err(err) => {
            unsafe { device.destroy_semaphore(image_available, None) };
            return Err(err.into());
        }
    };
    let fence = match unsafe { device.create_fence(&fence_info, None) } {
        Ok(fence) => fence,
        Err(err) => {
            unsafe {
                device.destroy_semaphore(render_finished, None);
                device.destroy_semaphore(image_available, None);
            }
            return Err(err.into());
        }
    };

    let mut command_buffers = match unsafe {
        device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )
    } {
        Ok(command_buffers) => command_buffers,
        Err(err) => {
            unsafe {
                device.destroy_fence(fence, None);
                device.destroy_semaphore(render_finished, None);
                device.destroy_semaphore(image_available, None);
            }
            return Err(err.into());
        }
    };
    let Some(command_buffer) = command_buffers.pop() else {
        unsafe {
            device.destroy_fence(fence, None);
            device.destroy_semaphore(render_finished, None);
            device.destroy_semaphore(image_available, None);
        }
        return Err(RendererError::Init(
            "Vulkan command buffer allocation returned no buffers".into(),
        ));
    };

    Ok(FrameSync {
        fence,
        command_buffer,
        image_available,
        render_finished,
    })
}

pub(super) fn destroy_frame_syncs(
    device: &Device,
    command_pool: vk::CommandPool,
    frames: Vec<FrameSync>,
) {
    unsafe {
        for frame in frames {
            device.destroy_semaphore(frame.image_available, None);
            device.destroy_semaphore(frame.render_finished, None);
            device.destroy_fence(frame.fence, None);
            device.free_command_buffers(command_pool, &[frame.command_buffer]);
        }
    }
}

pub(super) fn create_frame_syncs(
    device: &Device,
    command_pool: vk::CommandPool,
    count: usize,
) -> RendererResult<Vec<FrameSync>> {
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        match create_frame_sync(device, command_pool) {
            Ok(frame) => frames.push(frame),
            Err(err) => {
                destroy_frame_syncs(device, command_pool, frames);
                return Err(err);
            }
        }
    }
    Ok(frames)
}
