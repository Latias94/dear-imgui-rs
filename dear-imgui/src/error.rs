//! Error types for Dear ImGui
//!
//! This module provides comprehensive error handling for the Dear ImGui library,
//! covering context creation, resource allocation, and runtime errors.
//!
//! Backend errors are wrapped explicitly in [`ImGuiError::Renderer`]. The blanket
//! `IntoImGuiError` conversion trait is intentionally unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::IntoImGuiError;
//! ```

use thiserror::Error;

/// Result type for Dear ImGui operations
pub type ImGuiResult<T> = Result<T, ImGuiError>;

/// Errors that can occur in Dear ImGui operations
#[derive(Error, Debug)]
pub enum ImGuiError {
    /// Context creation failed
    #[error("Failed to create Dear ImGui context: {reason}")]
    ContextCreation { reason: String },

    /// Context is already active
    #[error("A Dear ImGui context is already active")]
    ContextAlreadyActive,

    /// Invalid operation attempted
    #[error("Invalid operation: {operation}")]
    InvalidOperation { operation: String },

    /// Resource allocation failed
    #[error("Resource allocation failed: {resource}")]
    ResourceAllocation { resource: String },

    /// Font loading error
    #[error("Font loading failed: {reason}")]
    FontLoading { reason: String },

    /// Texture operation error
    #[error("Texture operation failed: {operation}")]
    TextureOperation { operation: String },

    /// Renderer error (from backends)
    #[error("Renderer error")]
    Renderer(#[from] Box<dyn std::error::Error + Send + Sync>),

    /// IO operation error
    #[error("IO operation failed: {operation}")]
    IoOperation { operation: String },

    /// Configuration error
    #[error("Configuration error: {setting}")]
    Configuration { setting: String },

    /// Generic error with custom message
    #[error("{message}")]
    Generic { message: String },
}

impl ImGuiError {
    /// Create a context creation error
    pub fn context_creation(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::ContextCreation { reason }
    }

    /// Create an invalid operation error
    pub fn invalid_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        Self::InvalidOperation { operation }
    }

    /// Create a resource allocation error
    pub fn resource_allocation(resource: impl Into<String>) -> Self {
        let resource = resource.into();
        Self::ResourceAllocation { resource }
    }

    /// Create a font loading error
    pub fn font_loading(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::FontLoading { reason }
    }

    /// Create a texture operation error
    pub fn texture_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        Self::TextureOperation { operation }
    }

    /// Create an IO operation error
    pub fn io_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        Self::IoOperation { operation }
    }

    /// Create a configuration error
    pub fn configuration(setting: impl Into<String>) -> Self {
        let setting = setting.into();
        Self::Configuration { setting }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        let message = message.into();
        Self::Generic { message }
    }
}

/// Helper trait for safe string conversion
pub trait SafeStringConversion {
    /// Convert to CString safely, returning an error if the string contains null bytes
    fn to_cstring_safe(&self) -> Result<std::ffi::CString, ImGuiError>;
}

impl SafeStringConversion for str {
    fn to_cstring_safe(&self) -> Result<std::ffi::CString, ImGuiError> {
        std::ffi::CString::new(self).map_err(|_| ImGuiError::InvalidOperation {
            operation: format!("String contains null byte: {}", self),
        })
    }
}

impl SafeStringConversion for String {
    fn to_cstring_safe(&self) -> Result<std::ffi::CString, ImGuiError> {
        self.as_str().to_cstring_safe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_error_creation() {
        let err = ImGuiError::context_creation("test reason");
        assert!(err.to_string().contains("test reason"));
    }

    #[test]
    fn test_error_chain() {
        let source_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let imgui_err = ImGuiError::Renderer(Box::new(source_err));
        assert!(imgui_err.source().is_some());
    }
}
