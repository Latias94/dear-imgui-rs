//! Error types for the Vulkan (Ash) renderer.

use thiserror::Error;

/// Result type for renderer operations.
pub type RendererResult<T> = Result<T, RendererError>;

/// Errors that can occur during Vulkan renderer initialization or rendering.
#[derive(Debug, Error)]
pub enum RendererError {
    /// This backend is not supported on the current compilation target.
    #[error("dear-imgui-ash is not supported on this target")]
    UnsupportedTarget,

    /// Vulkan API error.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] ash::vk::Result),

    /// SPIR-V parsing error (when loading embedded shader bytecode).
    #[error("SPIR-V parsing error: {0}")]
    Spv(#[from] std::io::Error),

    /// Initialization error.
    #[error("Initialization error: {0}")]
    Init(String),

    /// Bad texture id (no matching descriptor set).
    #[error("Bad texture id: {0}")]
    BadTextureId(u64),

    /// Allocator error.
    #[error("Allocator error: {0}")]
    Allocator(String),

    /// GPU allocator error (when `gpu-allocator` feature is enabled).
    #[cfg(all(not(target_arch = "wasm32"), feature = "gpu-allocator"))]
    #[error("gpu-allocator error: {0}")]
    GpuAllocator(#[from] gpu_allocator::AllocationError),
    // NOTE: vk-mem (VMA) APIs return `ash::vk::Result` on failure, which is already covered by
    // the `Vulkan` variant above. We intentionally don't carry a separate vk-mem error variant
    // to avoid duplicate `From<ash::vk::Result>` implementations.
}
