use thiserror::Error;

use super::{ManagedTextureId, TextureRegion, TextureStatus};
use crate::ContextId;

/// Validation failure while creating or mutating CPU-side texture data.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum TextureDataError {
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
}

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

/// Failure to access or mutate a Context-owned texture.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ManagedTextureMutationError {
    /// The managed handle could not be accessed.
    #[error(transparent)]
    Access(#[from] ManagedTextureError),

    /// The requested pixel mutation was invalid.
    #[error(transparent)]
    Data(#[from] TextureDataError),
}
