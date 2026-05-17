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

    /// Texture byte length does not match the expected size for the format.
    #[error("{format:?} texture data size mismatch: expected {expected} bytes, got {actual}")]
    TextureDataSizeMismatch {
        format: TextureFormat,
        expected: usize,
        actual: usize,
    },

    /// TextureId cannot be represented as an OpenGL texture name.
    #[error("TextureId is out of range for OpenGL: {0}")]
    TextureIdOutOfRange(u64),

    /// TextureId zero/null is not valid for this operation.
    #[error("TextureId must be non-zero for OpenGL")]
    NullTextureId,

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

    /// Generic rendering error
    #[error("Rendering error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Result type for initialization operations
pub type InitResult<T> = Result<T, InitError>;

/// Result type for rendering operations
pub type RenderResult<T> = Result<T, RenderError>;
