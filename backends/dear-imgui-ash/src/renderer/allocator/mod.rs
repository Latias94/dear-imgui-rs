mod default;

#[cfg(feature = "gpu-allocator")]
mod gpu;

#[cfg(feature = "vk-mem")]
mod vkmem;

#[cfg(any(feature = "gpu-allocator", feature = "vk-mem"))]
use crate::RendererError;
use crate::RendererResult;
use ash::{Device, vk};
#[cfg(any(feature = "gpu-allocator", feature = "vk-mem"))]
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum Memory {
    Default(default::Memory),
    #[cfg(feature = "gpu-allocator")]
    Gpu(gpu::Memory),
    #[cfg(feature = "vk-mem")]
    VkMem(vkmem::Memory),
}

pub enum Allocator {
    Default(Box<default::Allocator>),
    #[cfg(feature = "gpu-allocator")]
    Gpu(gpu::Allocator),
    #[cfg(feature = "vk-mem")]
    VkMem(vkmem::Allocator),
}

impl Allocator {
    pub fn new_default(properties: vk::PhysicalDeviceMemoryProperties) -> Self {
        Self::Default(Box::new(default::Allocator::new(properties)))
    }

    #[cfg(feature = "gpu-allocator")]
    pub fn new_gpu(allocator: Arc<Mutex<gpu_allocator::vulkan::Allocator>>) -> Self {
        Self::Gpu(gpu::Allocator::new(allocator))
    }

    #[cfg(feature = "vk-mem")]
    pub fn new_vk_mem(allocator: Arc<Mutex<vk_mem::Allocator>>) -> Self {
        Self::VkMem(vkmem::Allocator::new(allocator))
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
        match self {
            Self::Default(allocator) => allocator
                .create_buffer(device, size, usage)
                .map(|(buffer, memory)| (buffer, Memory::Default(memory))),
            #[cfg(feature = "gpu-allocator")]
            Self::Gpu(allocator) => allocator
                .create_buffer(device, size, usage)
                .map(|(buffer, memory)| (buffer, Memory::Gpu(memory))),
            #[cfg(feature = "vk-mem")]
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
            Self::Default(allocator) => allocator
                .create_image(device, width, height, format)
                .map(|(image, memory)| (image, Memory::Default(memory))),
            #[cfg(feature = "gpu-allocator")]
            Self::Gpu(allocator) => allocator
                .create_image(device, width, height, format)
                .map(|(image, memory)| (image, Memory::Gpu(memory))),
            #[cfg(feature = "vk-mem")]
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
            (Self::Default(allocator), Memory::Default(memory)) => {
                allocator.destroy_buffer(device, buffer, memory)
            }
            #[cfg(feature = "gpu-allocator")]
            (Self::Gpu(allocator), Memory::Gpu(memory)) => {
                allocator.destroy_buffer(device, buffer, memory)
            }
            #[cfg(feature = "vk-mem")]
            (Self::VkMem(allocator), Memory::VkMem(memory)) => {
                allocator.destroy_buffer(device, buffer, memory)
            }
            #[cfg(any(feature = "gpu-allocator", feature = "vk-mem"))]
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
            (Self::Default(allocator), Memory::Default(memory)) => {
                allocator.destroy_image(device, image, memory)
            }
            #[cfg(feature = "gpu-allocator")]
            (Self::Gpu(allocator), Memory::Gpu(memory)) => {
                allocator.destroy_image(device, image, memory)
            }
            #[cfg(feature = "vk-mem")]
            (Self::VkMem(allocator), Memory::VkMem(memory)) => {
                allocator.destroy_image(device, image, memory)
            }
            #[cfg(any(feature = "gpu-allocator", feature = "vk-mem"))]
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
            (Self::Default(allocator), Memory::Default(memory)) => {
                allocator.update_buffer(device, memory, data)
            }
            #[cfg(feature = "gpu-allocator")]
            (Self::Gpu(allocator), Memory::Gpu(memory)) => {
                allocator.update_buffer(device, memory, data)
            }
            #[cfg(feature = "vk-mem")]
            (Self::VkMem(allocator), Memory::VkMem(memory)) => {
                allocator.update_buffer(device, memory, data)
            }
            #[cfg(any(feature = "gpu-allocator", feature = "vk-mem"))]
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
