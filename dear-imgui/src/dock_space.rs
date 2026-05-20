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
//! Docking is always enabled in this crate; no feature flag required.
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
pub use window_class::{WindowClass, WindowClassParentViewport};

pub(crate) use flags::validate_dock_node_flags;
pub(crate) use validation::{assert_finite_vec2, assert_nonzero_id, assert_positive_finite_vec2};
