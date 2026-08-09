//! Declarative docking layouts.
//!
//! A [`DockLayout`] describes the complete dock tree. Applying it through
//! [`Ui::dockspace`](crate::Ui::dockspace) owns the full docking-layout lifecycle, including
//! validation, splitting, window assignment, and finalization.
//!
//! Submit a layout before any window named by the layout or already hosted by its dock tree. This
//! ordering is required by Dear ImGui and is enforced before a safe replacement can mutate native
//! nodes. Validation and recoverable staging failures keep an existing root alive without changing
//! its flags, window class, topology, or persisted settings.
//!
//! Advanced integrations that need Dear ImGui's unstable imperative docking API can use the
//! raw `dear_imgui_rs::sys::igDockBuilder*` functions explicitly.
//!
//! The provisional compatibility types are intentionally absent:
//!
//! ```compile_fail
//! use dear_imgui_rs::{DockLayoutError, DockspaceOptions};
//! ```

mod compile;
mod model;

#[cfg(test)]
mod tests;

pub use model::{DockLayout, DockLayoutApply, DockSplit, DockspaceError};

pub(crate) use compile::{DockspaceHost, submit_and_apply, submit_without_layout};
pub(crate) use model::DockspaceConfig;
