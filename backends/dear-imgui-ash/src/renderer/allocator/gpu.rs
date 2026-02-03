#![cfg(feature = "gpu-allocator")]

use crate::{RendererError, RendererResult};
use ash::{Device, vk};
use gpu_allocator::{
    MemoryLocation,
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator as GpuAllocator},
};
use std::sync::{Arc, Mutex, MutexGuard};

use super::Allocate;

/// Abstraction over memory used by Vulkan resources.
pub type Memory = Allocation;

pub struct Allocator {
    pub allocator: Arc<Mutex<GpuAllocator>>,
}

impl Allocator {
    pub fn new(allocator: Arc<Mutex<gpu_allocator::vulkan::Allocator>>) -> Self {
        Self { allocator }
    }

    fn get_allocator(&self) -> RendererResult<MutexGuard<GpuAllocator>> {
        self.allocator.lock().map_err(|e| {
            RendererError::Allocator(format!("Failed to acquire lock on allocator: {e}"))
        })
    }
}

impl Allocate for Allocator {
    type Memory = Memory;

    fn create_buffer(
        &mut self,
        device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Self::Memory)> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size as _)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let mut allocator = self.get_allocator()?;
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe { device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())? };
        Ok((buffer, allocation))
    }

    fn create_image(
        &mut self,
        device: &Device,
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

        let image = unsafe { device.create_image(&image_info, None)? };
        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let mut allocator = self.get_allocator()?;
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "",
            requirements,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe { device.bind_image_memory(image, allocation.memory(), allocation.offset())? };
        Ok((image, allocation))
    }

    fn destroy_buffer(
        &mut self,
        device: &Device,
        buffer: vk::Buffer,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        let mut allocator = self.get_allocator()?;
        allocator.free(memory)?;
        unsafe { device.destroy_buffer(buffer, None) };
        Ok(())
    }

    fn destroy_image(
        &mut self,
        device: &Device,
        image: vk::Image,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        let mut allocator = self.get_allocator()?;
        allocator.free(memory)?;
        unsafe { device.destroy_image(image, None) };
        Ok(())
    }

    fn update_buffer<T: Copy>(
        &mut self,
        _device: &Device,
        memory: &mut Self::Memory,
        data: &[T],
    ) -> RendererResult<()> {
        let size = std::mem::size_of_val(data) as _;
        unsafe {
            let data_ptr = memory.mapped_ptr().ok_or_else(|| {
                RendererError::Allocator("Failed to get mapped memory pointer".into())
            })?;
            let mut align =
                ash::util::Align::new(data_ptr.as_ptr(), std::mem::align_of::<T>() as _, size);
            align.copy_from_slice(data);
        }
        Ok(())
    }
}
