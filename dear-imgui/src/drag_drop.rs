#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    // We intentionally keep explicit casts for FFI clarity; avoid auto-fix churn.
    clippy::unnecessary_cast
)]
//! Drag and Drop functionality for Dear ImGui
//!
//! This module provides a complete drag and drop system that allows users to transfer
//! data between UI elements. The system consists of drag sources and drop targets,
//! with type-safe payload management.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Create a drag source
//! ui.button("Drag me!");
//! if let Some(source) = ui.drag_drop_source_config("MY_DATA").begin() {
//!     ui.text("Dragging...");
//!     source.end();
//! }
//!
//! // Create a drop target
//! ui.button("Drop here!");
//! if let Some(target) = ui.drag_drop_target() {
//!     if target
//!         .accept_payload_empty("MY_DATA", DragDropTargetFlags::NONE)
//!         .is_some()
//!     {
//!         println!("Data dropped!");
//!     }
//!     target.pop();
//! }
//! ```

mod flags;
mod payload;
mod source;
mod target;
#[cfg(test)]
mod tests;
mod ui;
mod validation;

pub use flags::{DragDropPayloadCond, DragDropSourceFlags, DragDropTargetFlags};
pub use payload::{DragDropPayload, DragDropPayloadEmpty, DragDropPayloadPod, PayloadIsWrongType};
pub use source::{DragDropSource, DragDropSourceTooltip};
pub use target::DragDropTarget;
