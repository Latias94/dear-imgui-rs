use thiserror::Error;

use super::{ManagedTextureId, TextureStatus};
use crate::ContextId;

/// Failure to access or retire a Context-owned managed texture.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ManagedTextureError {
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
}
