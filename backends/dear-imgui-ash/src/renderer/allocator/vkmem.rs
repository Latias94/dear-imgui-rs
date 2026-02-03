#![cfg(feature = "vk-mem")]

use super::Allocate;
use crate::{RendererError, RendererResult};
use ash::{Device, vk};
use std::sync::{Arc, Mutex, MutexGuard};
use vk_mem::{
    Allocation, AllocationCreateFlags, AllocationCreateInfo, Allocator as VkMemAllocator,
    MemoryUsage,
};

/// Abstraction over memory used by Vulkan resources.
pub type Memory = Allocation;

pub struct Allocator {
    pub allocator: Arc<Mutex<VkMemAllocator>>,
}

impl Allocator {
    pub fn new(allocator: Arc<Mutex<vk_mem::Allocator>>) -> Self {
        Self { allocator }
    }

    fn get_allocator(&self) -> RendererResult<MutexGuard<VkMemAllocator>> {
        self.allocator.lock().map_err(|e| {
            RendererError::Allocator(format!("Failed to acquire lock on allocator: {e}"))
        })
    }
}

impl Allocate for Allocator {
    type Memory = Memory;

    fn create_buffer(
        &mut self,
        _device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Self::Memory)> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size as _)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer_alloc_info = AllocationCreateInfo {
            usage: MemoryUsage::AutoPreferHost,
            flags: AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };

        let allocator = self.get_allocator()?;
        let (buffer, allocation) =
            unsafe { allocator.create_buffer(&buffer_info, &buffer_alloc_info)? };
        Ok((buffer, allocation))
    }

    fn create_image(
        &mut self,
        _device: &Device,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> RendererResult<(vk::Image, Self::Memory)> {
        let extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(extent)
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::empty());

        let image_alloc_info = AllocationCreateInfo {
            usage: MemoryUsage::AutoPreferDevice,
            ..Default::default()
        };

        let allocator = self.get_allocator()?;
        let (image, allocation) =
            unsafe { allocator.create_image(&image_info, &image_alloc_info)? };
        Ok((image, allocation))
    }

    fn destroy_buffer(
        &mut self,
        _device: &Device,
        buffer: vk::Buffer,
        mut memory: Self::Memory,
    ) -> RendererResult<()> {
        let allocator = self.get_allocator()?;
        unsafe { allocator.destroy_buffer(buffer, &mut memory) };
        Ok(())
    }

    fn destroy_image(
        &mut self,
        _device: &Device,
        image: vk::Image,
        mut memory: Self::Memory,
    ) -> RendererResult<()> {
        let allocator = self.get_allocator()?;
        unsafe { allocator.destroy_image(image, &mut memory) };
        Ok(())
    }

    fn update_buffer<T: Copy>(
        &mut self,
        _device: &Device,
        memory: &mut Self::Memory,
        data: &[T],
    ) -> RendererResult<()> {
        let size = std::mem::size_of_val(data) as _;

        let allocator = self.get_allocator()?;
        let data_ptr = unsafe { allocator.map_memory(memory)? as *mut std::ffi::c_void };
        let mut align =
            unsafe { ash::util::Align::new(data_ptr, std::mem::align_of::<T>() as _, size) };
        align.copy_from_slice(data);
        unsafe { allocator.unmap_memory(memory) };

        Ok(())
    }
}
