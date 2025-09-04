use thiserror::Error;

/// Errors that can occur when using Dear ImGui
#[derive(Debug, Error)]
pub enum ImGuiError {
    #[error("Context creation failed")]
    ContextCreationFailed,

    #[error("Context not initialized")]
    ContextNotInitialized,

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Backend error: {0}")]
    BackendError(String),

    #[error("Texture error: {0}")]
    TextureError(String),

    #[error("Font loading error: {0}")]
    FontError(String),
}

/// Result type alias for Dear ImGui operations
pub type Result<T> = std::result::Result<T, ImGuiError>;
