//! Declarative docking layouts.
//!
//! A [`DockLayout`] describes the complete dock tree. Applying it through
//! [`Ui::dock_space_with_layout`](crate::Ui::dock_space_with_layout) or
//! [`Ui::dockspace_over_main_viewport_with_layout`](crate::Ui::dockspace_over_main_viewport_with_layout)
//! owns the full docking-layout lifecycle, including validation, splitting, window assignment, and
//! finalization.
//!
//! Submit a layout before any window named by the layout or already hosted by its dock tree. This
//! ordering is required by Dear ImGui and is enforced before a safe replacement can mutate native
//! nodes. Validation and recoverable staging failures keep an existing root alive without changing
//! its flags, window class, topology, or persisted settings.
//!
//! Advanced integrations that need Dear ImGui's unstable imperative docking API can use the
//! raw `dear_imgui_rs::sys::igDockBuilder*` functions explicitly.
//!
//! The provisional compatibility names are intentionally absent:
//!
//! ```compile_fail
//! use dear_imgui_rs::{DockFlags, DockspaceTarget};
//! ```

mod compile;
mod model;

#[cfg(test)]
mod tests;

pub use model::{DockLayout, DockLayoutApply, DockLayoutError, DockSplit, DockspaceOptions};

pub(crate) use compile::{DockspaceSubmission, submit_and_apply};
