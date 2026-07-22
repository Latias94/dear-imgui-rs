//! Error types for the Vulkan (Ash) renderer.

use thiserror::Error;

/// Result type for renderer operations.
pub type RendererResult<T> = Result<T, RendererError>;

/// Errors that can occur during Vulkan renderer initialization or rendering.
#[derive(Debug, Error)]
#[non_exhaustive]
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

    /// Renderer options or state are invalid.
    #[error("Invalid render state: {0}")]
    InvalidRenderState(String),

    /// Context renderer state was already claimed before Ash initialization.
    #[error("Dear ImGui renderer state `{field}` is already occupied")]
    RendererStateOccupied { field: &'static str },

    /// Context renderer state changed after Ash published its ownership contract.
    #[error("Dear ImGui renderer state `{field}` was replaced while Ash was attached")]
    RendererStateReplaced { field: &'static str },

    /// Draw frame resources are unavailable.
    #[error("Frame resources are not initialized")]
    FrameResourcesUnavailable,

    /// The renderer has already released its Vulkan resources.
    #[error("Renderer has been shut down")]
    RendererDestroyed,

    /// The renderer is not attached to a Dear ImGui context.
    #[error("Renderer is not attached to a Dear ImGui context")]
    RendererNotAttached,

    /// A frame or lifecycle call used a different Dear ImGui context.
    #[error("Renderer context mismatch: expected {expected:?}, got {actual:?}")]
    ContextMismatch {
        expected: dear_imgui_rs::ContextId,
        actual: dear_imgui_rs::ContextId,
    },

    /// A rendered frame belongs to an obsolete renderer-consumer generation.
    #[error("Renderer consumer generation mismatch: expected {expected}, got {actual}")]
    ConsumerGenerationMismatch { expected: u64, actual: u64 },

    /// Fence-proven texture retirement requires at least one completion fence.
    #[error("Texture retirement completion requires at least one Vulkan fence")]
    TextureRetirementFencesEmpty,

    /// A completion fence was null.
    #[error("Texture retirement completion fence {index} is null")]
    TextureRetirementFenceNull { index: usize },

    /// A completion fence has not signaled yet.
    #[error("Texture retirement completion fence {index} has not signaled")]
    TextureRetirementFencePending { index: usize },

    /// Bad texture id (no matching descriptor set).
    #[error("Bad texture id: {0}")]
    BadTextureId(u64),

    /// Allocator error.
    #[error("Allocator error: {0}")]
    Allocator(String),

    /// Managed renderer-consumer contract error.
    #[error("Renderer consumer error: {0}")]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),

    /// Managed texture feedback did not match its request.
    #[error("Texture feedback error: {0}")]
    TextureFeedback(#[from] dear_imgui_rs::render::TextureFeedbackError),

    /// GPU allocator error (when `gpu-allocator` feature is enabled).
    #[cfg(all(not(target_arch = "wasm32"), feature = "gpu-allocator"))]
    #[error("gpu-allocator error: {0}")]
    GpuAllocator(#[from] gpu_allocator::AllocationError),
    // NOTE: vk-mem (VMA) APIs return `ash::vk::Result` on failure, which is already covered by
    // the `Vulkan` variant above. We intentionally don't carry a separate vk-mem error variant
    // to avoid duplicate `From<ash::vk::Result>` implementations.
}
