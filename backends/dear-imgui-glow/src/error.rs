//! Error types for the Dear ImGui Glow renderer

use thiserror::Error;

/// Errors that can occur during renderer initialization
#[derive(Error, Debug)]
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

    /// Generic initialization error
    #[error("Initialization error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Errors that can occur during rendering
#[derive(Error, Debug)]
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

    /// Generic rendering error
    #[error("Rendering error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Result type for initialization operations
pub type InitResult<T> = Result<T, InitError>;

/// Result type for rendering operations
pub type RenderResult<T> = Result<T, RenderError>;
