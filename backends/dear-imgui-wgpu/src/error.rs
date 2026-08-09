//! Error types for the WGPU renderer

use thiserror::Error;

/// Result type for renderer operations
pub type RendererResult<T> = Result<T, RendererError>;

/// Errors that can occur during rendering operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RendererError {
    /// Generic error with message
    #[error("Renderer error: {0}")]
    Generic(String),

    /// Bad texture error
    #[error("Bad texture error: {0}")]
    BadTexture(String),

    /// Invalid render state
    #[error("Invalid render state: {0}")]
    InvalidRenderState(String),

    /// Secondary viewport rendering began before the Context frame was reconciled.
    #[error(
        "WGPU secondary viewport rendering requires a reconciled frame before platform callbacks"
    )]
    FrameNotPrepared,

    /// A frame older than the renderer's active resource epoch was supplied.
    #[error("WGPU frame epoch {received} is older than active epoch {active}")]
    FrameEpochOutOfOrder { active: u64, received: u64 },

    /// The renderer has not been attached to a Dear ImGui context.
    #[error("WGPU renderer is not bound to a Dear ImGui context")]
    ContextNotBound,

    /// The Dear ImGui context used to initialize the renderer has been dropped.
    #[error("the Dear ImGui context bound to this WGPU renderer has been dropped")]
    ContextDropped,

    /// A context-taking operation received a context other than the renderer owner.
    #[error("WGPU renderer received a different Dear ImGui context than its bound context")]
    ContextMismatch,

    /// The context already has renderer-owned state that this renderer cannot claim safely.
    #[error("Dear ImGui context is already configured for a renderer backend")]
    ContextAlreadyHasRenderer,

    /// Renderer-owned Dear ImGui state changed while this renderer remained attached.
    #[error("WGPU renderer lost ownership of Dear ImGui field `{field}`")]
    RendererStateDrift { field: &'static str },

    /// Draw buffer length exceeds renderer index ranges.
    #[error("{buffer} draw buffer length exceeds renderer limits")]
    DrawBufferTooLarge { buffer: &'static str },

    /// Draw buffer offset overflowed while accumulating draw lists.
    #[error("{buffer} draw buffer offset overflow")]
    DrawBufferOffsetOverflow { buffer: &'static str },

    /// A draw command addresses indices outside its parent draw list.
    #[error("draw command index range {start}..{end} exceeds its parent index buffer length {len}")]
    DrawCommandIndexRangeOutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },

    /// A draw command references a vertex outside its parent draw list.
    #[error(
        "draw command references vertex {index}, but its parent vertex buffer has length {len}"
    )]
    DrawCommandVertexOutOfBounds { index: usize, len: usize },

    /// Native raw draw callbacks cannot be invoked by this target.
    #[error("raw draw callbacks are not supported by the WGPU renderer on this target")]
    RawDrawCallbackUnsupported,

    /// Buffer creation failed
    #[error("Buffer creation failed: {0}")]
    BufferCreationFailed(String),

    /// Texture creation failed
    #[error("Texture creation failed: {0}")]
    TextureCreationFailed(String),

    /// Pipeline creation failed
    #[error("Pipeline creation failed: {0}")]
    PipelineCreationFailed(String),

    /// Shader compilation failed
    #[error("Shader compilation failed: {0}")]
    ShaderCompilationFailed(String),

    /// WGPU error
    #[error("WGPU error")]
    Wgpu(#[from] wgpu::Error),

    /// Invalid texture ID
    #[error("Invalid texture ID: {0:?}")]
    InvalidTextureId(dear_imgui_rs::TextureId),

    /// The process-wide renderer texture identifier space is exhausted.
    #[error("WGPU texture identifier space is exhausted")]
    TextureIdExhausted,

    /// An external texture handle does not belong to this renderer or was already removed.
    #[error("external WGPU texture is not registered: {0:?}")]
    ExternalTextureNotFound(dear_imgui_rs::TextureId),

    /// A managed update arrived after the renderer lost its matching GPU resource.
    #[error(
        "managed texture {0:?} has no WGPU resource; reset renderer bindings to request a full create"
    )]
    ManagedTextureMissing(dear_imgui_rs::render::SnapshotTextureId),

    /// A managed identity was reused with incompatible dimensions.
    #[error(
        "managed texture {texture:?} layout changed without a new identity: expected {expected:?}, got {actual:?}"
    )]
    ManagedTextureLayoutMismatch {
        texture: dear_imgui_rs::render::SnapshotTextureId,
        expected: [u32; 2],
        actual: [u32; 2],
    },

    /// Context-owned renderer consumer state rejected the operation.
    #[error(transparent)]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),

    /// Managed frame finalization or texture-request capture failed.
    #[error(transparent)]
    FrameCapture(#[from] dear_imgui_rs::render::SnapshotError),

    /// Context attachment registration rejected the renderer's deferred-drop cleanup hook.
    #[error(transparent)]
    ContextAttachment(#[from] dear_imgui_rs::ContextAttachmentError),

    /// A request was completed with the wrong feedback kind.
    #[error(transparent)]
    TextureFeedback(#[from] dear_imgui_rs::render::TextureFeedbackError),
}

// Display and Error traits are automatically implemented by thiserror
