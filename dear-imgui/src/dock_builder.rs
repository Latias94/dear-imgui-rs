//! DockBuilder API for programmatic dock layout creation
//!
//! This module provides the DockBuilder API which allows you to programmatically
//! create and manage dock layouts. This is useful for creating complex docking
//! layouts that would be difficult to achieve through manual docking.
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
//! // Create a dockspace
//! let dockspace_id = ui.get_id("MyDockspace");
//! DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
//!
//! // Split the dockspace: 30% left panel, 70% remaining
//! let (left_panel, main_area) = DockBuilder::split_node(
//!     dockspace_id,
//!     SplitDirection::Left,
//!     0.30
//! );
//!
//! // Further split the main area: 70% top, 30% bottom
//! let (top_area, bottom_area) = DockBuilder::split_node(
//!     main_area,
//!     SplitDirection::Down,
//!     0.30
//! );
//!
//! // Dock windows to specific nodes
//! DockBuilder::dock_window("Tool Panel", left_panel);
//! DockBuilder::dock_window("Main View", top_area);
//! DockBuilder::dock_window("Console", bottom_area);
//!
//! // Finish the layout
//! DockBuilder::finish(dockspace_id);
//! ```

mod direction;
mod node;
mod operations;
mod validation;

#[cfg(test)]
mod tests;

pub use direction::SplitDirection;
pub use node::{DockNode, NodeRect};
pub use operations::DockBuilder;
