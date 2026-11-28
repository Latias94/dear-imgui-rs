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

use crate::ContextId;
use crate::render::SnapshotTextureId;
use crate::texture::{ManagedTextureId, TextureRegion, TextureStatus};

pub mod context;
mod font;

pub use context::ContextError;
pub use font::FontError;

/// Result type for Dear ImGui operations
pub type ImGuiResult<T> = Result<T, ImGuiError>;

/// Errors that can occur in Dear ImGui operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ImGuiError {
    /// A frame or renderer-consumer contract failed.
    #[error("frame operation failed: {0}")]
    Frame(#[from] FrameError),

    /// A texture operation failed.
    #[error("texture operation failed: {0}")]
    Texture(#[from] TextureError),

    /// A Context lifecycle, configuration, or platform operation failed.
    #[error("{0}")]
    Context(#[from] ContextError),

    /// Atlas configuration or font loading failed.
    #[error("{0}")]
    Font(#[from] FontError),

    /// Invalid operation attempted
    #[error("Invalid operation: {operation}")]
    InvalidOperation { operation: String },

    /// Resource allocation failed
    #[error("Resource allocation failed: {resource}")]
    ResourceAllocation { resource: String },

    /// Renderer error (from backends)
    #[error("Renderer error: {0}")]
    Renderer(#[from] Box<dyn std::error::Error + Send + Sync>),

    /// Generic error with custom message
    #[error("{message}")]
    Generic { message: String },
}

impl ImGuiError {
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

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        let message = message.into();
        Self::Generic { message }
    }
}

/// Failure while capturing a frame, reconciling its work, or publishing renderer state.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum FrameError {
    #[error("this frame already rendered for synchronous capture")]
    FrameAlreadyRendered,
    #[error("user callback commands are not supported by detached snapshots")]
    UserCallbackUnsupported,
    #[error("draw data contains a managed texture not owned by this Context or its font atlas")]
    UnknownManagedTexture,
    #[error("managed texture {id:?} has status {status:?} but no pixel buffer is available")]
    TexturePixelsMissing {
        id: SnapshotTextureId,
        status: TextureStatus,
    },
    #[error(
        "managed texture {id:?} has invalid dimensions/format (width={width}, height={height}, bpp={bpp})"
    )]
    TextureInvalidLayout {
        id: SnapshotTextureId,
        width: i32,
        height: i32,
        bpp: i32,
    },
    #[error(
        "managed texture {id:?} full update exceeds ImTextureRect limits (width={width}, height={height})"
    )]
    TextureFullUpdateOutOfRange {
        id: SnapshotTextureId,
        width: u32,
        height: u32,
    },

    #[error("this Context already has an active renderer consumer")]
    ConsumerAlreadyActive,
    #[error(
        "managed font-atlas rendering requires exactly one registered Context; found {registered_contexts}"
    )]
    SharedFontAtlasRequiresExclusiveContext { registered_contexts: usize },
    #[error(
        "the font atlas is claimed by a legacy renderer or contains legacy-preloaded data; clear and repopulate it before attaching a managed renderer"
    )]
    FontAtlasRequiresManagedRebuild,
    #[error(
        "the shared font atlas still belongs to a renderer whose texture release was not committed"
    )]
    SharedFontAtlasRendererReleasePending,
    #[error("the previous renderer consumer is still draining outstanding epochs")]
    ConsumerDraining,
    /// The previous renderer's texture resources must be released before this generation draws.
    #[error(
        "release the previous renderer's texture resources and commit a renderer texture reset"
    )]
    RendererTextureResetRequired,
    #[error("this Context has no active renderer consumer")]
    NoActiveConsumer,
    #[error("{caller} requires a renderer that advertises RENDERER_HAS_TEXTURES")]
    RendererTexturesUnavailable { caller: &'static str },
    #[error("renderer consumer belongs to Context {actual:?}, not Context {expected:?}")]
    ForeignContext {
        expected: ContextId,
        actual: ContextId,
    },
    #[error("renderer consumer generation {actual} is stale; current generation is {expected}")]
    StaleConsumerGeneration { expected: u64, actual: u64 },
    #[error("renderer consumer generation space is exhausted")]
    ConsumerGenerationExhausted,
    #[error("snapshot epoch space is exhausted")]
    EpochExhausted,
    #[error("snapshot completion references unknown epoch {epoch}")]
    UnknownEpoch { epoch: u64 },
    #[error("snapshot epoch {epoch} was completed more than once")]
    EpochAlreadyCompleted { epoch: u64 },
    #[error("renderer consumer still owns {count} outstanding epoch(s)")]
    OutstandingEpochs { count: usize },
    #[error("snapshot epoch {epoch} contains duplicate feedback for {texture:?}")]
    DuplicateFeedback {
        epoch: u64,
        texture: SnapshotTextureId,
    },
    #[error("snapshot epoch {epoch} is missing {count} required feedback outcome(s)")]
    MissingFeedback { epoch: u64, count: usize },
    #[error("snapshot epoch {epoch} did not request feedback for {texture:?}")]
    FeedbackNotRequested {
        epoch: u64,
        texture: SnapshotTextureId,
    },
    #[error("feedback result does not match the request kind for {texture:?}")]
    InvalidFeedbackTransition { texture: SnapshotTextureId },
    #[error("font-atlas feedback targets a stale atlas allocation or generation")]
    StaleFontAtlas,

    /// The snapshot's owning Context stopped accepting completion.
    #[error("the snapshot's owning Context no longer accepts completion")]
    ContextDropped,

    /// Dear ImGui did not provide Platform IO for the active Context.
    #[error("the Context has no Platform IO")]
    MissingPlatformIo,
    /// A renderer already published transient callback state for this draw scope.
    #[error("a renderer already published state in Renderer_RenderState")]
    AlreadyOccupied,
    /// A raw callback replaced the state published by the renderer.
    #[error("a callback replaced the published Renderer_RenderState")]
    Drift,

    /// The frame could not use a Context-owned texture.
    #[error("frame texture contract failed: {0}")]
    Texture(#[from] TextureError),
}

/// Failure to create, mutate, retire or acknowledge a Dear ImGui texture.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum TextureError {
    /// Another OS thread owns Dear ImGui's native state.
    #[error("cannot allocate a texture while another OS thread owns Dear ImGui native state")]
    ContextThreadConflict,

    /// The thread lease count was exhausted or the native texture allocation failed.
    #[error("failed to allocate the owned Dear ImGui texture or its thread lease")]
    AllocationFailed,

    /// Width or height was zero.
    #[error("texture dimensions must be positive (got {width}x{height})")]
    InvalidDimensions {
        /// Requested texture width.
        width: u32,
        /// Requested texture height.
        height: u32,
    },

    /// Width does not fit Dear ImGui's signed native dimension field.
    #[error("texture width {0} exceeds Dear ImGui's signed dimension range")]
    WidthOutOfRange(u32),

    /// Height does not fit Dear ImGui's signed native dimension field.
    #[error("texture height {0} exceeds Dear ImGui's signed dimension range")]
    HeightOutOfRange(u32),

    /// The native allocation size overflowed or exceeded its signed size limit.
    #[error(
        "texture byte size is not representable: {width}x{height} pixels at {bytes_per_pixel} bytes per pixel"
    )]
    ByteSizeOutOfRange {
        /// Texture width.
        width: u32,
        /// Texture height.
        height: u32,
        /// Bytes per pixel.
        bytes_per_pixel: usize,
    },

    /// Native metadata is not a valid texture layout.
    #[error(
        "texture metadata is invalid: width={width}, height={height}, bytes_per_pixel={bytes_per_pixel}"
    )]
    InvalidLayout {
        /// Native width.
        width: i32,
        /// Native height.
        height: i32,
        /// Native bytes per pixel.
        bytes_per_pixel: i32,
    },

    /// The supplied payload does not exactly match the requested byte contract.
    #[error("texture payload length mismatch: expected {expected} bytes, got {actual}")]
    ByteLengthMismatch {
        /// Required payload length.
        expected: usize,
        /// Supplied payload length.
        actual: usize,
    },

    /// Pixel mutation is unavailable in the `WantDestroy` or `Destroyed` lifecycle state.
    #[error("texture cannot be mutated while its status is {0:?}")]
    InvalidStatus(TextureStatus),

    /// A live texture does not have CPU-side pixel storage.
    #[error("texture status {0:?} has no CPU pixel storage")]
    MissingPixelStorage(TextureStatus),

    /// The full texture cannot be represented by Dear ImGui's 16-bit update rectangle.
    #[error("full texture update {width}x{height} cannot be represented by TextureRect")]
    FullUpdateRectOutOfRange {
        /// Texture width.
        width: u32,
        /// Texture height.
        height: u32,
    },

    /// A subresource region has a zero width or height.
    #[error("texture update region dimensions must be positive (got {width}x{height})")]
    InvalidRegionDimensions {
        /// Requested region width.
        width: u32,
        /// Requested region height.
        height: u32,
    },

    /// A subresource rectangle exceeds the texture bounds.
    #[error("texture update region {region:?} exceeds texture bounds {width}x{height}")]
    UpdateRegionOutOfBounds {
        /// Requested region.
        region: TextureRegion,
        /// Texture width.
        width: u32,
        /// Texture height.
        height: u32,
    },

    /// A live update region cannot be represented by Dear ImGui's native update queue.
    #[error("texture update region cannot be represented by the native update queue: {0:?}")]
    UpdateRegionNotRepresentable(TextureRegion),

    /// The supplied row pitch cannot hold one tightly packed source row.
    #[error("texture row pitch {actual} is smaller than the required {minimum} bytes")]
    RowPitchTooSmall {
        /// Minimum tightly packed row size.
        minimum: usize,
        /// Supplied row pitch.
        actual: usize,
    },

    /// Computing the strided payload size overflowed.
    #[error(
        "texture subresource payload size overflowed for row pitch {row_pitch} and height {height}"
    )]
    PayloadSizeOutOfRange {
        /// Supplied row pitch.
        row_pitch: usize,
        /// Rectangle height.
        height: u32,
    },

    /// The handle belongs to another Context.
    #[error("managed texture belongs to Context {actual:?}, not Context {expected:?}")]
    ForeignContext {
        /// Context used for the attempted operation.
        expected: ContextId,
        /// Context encoded in the handle.
        actual: ContextId,
    },

    /// The handle does not identify a slot allocated by this Context.
    #[error("managed texture handle has an unknown slot: {0:?}")]
    UnknownSlot(ManagedTextureId),

    /// The slot has been reused for a newer texture generation.
    #[error("managed texture handle has a stale generation: {0:?}")]
    StaleGeneration(ManagedTextureId),

    /// Removal has started and new drawing or mutation is no longer accepted.
    #[error("managed texture is retiring: {0:?}")]
    Retiring(ManagedTextureId),

    /// Removal was already requested and is waiting for renderer retirement.
    #[error("managed texture removal was already requested: {0:?}")]
    AlreadyRetiring(ManagedTextureId),

    /// The texture was already fully removed.
    #[error("managed texture was already removed: {0:?}")]
    AlreadyRemoved(ManagedTextureId),

    /// An atlas-backed texture was used with a Context that does not own that atlas.
    #[error("font-atlas texture belongs to a different Context atlas")]
    ForeignFontAtlas,

    /// One feedback batch addressed the same texture more than once.
    #[error("managed texture feedback contains a duplicate handle: {0:?}")]
    DuplicateFeedback(ManagedTextureId),

    /// Renderer feedback may acknowledge only an uploaded or destroyed texture.
    #[error("managed texture feedback for {id:?} used invalid status {status:?}")]
    InvalidFeedbackStatus {
        /// Texture addressed by the feedback.
        id: ManagedTextureId,
        /// Status rejected before native mutation.
        status: TextureStatus,
    },

    #[error("an upload result cannot complete a destroy request")]
    UploadForDestroy,
    #[error("a destroy result cannot complete a create or update request")]
    DestroyForUpload,
    #[error("an upload result requires a non-null renderer texture identifier")]
    NullTextureId,
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
        let err = ImGuiError::from(ContextError::Creation {
            reason: "test reason".into(),
        });
        assert!(err.to_string().contains("test reason"));
    }

    #[test]
    fn renderer_error_displays_and_retains_its_source() {
        let source_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let imgui_err = ImGuiError::Renderer(Box::new(source_err));
        assert_eq!(imgui_err.to_string(), "Renderer error: file not found");
        assert_eq!(imgui_err.source().unwrap().to_string(), "file not found");
    }
}
