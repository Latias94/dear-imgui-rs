#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]
//! Docking space functionality for Dear ImGui
//!
//! This module provides high-level Rust bindings for Dear ImGui's docking system,
//! allowing you to create dockable windows and manage dock spaces.
//!
//! # Notes
//!
//! Docking support is always compiled into this crate; no Cargo feature is required. Set
//! [`ConfigFlags::DOCKING_ENABLE`](crate::ConfigFlags::DOCKING_ENABLE) before the first frame when
//! the Context will use docking. The setting is intentionally stable for that Context's lifetime
//! because Dear ImGui destroys live dock nodes when it is disabled at runtime.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Create a dockspace over the main viewport
//! let dockspace_id = ui.dockspace_over_main_viewport();
//!
//! // Dock a window to the dockspace
//! ui.set_next_window_dock_id(dockspace_id);
//! ui.window("Tool Window").build(|| {
//!     ui.text("This window is docked!");
//! });
//! ```

mod flags;
mod ui;
mod validation;
mod window_class;

pub use flags::{DockFlags, DockNodeFlags};
pub(crate) use validation::{
    claim_dockspace_submission, main_viewport_dockspace_host_name, window_skips_items,
};
pub use window_class::{WindowClass, WindowClassParentViewport};
