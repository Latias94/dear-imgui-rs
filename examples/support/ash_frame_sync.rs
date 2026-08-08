use ash::{Device, vk};
use dear_imgui_ash::TextureRetirementBatch;

pub struct FrameSync {
    pub image_available: vk::Semaphore,
    pub fence: vk::Fence,
    pub command_buffer: vk::CommandBuffer,
    pub texture_retirement: Option<TextureRetirementBatch>,
}

pub fn create_frame_sync(
    device: &Device,
    command_pool: vk::CommandPool,
) -> Result<FrameSync, vk::Result> {
    let image_available =
        unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
    let fence = match unsafe {
        device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )
    } {
        Ok(fence) => fence,
        Err(error) => {
            unsafe { device.destroy_semaphore(image_available, None) };
            return Err(error);
        }
    };
    let command_buffer = match unsafe {
        device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )
    } {
        Ok(command_buffers) => match command_buffers.first().copied() {
            Some(command_buffer) => command_buffer,
            None => {
                unsafe {
                    device.destroy_fence(fence, None);
                    device.destroy_semaphore(image_available, None);
                }
                return Err(vk::Result::ERROR_INITIALIZATION_FAILED);
            }
        },
        Err(error) => {
            unsafe {
                device.destroy_fence(fence, None);
                device.destroy_semaphore(image_available, None);
            }
            return Err(error);
        }
    };

    Ok(FrameSync {
        image_available,
        fence,
        command_buffer,
        texture_retirement: None,
    })
}

pub fn create_frame_syncs(
    device: &Device,
    command_pool: vk::CommandPool,
    count: usize,
) -> Result<Vec<FrameSync>, vk::Result> {
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        match create_frame_sync(device, command_pool) {
            Ok(frame) => frames.push(frame),
            Err(error) => {
                destroy_frame_syncs(device, command_pool, &mut frames);
                return Err(error);
            }
        }
    }
    Ok(frames)
}

pub fn replace_frame_sync(
    device: &Device,
    command_pool: vk::CommandPool,
    frame: &mut FrameSync,
) -> Result<Option<TextureRetirementBatch>, vk::Result> {
    let replacement = create_frame_sync(device, command_pool)?;
    let previous = std::mem::replace(frame, replacement);
    let retirement = previous.texture_retirement;
    destroy_frame_sync(device, command_pool, previous);
    Ok(retirement)
}

pub fn destroy_frame_syncs(
    device: &Device,
    command_pool: vk::CommandPool,
    frames: &mut Vec<FrameSync>,
) {
    for frame in frames.drain(..) {
        destroy_frame_sync(device, command_pool, frame);
    }
}

fn destroy_frame_sync(device: &Device, command_pool: vk::CommandPool, frame: FrameSync) {
    unsafe {
        device.destroy_semaphore(frame.image_available, None);
        device.destroy_fence(frame.fence, None);
        device.free_command_buffers(command_pool, &[frame.command_buffer]);
    }
}

pub fn create_present_semaphores(
    device: &Device,
    count: usize,
) -> Result<Vec<vk::Semaphore>, vk::Result> {
    let mut semaphores = Vec::with_capacity(count);
    for _ in 0..count {
        match unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) } {
            Ok(semaphore) => semaphores.push(semaphore),
            Err(error) => {
                destroy_present_semaphores(device, &mut semaphores);
                return Err(error);
            }
        }
    }
    Ok(semaphores)
}

pub fn destroy_present_semaphores(device: &Device, semaphores: &mut Vec<vk::Semaphore>) {
    unsafe {
        for semaphore in semaphores.drain(..) {
            device.destroy_semaphore(semaphore, None);
        }
    }
}

pub fn clear_fence_references(images_in_flight: &mut [vk::Fence], fence: vk::Fence) {
    for image_fence in images_in_flight {
        if *image_fence == fence {
            *image_fence = vk::Fence::null();
        }
    }
}

/// Records one color swapchain-image layout transition.
///
/// # Safety
///
/// `command_buffer` must be recording on `device`, `image` must be a live color image owned by
/// that device, and `old_layout` must describe the image's current tracked layout.
#[cfg(feature = "ash-dynamic-rendering")]
pub unsafe fn transition_swapchain_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let (src_stage, src_access) = match old_layout {
        vk::ImageLayout::UNDEFINED => (
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::empty(),
        ),
        vk::ImageLayout::PRESENT_SRC_KHR => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::empty(),
        ),
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        _ => (
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
        ),
    };
    let (dst_stage, dst_access) = if new_layout == vk::ImageLayout::PRESENT_SRC_KHR {
        (
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::empty(),
        )
    } else {
        (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )
    };
    let barrier = vk::ImageMemoryBarrier::default()
        .src_access_mask(src_access)
        .dst_access_mask(dst_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ash::vk::Handle;

    #[test]
    fn clear_fence_references_only_clears_the_abandoned_fence() {
        let abandoned = vk::Fence::from_raw(7);
        let retained = vk::Fence::from_raw(9);
        let mut fences = [abandoned, retained, abandoned, vk::Fence::null()];

        clear_fence_references(&mut fences, abandoned);

        assert_eq!(
            fences,
            [
                vk::Fence::null(),
                retained,
                vk::Fence::null(),
                vk::Fence::null(),
            ]
        );
    }
}
