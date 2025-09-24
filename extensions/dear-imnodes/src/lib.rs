//! Dear ImNodes - Rust Bindings with Dear ImGui Compatibility
//!
//! High-level Rust bindings for ImNodes, the node editor for Dear ImGui.
//! This crate follows the same patterns as our `dear-implot` and `dear-imguizmo`
//! crates: Ui extensions, RAII tokens, and strongly-typed flags/enums.

use dear_imgui::Ui;
use dear_imnodes_sys as sys;

mod context;
mod style;
mod types;
mod ui_ext;

pub use context::*;
pub use style::*;
pub use types::*;
pub use ui_ext::*;
