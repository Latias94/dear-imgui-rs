//! Error types for the Dear ImGui Glow renderer

use std::{error::Error, fmt::Display};

/// Errors that can occur during renderer initialization
#[derive(Debug)]
pub enum InitError {
    /// Failed to create OpenGL buffer object
    CreateBufferObject(String),
    /// Failed to create OpenGL texture
    CreateTexture(String),
    /// Failed to create OpenGL shader
    CreateShader(String),
    /// Failed to compile shader
    CompileShader(String),
    /// Failed to link shader program
    LinkProgram(String),
    /// Failed to create vertex array object
    CreateVertexArray(String),
    /// OpenGL version not supported
    UnsupportedVersion(String),
    /// Generic initialization error
    Generic(String),
}

impl Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::CreateBufferObject(msg) => {
                write!(f, "Failed to create buffer object: {}", msg)
            }
            InitError::CreateTexture(msg) => write!(f, "Failed to create texture: {}", msg),
            InitError::CreateShader(msg) => write!(f, "Failed to create shader: {}", msg),
            InitError::CompileShader(msg) => write!(f, "Failed to compile shader: {}", msg),
            InitError::LinkProgram(msg) => write!(f, "Failed to link program: {}", msg),
            InitError::CreateVertexArray(msg) => {
                write!(f, "Failed to create vertex array: {}", msg)
            }
            InitError::UnsupportedVersion(msg) => write!(f, "Unsupported OpenGL version: {}", msg),
            InitError::Generic(msg) => write!(f, "Initialization error: {}", msg),
        }
    }
}

impl Error for InitError {}

/// Errors that can occur during rendering
#[derive(Debug)]
pub enum RenderError {
    /// OpenGL error
    OpenGLError(String),
    /// Invalid texture ID
    InvalidTexture(String),
    /// Renderer was destroyed
    RendererDestroyed,
    /// Generic rendering error
    Generic(String),
}

impl Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::OpenGLError(msg) => write!(f, "OpenGL error: {}", msg),
            RenderError::InvalidTexture(msg) => write!(f, "Invalid texture: {}", msg),
            RenderError::RendererDestroyed => write!(f, "Renderer was destroyed"),
            RenderError::Generic(msg) => write!(f, "Rendering error: {}", msg),
        }
    }
}

impl Error for RenderError {}

/// Result type for initialization operations
pub type InitResult<T> = Result<T, InitError>;

/// Result type for rendering operations
pub type RenderResult<T> = Result<T, RenderError>;
