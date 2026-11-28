//! Shared rendering primitives for Dear ImGui backends.
//!
//! The helpers use the core crate's texture identities, draw commands, and completion
//! watermarks. Backends own GPU resources, submission ordering, and renderer feedback.

mod callback;
mod color_scissor;
mod error;
mod texture;

pub use callback::{
    DrawCallbackDispatcher, ScopedRenderState, ScopedRenderStateError, StandardDrawCallback,
};
pub use color_scissor::{
    SRGB_DECODE_THRESHOLD, SRGB_EXPONENT, SRGB_LINEAR_DIVISOR, SRGB_OFFSET, SRGB_SCALE,
    compute_scissor_rect, srgb_to_linear_channel, srgb_vertex_color_to_linear,
    try_compute_scissor_rect,
};
pub use error::BackendError;
pub use texture::{DirtyRectNormalizer, ManagedTextureTable};
