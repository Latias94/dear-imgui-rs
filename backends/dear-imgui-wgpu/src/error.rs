//! Error types for the WGPU renderer

use thiserror::Error;

/// Result type for renderer operations
pub type RendererResult<T> = Result<T, RendererError>;

/// Errors that can occur during rendering operations
#[derive(Error, Debug)]
pub enum RendererError {
    /// Generic error with message
    #[error("Renderer error: {0}")]
    Generic(String),

    /// Bad texture error
    #[error("Bad texture error: {0}")]
    BadTexture(String),

    /// Device lost error
    #[error("Device lost")]
    DeviceLost,

    /// Invalid render state
    #[error("Invalid render state: {0}")]
    InvalidRenderState(String),

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
}

// Display and Error traits are automatically implemented by thiserror
