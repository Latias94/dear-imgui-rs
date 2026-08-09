//! Rendering system for Dear ImGui.
//!
//! Synchronous rendering moves from [`PendingFrame`] to [`ReconciledFrame`] while borrowing its
//! owning Context. Detached
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
//! The provisional frame and consumer names are also intentionally unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::render::RenderedFrame;
//! ```
//!
//! ```compile_fail
//! use dear_imgui_rs::render::RendererConsumer;
//! ```
//!
//! Pending frames expose request and capability metadata, but never native draw data:
//!
//! ```compile_fail
//! fn inspect(frame: &dear_imgui_rs::render::PendingFrame<'_>) {
//!     let _ = frame.draw_data();
//! }
//! ```
//!
//! ```compile_fail
//! fn draw(_: &dear_imgui_rs::render::DrawData) {}
//! fn draw_pending(frame: &dear_imgui_rs::render::PendingFrame<'_>) {
//!     draw(frame);
//! }
//! ```
//!
//! Reconciliation consumes the pending capability, so it cannot be reused:
//!
//! ```compile_fail
//! fn reconcile_twice(frame: dear_imgui_rs::render::PendingFrame<'_>) {
//!     let _ = frame.reconcile_texture_feedback(std::iter::empty());
//!     let _ = frame.epoch();
//! }
//! ```
//!
//! A reconciled frame cannot outlive the Context borrow that owns its draw data:
//!
//! ```compile_fail
//! fn escape(
//!     frame: dear_imgui_rs::render::ReconciledFrame<'_>,
//! ) -> dear_imgui_rs::render::ReconciledFrame<'static> {
//!     frame
//! }
//! ```
//!
//! Consumer kind is fixed at creation rather than claimed by first use:
//!
//! ```compile_fail
//! fn wrong(
//!     context: &mut dear_imgui_rs::Context,
//!     consumer: &dear_imgui_rs::render::DetachedRendererConsumer,
//! ) {
//!     let _ = context.render(consumer);
//! }
//! ```
//!
//! ```compile_fail
//! fn wrong(
//!     context: &mut dear_imgui_rs::Context,
//!     consumer: &dear_imgui_rs::render::SynchronousRendererConsumer,
//! ) {
//!     let _ = context.render_snapshot(consumer);
//! }
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
pub use frame::{PendingFrame, ReconciledFrame};
pub use snapshot::*;
