//! Error types for the Dear ImGui Glow renderer

use dear_imgui_rs::TextureFormat;
use thiserror::Error;

/// Errors that can occur during renderer initialization
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum InitError {
    /// Failed to create OpenGL buffer object
    #[error("Failed to create buffer object: {0}")]
    CreateBufferObject(String),

    /// Failed to create OpenGL texture
    #[error("Failed to create texture: {0}")]
    CreateTexture(String),

    /// Failed to create OpenGL shader
    #[error("Failed to create shader: {0}")]
    CreateShader(String),

    /// Failed to compile shader
    #[error("Failed to compile shader: {0}")]
    CompileShader(String),

    /// Failed to link shader program
    #[error("Failed to link program: {0}")]
    LinkProgram(String),

    /// Failed to create vertex array object
    #[error("Failed to create vertex array: {0}")]
    CreateVertexArray(String),

    /// OpenGL version not supported
    #[error("Unsupported OpenGL version: {0}")]
    UnsupportedVersion(String),

    /// An owned OpenGL context is required for this operation.
    #[error("No OpenGL context available")]
    MissingGlContext,

    /// A compiled shader program is missing a required vertex attribute.
    #[error("Could not find shader attribute: {0}")]
    MissingShaderAttribute(&'static str),

    /// Texture dimensions overflowed while computing the required byte length.
    #[error("{format:?} texture size overflow")]
    TextureSizeOverflow { format: TextureFormat },

    /// Texture dimensions do not fit OpenGL's signed size parameters.
    #[error("Texture {dimension} is out of range for OpenGL: {value}")]
    TextureDimensionOutOfRange { dimension: &'static str, value: u32 },

    /// Texture byte length does not match the expected size for the format.
    #[error("{format:?} texture data size mismatch: expected {expected} bytes, got {actual}")]
    TextureDataSizeMismatch {
        format: TextureFormat,
        expected: usize,
        actual: usize,
    },

    /// TextureId zero/null is not valid for this operation.
    #[error("TextureId must be non-zero for OpenGL")]
    NullTextureId,

    /// TextureId allocation space was exhausted.
    #[error("TextureId allocation space is exhausted")]
    TextureIdExhausted,

    /// The texture map does not contain the requested texture ID.
    #[error("TextureId is not registered: {0:?}")]
    UnknownTextureId(dear_imgui_rs::TextureId),

    /// A texture upload row is shorter than the pixels required by its format.
    #[error("{format:?} texture row pitch is too small: expected at least {minimum}, got {actual}")]
    TextureRowPitchTooSmall {
        format: TextureFormat,
        minimum: usize,
        actual: usize,
    },

    /// A managed texture update rectangle lies outside its texture allocation.
    #[error(
        "texture update rectangle ({x}, {y}, {width}, {height}) exceeds texture size {texture_width}x{texture_height}"
    )]
    TextureUpdateOutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        texture_width: u32,
        texture_height: u32,
    },

    /// The Context could not register this renderer's sole consumer capability.
    #[error(transparent)]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),

    /// Generic initialization error
    #[error("Initialization error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Errors that can occur during rendering
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RenderError {
    /// OpenGL error
    #[error("OpenGL error: {0}")]
    OpenGLError(String),

    /// Invalid texture ID
    #[error("Invalid texture: {0}")]
    InvalidTexture(String),

    /// Renderer was destroyed
    #[error("Renderer was destroyed")]
    RendererDestroyed,

    /// An OpenGL context is required for this operation.
    #[error(
        "No OpenGL context available. Use the matching *_with_context method for externally managed contexts."
    )]
    MissingGlContext,

    /// Renderer device object initialization failed.
    #[error("Device object initialization failed: {0}")]
    DeviceObjectInit(#[source] InitError),

    /// Failed to create an OpenGL resource while rendering.
    #[error("Failed to create {resource}: {error}")]
    CreateResource {
        resource: &'static str,
        error: String,
    },

    /// This renderer is no longer attached to a Context consumer.
    #[error("Glow renderer is not attached to a Dear ImGui renderer consumer")]
    RendererNotAttached,

    /// A rendered frame belongs to a different Context.
    #[error("rendered frame belongs to Context {actual:?}, not Context {expected:?}")]
    ContextMismatch {
        expected: dear_imgui_rs::ContextId,
        actual: dear_imgui_rs::ContextId,
    },

    /// A frame was rendered without the managed-texture consumer epoch Glow requires.
    #[error("rendered frame does not carry a managed-texture renderer epoch")]
    MissingRendererEpoch,

    /// A rendered frame belongs to an obsolete or foreign consumer generation.
    #[error(
        "rendered frame consumer generation {actual} does not match renderer generation {expected}"
    )]
    ConsumerGenerationMismatch { expected: u64, actual: u64 },

    /// Context-owned renderer epoch validation failed.
    #[error(transparent)]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),

    /// Texture feedback was constructed for the wrong request kind.
    #[error(transparent)]
    TextureFeedback(#[from] dear_imgui_rs::render::TextureFeedbackError),

    /// Generic rendering error
    #[error("Rendering error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Result type for initialization operations
pub type InitResult<T> = Result<T, InitError>;

/// Result type for rendering operations
pub type RenderResult<T> = Result<T, RenderError>;
