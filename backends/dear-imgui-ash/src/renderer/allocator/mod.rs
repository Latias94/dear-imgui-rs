#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
mod default;
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
pub use self::default::{Allocator, Memory};

#[cfg(feature = "gpu-allocator")]
mod gpu;
#[cfg(feature = "gpu-allocator")]
pub use self::gpu::{Allocator, Memory};

#[cfg(feature = "vk-mem")]
mod vkmem;
#[cfg(feature = "vk-mem")]
pub use self::vkmem::{Allocator, Memory};

use crate::RendererResult;
use ash::{Device, vk};

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
