//! Dear ImNodes - Rust Bindings with Dear ImGui Compatibility
//!
//! High-level Rust bindings for ImNodes, the node editor for Dear ImGui.
//! Configure an editor with [`ImNodesExt::imnodes_editor`], draw its nodes inside
//! [`NodeEditorSetup::with_nodes`], and read the captured interactions from [`PostEditor`].

use dear_imnodes_sys as sys;

mod context;
mod style;
mod types;
mod ui_ext;

pub use context::*;
pub use style::*;
pub use types::*;
pub use ui_ext::*;
