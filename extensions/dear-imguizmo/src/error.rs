//! Error handling for Dear ImGuizmo
//!
//! This module provides comprehensive error handling for the ImGuizmo library,
//! covering context creation, matrix operations, and runtime errors.

use thiserror::Error;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, trace, warn};

/// Result type for ImGuizmo operations
pub type GuizmoResult<T> = Result<T, GuizmoError>;

/// Errors that can occur in ImGuizmo operations
#[derive(Error, Debug)]
pub enum GuizmoError {
    /// Context creation failed
    #[error("Failed to create ImGuizmo context: {reason}")]
    ContextCreation {
        /// The reason for the context creation failure
        reason: String,
    },

    /// Context is not initialized
    #[error("ImGuizmo context is not properly initialized")]
    ContextNotInitialized,

    /// Invalid matrix data
    #[error("Invalid matrix data: {reason}")]
    InvalidMatrix {
        /// The reason why the matrix is invalid
        reason: String,
    },

    /// Invalid operation attempted
    #[error("Invalid operation: {operation}")]
    InvalidOperation {
        /// The invalid operation that was attempted
        operation: String,
    },

    /// Invalid transformation mode
    #[error("Invalid transformation mode: {mode}")]
    InvalidMode {
        /// The invalid mode that was specified
        mode: String,
    },

    /// Invalid viewport configuration
    #[error("Invalid viewport: {reason}")]
    InvalidViewport {
        /// The reason why the viewport is invalid
        reason: String,
    },

    /// Math operation error
    #[error("Math operation failed: {operation}")]
    MathOperation {
        /// The mathematical operation that failed
        operation: String,
    },

    /// Rendering error
    #[error("Rendering error: {reason}")]
    Rendering {
        /// The reason for the rendering error
        reason: String,
    },

    /// Input/interaction error
    #[error("Input error: {reason}")]
    Input {
        /// The reason for the input error
        reason: String,
    },

    /// Style configuration error
    #[error("Style configuration error: {setting}")]
    StyleConfiguration {
        /// The style setting that caused the error
        setting: String,
    },

    /// Extension module error
    #[error("Extension error in {module}: {reason}")]
    Extension {
        /// The extension module where the error occurred
        module: String,
        /// The reason for the extension error
        reason: String,
    },

    /// Generic error with custom message
    #[error("{message}")]
    Generic {
        /// The error message
        message: String,
    },
}

impl GuizmoError {
    /// Create a context creation error
    pub fn context_creation(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        error!("ImGuizmo context creation failed: {}", reason);
        Self::ContextCreation { reason }
    }

    /// Create a context not initialized error
    pub fn context_not_initialized() -> Self {
        #[cfg(feature = "tracing")]
        error!("ImGuizmo context not initialized");
        Self::ContextNotInitialized
    }

    /// Create an invalid matrix error
    pub fn invalid_matrix(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        warn!("Invalid matrix data: {}", reason);
        Self::InvalidMatrix { reason }
    }

    /// Create an invalid operation error
    pub fn invalid_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        #[cfg(feature = "tracing")]
        warn!("Invalid operation: {}", operation);
        Self::InvalidOperation { operation }
    }

    /// Create an invalid mode error
    pub fn invalid_mode(mode: impl Into<String>) -> Self {
        let mode = mode.into();
        #[cfg(feature = "tracing")]
        warn!("Invalid transformation mode: {}", mode);
        Self::InvalidMode { mode }
    }

    /// Create an invalid viewport error
    pub fn invalid_viewport(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        warn!("Invalid viewport: {}", reason);
        Self::InvalidViewport { reason }
    }

    /// Create a math operation error
    pub fn math_operation(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        #[cfg(feature = "tracing")]
        error!("Math operation failed: {}", operation);
        Self::MathOperation { operation }
    }

    /// Create a rendering error
    pub fn rendering(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        error!("Rendering error: {}", reason);
        Self::Rendering { reason }
    }

    /// Create an input error
    pub fn input(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        debug!("Input error: {}", reason);
        Self::Input { reason }
    }

    /// Create a style configuration error
    pub fn style_configuration(setting: impl Into<String>) -> Self {
        let setting = setting.into();
        #[cfg(feature = "tracing")]
        warn!("Style configuration error: {}", setting);
        Self::StyleConfiguration { setting }
    }

    /// Create an extension error
    pub fn extension(module: impl Into<String>, reason: impl Into<String>) -> Self {
        let module = module.into();
        let reason = reason.into();
        #[cfg(feature = "tracing")]
        error!("Extension error in {}: {}", module, reason);
        Self::Extension { module, reason }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        let message = message.into();
        #[cfg(feature = "tracing")]
        debug!("Generic error: {}", message);
        Self::Generic { message }
    }
}

/// Trait for safe string conversion
pub trait SafeStringConversion {
    /// Convert to CString safely, returning an error if the string contains null bytes
    fn to_cstring_safe(&self) -> Result<std::ffi::CString, GuizmoError>;
}

impl SafeStringConversion for str {
    fn to_cstring_safe(&self) -> Result<std::ffi::CString, GuizmoError> {
        std::ffi::CString::new(self).map_err(|_| GuizmoError::InvalidOperation {
            operation: format!("String contains null byte: {}", self),
        })
    }
}

impl SafeStringConversion for String {
    fn to_cstring_safe(&self) -> Result<std::ffi::CString, GuizmoError> {
        self.as_str().to_cstring_safe()
    }
}

/// Macro for conditional tracing
#[macro_export]
macro_rules! guizmo_trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::trace!($($arg)*);
    };
}

/// Macro for conditional debug logging
#[macro_export]
macro_rules! guizmo_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)*);
    };
}

/// Macro for conditional info logging
#[macro_export]
macro_rules! guizmo_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)*);
    };
}

/// Macro for conditional warning logging
#[macro_export]
macro_rules! guizmo_warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::warn!($($arg)*);
    };
}

/// Macro for conditional error logging
#[macro_export]
macro_rules! guizmo_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::error!($($arg)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_error_creation() {
        let err = GuizmoError::context_creation("test reason");
        assert!(err.to_string().contains("test reason"));
    }

    #[test]
    fn test_error_display() {
        let err = GuizmoError::invalid_matrix("singular matrix");
        assert!(err.to_string().contains("singular matrix"));
    }

    #[test]
    fn test_safe_string_conversion() {
        let valid_str = "valid string";
        assert!(valid_str.to_cstring_safe().is_ok());

        let invalid_str = "invalid\0string";
        assert!(invalid_str.to_cstring_safe().is_err());
    }

    #[test]
    fn test_logging_macros() {
        // Test that macros compile without tracing feature
        guizmo_trace!("test trace");
        guizmo_debug!("test debug");
        guizmo_info!("test info");
        guizmo_warn!("test warn");
        guizmo_error!("test error");
    }
}
