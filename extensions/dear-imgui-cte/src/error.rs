use dear_imgui_rs::{ContextBindingError, ContextId};
use std::{ffi::NulError, str::Utf8Error};

/// Errors reported by the safe cimCTE layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CteError {
    /// A native owner could not be allocated.
    #[error("{object} creation returned a null pointer")]
    CreationFailed { object: &'static str },

    /// The owning Dear ImGui context can no longer be entered safely.
    #[error("{operation} could not enter the owning Dear ImGui context: {source}")]
    ContextBinding {
        operation: &'static str,
        #[source]
        source: ContextBindingError,
    },

    /// A [`dear_imgui_rs::Ui`] belongs to another Dear ImGui context.
    #[error("{operation} requires context {expected:?}, but the Ui belongs to context {actual:?}")]
    WrongContext {
        operation: &'static str,
        expected: ContextId,
        actual: ContextId,
    },

    /// A string passed to C contained an interior NUL byte.
    #[error("{operation} received a string containing an interior NUL byte")]
    InteriorNul {
        operation: &'static str,
        #[source]
        source: NulError,
    },

    /// A native string was not valid UTF-8.
    #[error("{operation} returned invalid UTF-8")]
    InvalidUtf8 {
        operation: &'static str,
        #[source]
        source: Utf8Error,
    },

    /// A native operation unexpectedly returned a null pointer.
    #[error("{operation} returned a null pointer")]
    NullResult { operation: &'static str },

    /// A line index was outside the current document.
    #[error("{operation} received line {line}, but the document has {line_count} lines")]
    LineOutOfBounds {
        operation: &'static str,
        line: usize,
        line_count: usize,
    },

    /// A glyph column was outside its document line.
    #[error(
        "{operation} received column {column} on line {line}, but the line has {column_count} glyphs"
    )]
    ColumnOutOfBounds {
        operation: &'static str,
        line: usize,
        column: usize,
        column_count: usize,
    },

    /// A cursor index was outside the current cursor set.
    #[error("{operation} received cursor {cursor}, but the editor has {cursor_count} cursors")]
    CursorOutOfBounds {
        operation: &'static str,
        cursor: usize,
        cursor_count: usize,
    },

    /// A selection had its end before its start.
    #[error("{operation} requires an ordered selection")]
    ReversedSelection { operation: &'static str },

    /// A numeric configuration value violated its safe precondition.
    #[error("{operation} requires {parameter} to be {requirement}")]
    InvalidValue {
        operation: &'static str,
        parameter: &'static str,
        requirement: &'static str,
    },

    /// A vector contained NaN or infinity.
    #[error("{operation} requires {parameter} to contain finite values")]
    NonFinite {
        operation: &'static str,
        parameter: &'static str,
    },

    /// The compatibility bridge rejected an operation.
    #[error("{operation} failed with native status {status}")]
    NativeStatus {
        operation: &'static str,
        status: u32,
    },

    /// Two upstream features would overwrite the same native callback slots.
    #[error("{operation} conflicts with active {active}")]
    CallbackConflict {
        operation: &'static str,
        active: &'static str,
    },
}

/// Result alias used by the safe cimCTE layer.
pub type CteResult<T> = Result<T, CteError>;

pub(crate) fn c_string(operation: &'static str, value: &str) -> CteResult<std::ffi::CString> {
    std::ffi::CString::new(value).map_err(|source| CteError::InteriorNul { operation, source })
}
