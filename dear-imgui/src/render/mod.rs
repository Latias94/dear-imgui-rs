//! Rendering system for Dear ImGui.
//!
//! Synchronous rendering borrows [`RenderedFrame`] from its owning Context. Detached
//! rendering uses [`FrameSnapshot`], which contains no native draw pointers and carries
//! exactly one renderer-completion ticket.
//!
//! The provisional pseudo-owned draw-data types are intentionally unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::render::OwnedDrawData;
//! ```
//!
//! ```compile_fail
//! use dear_imgui_rs::render::OwnedDrawList;
//! ```
//!
//! Renderer implementations are supplied by backend crates rather than a compatibility
//! module in the core crate:
//!
//! ```compile_fail
//! use dear_imgui_rs::render::renderer;
//! ```
//!
//! Live draw commands are linear and cannot be detached or cloned:
//!
//! ```compile_fail
//! fn require_clone<T: Clone>() {}
//! require_clone::<dear_imgui_rs::render::DrawCmd<'static>>();
//! ```

mod callback_state;
pub mod draw_data;
mod frame;
pub mod snapshot;

// Re-export commonly used types
#[doc(hidden)]
pub use callback_state::{RendererRenderStateGuard, RendererRenderStateGuardError};
pub use draw_data::*;
pub use frame::{ReconciledFrame, RenderedFrame};
pub use snapshot::*;
