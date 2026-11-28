use dear_imgui_rs::{TextureBinding, TextureRect};
use thiserror::Error;

use crate::callback::ScopedRenderStateError;

/// Common renderer failures. Backends can also wrap these in their API-specific errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BackendError {
    /// A renderer resource could not be initialized.
    #[error("could not initialize {resource}: {reason}")]
    ResourceInitialization {
        /// Name of the resource being initialized.
        resource: &'static str,
        /// Details of the initialization failure.
        reason: String,
    },
    /// Pixel upload failed for a managed or application-provided texture.
    #[error("could not upload texture {binding:?}: {reason}")]
    TextureUpload {
        /// Binding identifying the upload destination.
        binding: TextureBinding,
        /// Details of the upload failure.
        reason: String,
    },
    /// A texture upload rectangle is empty or lies outside its destination texture.
    #[error("texture upload rectangle {rect:?} is empty or outside texture size {texture_size:?}")]
    InvalidTextureRect {
        /// Rectangle from the upload request.
        rect: TextureRect,
        /// Destination `[width, height]` in pixels.
        texture_size: [u32; 2],
    },
    /// The supplied frame violates a renderer precondition.
    #[error("invalid frame: {0}")]
    FrameContract(String),
    /// Projecting a draw command produced a non-finite clip edge.
    #[error("draw command clip rectangle {0:?} projects to a non-finite framebuffer coordinate")]
    NonFiniteClipRect([f32; 4]),
    /// A callback or transient renderer state slot conflicts with backend ownership.
    #[error("callback slot `{slot}` is unavailable: {reason}")]
    CallbackConflict {
        /// Dear ImGui field associated with the conflict.
        slot: &'static str,
        /// Details of the conflict.
        reason: &'static str,
    },
    /// The rendering device or graphics context was lost.
    #[error("renderer device lost: {0}")]
    DeviceLost(String),
    /// A renderer operation conflicts with its lifecycle state.
    #[error("cannot {operation}: {reason}")]
    Lifecycle {
        /// Operation attempted by the caller.
        operation: &'static str,
        /// State that prevented the operation.
        reason: String,
    },
}

impl From<ScopedRenderStateError> for BackendError {
    fn from(error: ScopedRenderStateError) -> Self {
        let reason = match error {
            ScopedRenderStateError::MissingPlatformIo => "the Context has no Platform IO",
            ScopedRenderStateError::AlreadyOccupied => "a renderer already published state",
            ScopedRenderStateError::Drift => "a callback replaced the published state",
        };
        Self::CallbackConflict {
            slot: "Renderer_RenderState",
            reason,
        }
    }
}
