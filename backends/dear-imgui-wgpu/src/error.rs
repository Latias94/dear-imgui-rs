//! Error types for the WGPU renderer

use std::fmt;

/// Result type for renderer operations
pub type RendererResult<T> = Result<T, RendererError>;

/// Errors that can occur during rendering operations
#[derive(Debug)]
pub enum RendererError {
    /// Generic error with message
    Generic(String),
    /// Bad texture error
    BadTexture(String),
    /// Device lost error
    DeviceLost,
    /// Invalid render state
    InvalidRenderState(String),
    /// Buffer creation failed
    BufferCreationFailed(String),
    /// Texture creation failed
    TextureCreationFailed(String),
    /// Pipeline creation failed
    PipelineCreationFailed(String),
    /// Shader compilation failed
    ShaderCompilationFailed(String),
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RendererError::Generic(msg) => write!(f, "Renderer error: {}", msg),
            RendererError::BadTexture(msg) => write!(f, "Bad texture error: {}", msg),
            RendererError::DeviceLost => write!(f, "Device lost"),
            RendererError::InvalidRenderState(msg) => write!(f, "Invalid render state: {}", msg),
            RendererError::BufferCreationFailed(msg) => {
                write!(f, "Buffer creation failed: {}", msg)
            }
            RendererError::TextureCreationFailed(msg) => {
                write!(f, "Texture creation failed: {}", msg)
            }
            RendererError::PipelineCreationFailed(msg) => {
                write!(f, "Pipeline creation failed: {}", msg)
            }
            RendererError::ShaderCompilationFailed(msg) => {
                write!(f, "Shader compilation failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for RendererError {}
