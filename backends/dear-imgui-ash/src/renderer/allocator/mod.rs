#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
mod default;
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
pub use self::default::{Allocator, Memory};

#[cfg(feature = "gpu-allocator")]
mod gpu;
#[cfg(all(feature = "gpu-allocator", not(feature = "vk-mem")))]
pub use self::gpu::{Allocator, Memory};

#[cfg(feature = "vk-mem")]
mod vkmem;
#[cfg(all(feature = "vk-mem", not(feature = "gpu-allocator")))]
pub use self::vkmem::{Allocator, Memory};

#[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
use crate::RendererError;
use crate::RendererResult;
use ash::{Device, vk};
#[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
use std::sync::{Arc, Mutex};

#[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
#[derive(Debug)]
pub enum Memory {
    Gpu(gpu::Memory),
    VkMem(vkmem::Memory),
}

#[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
pub enum Allocator {
    Gpu(gpu::Allocator),
    VkMem(vkmem::Allocator),
}

#[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
impl Allocator {
    pub fn new_gpu(allocator: Arc<Mutex<gpu_allocator::vulkan::Allocator>>) -> Self {
        Self::Gpu(gpu::Allocator::new(allocator))
    }

    pub fn new_vk_mem(allocator: Arc<Mutex<vk_mem::Allocator>>) -> Self {
        Self::VkMem(vkmem::Allocator::new(allocator))
    }
}

#[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
impl Allocate for Allocator {
    type Memory = Memory;

    fn create_buffer(
        &mut self,
        device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Self::Memory)> {
        match self {
            Self::Gpu(allocator) => allocator
                .create_buffer(device, size, usage)
                .map(|(buffer, memory)| (buffer, Memory::Gpu(memory))),
            Self::VkMem(allocator) => allocator
                .create_buffer(device, size, usage)
                .map(|(buffer, memory)| (buffer, Memory::VkMem(memory))),
        }
    }

    fn create_image(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> RendererResult<(vk::Image, Self::Memory)> {
        match self {
            Self::Gpu(allocator) => allocator
                .create_image(device, width, height, format)
                .map(|(image, memory)| (image, Memory::Gpu(memory))),
            Self::VkMem(allocator) => allocator
                .create_image(device, width, height, format)
                .map(|(image, memory)| (image, Memory::VkMem(memory))),
        }
    }

    fn destroy_buffer(
        &mut self,
        device: &Device,
        buffer: vk::Buffer,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        match (self, memory) {
            (Self::Gpu(allocator), Memory::Gpu(memory)) => {
                allocator.destroy_buffer(device, buffer, memory)
            }
            (Self::VkMem(allocator), Memory::VkMem(memory)) => {
                allocator.destroy_buffer(device, buffer, memory)
            }
            _ => Err(RendererError::Allocator(
                "allocator and buffer memory backend mismatch".into(),
            )),
        }
    }

    fn destroy_image(
        &mut self,
        device: &Device,
        image: vk::Image,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        match (self, memory) {
            (Self::Gpu(allocator), Memory::Gpu(memory)) => {
                allocator.destroy_image(device, image, memory)
            }
            (Self::VkMem(allocator), Memory::VkMem(memory)) => {
                allocator.destroy_image(device, image, memory)
            }
            _ => Err(RendererError::Allocator(
                "allocator and image memory backend mismatch".into(),
            )),
        }
    }

    fn update_buffer<T: Copy>(
        &mut self,
        device: &Device,
        memory: &mut Self::Memory,
        data: &[T],
    ) -> RendererResult<()> {
        match (self, memory) {
            (Self::Gpu(allocator), Memory::Gpu(memory)) => {
                allocator.update_buffer(device, memory, data)
            }
            (Self::VkMem(allocator), Memory::VkMem(memory)) => {
                allocator.update_buffer(device, memory, data)
            }
            _ => Err(RendererError::Allocator(
                "allocator and buffer memory backend mismatch".into(),
            )),
        }
    }
}

/// Base allocator trait for all implementations.
pub trait Allocate {
    type Memory;

    fn create_buffer(
        &mut self,
        device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Self::Memory)>;

    fn create_image(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> RendererResult<(vk::Image, Self::Memory)>;

    fn destroy_buffer(
        &mut self,
        device: &Device,
        buffer: vk::Buffer,
        memory: Self::Memory,
    ) -> RendererResult<()>;

    fn destroy_image(
        &mut self,
        device: &Device,
        image: vk::Image,
        memory: Self::Memory,
    ) -> RendererResult<()>;

    fn update_buffer<T: Copy>(
        &mut self,
        device: &Device,
        memory: &mut Self::Memory,
        data: &[T],
    ) -> RendererResult<()>;
}
