//! Error types for Dear ImGui
//!
//! This module provides comprehensive error handling for the Dear ImGui library,
//! covering context creation, resource allocation, and runtime errors.

use thiserror::Error;

#[cfg(feature = "tracing")]
use tracing::{debug, error, warn};

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
        #[cfg(feature = "tracing")]
        error!("Context creation failed: {}", reason);
        Self::ContextCreation { reason }
    }

    /// Create an invalid operation error
    pub fn invalid_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        #[cfg(feature = "tracing")]
        warn!("Invalid operation: {}", operation);
        Self::InvalidOperation { operation }
    }

    /// Create a resource allocation error
    pub fn resource_allocation(resource: impl Into<String>) -> Self {
        let resource = resource.into();
        #[cfg(feature = "tracing")]
        error!("Resource allocation failed: {}", resource);
        Self::ResourceAllocation { resource }
    }

    /// Create a font loading error
    pub fn font_loading(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        error!("Font loading failed: {}", reason);
        Self::FontLoading { reason }
    }

    /// Create a texture operation error
    pub fn texture_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        #[cfg(feature = "tracing")]
        error!("Texture operation failed: {}", operation);
        Self::TextureOperation { operation }
    }

    /// Create an IO operation error
    pub fn io_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        #[cfg(feature = "tracing")]
        warn!("IO operation failed: {}", operation);
        Self::IoOperation { operation }
    }

    /// Create a configuration error
    pub fn configuration(setting: impl Into<String>) -> Self {
        let setting = setting.into();
        #[cfg(feature = "tracing")]
        warn!("Configuration error: {}", setting);
        Self::Configuration { setting }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        let message = message.into();
        #[cfg(feature = "tracing")]
        debug!("Generic error: {}", message);
        Self::Generic { message }
    }
}

/// Trait for converting backend errors to ImGuiError
pub trait IntoImGuiError {
    fn into_imgui_error(self) -> ImGuiError;
}

impl<E> IntoImGuiError for E
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_imgui_error(self) -> ImGuiError {
        ImGuiError::Renderer(Box::new(self))
    }
}

/// Trait for backend renderers with unified error handling
pub trait ImGuiRenderer {
    /// Backend-specific error type
    type Error: std::error::Error + Send + Sync + 'static;

    /// Initialize the renderer
    fn init(&mut self) -> Result<(), Self::Error>;

    /// Render the current frame
    fn render(&mut self, draw_data: &crate::render::DrawData) -> Result<(), Self::Error>;

    /// Handle device lost/reset scenarios
    fn device_lost(&mut self) -> Result<(), Self::Error> {
        // Default implementation does nothing
        Ok(())
    }

    /// Clean up resources
    fn shutdown(&mut self) -> Result<(), Self::Error> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Trait for platform backends with unified error handling
pub trait ImGuiPlatform {
    /// Platform-specific error type
    type Error: std::error::Error + Send + Sync + 'static;

    /// Initialize the platform backend
    fn init(&mut self) -> Result<(), Self::Error>;

    /// Handle platform events
    fn handle_event(&mut self, event: &dyn std::any::Any) -> Result<bool, Self::Error>;

    /// Update platform state for new frame
    fn new_frame(&mut self) -> Result<(), Self::Error>;

    /// Clean up platform resources
    fn shutdown(&mut self) -> Result<(), Self::Error> {
        // Default implementation does nothing
        Ok(())
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
        let imgui_err = source_err.into_imgui_error();
        assert!(imgui_err.source().is_some());
    }
}
