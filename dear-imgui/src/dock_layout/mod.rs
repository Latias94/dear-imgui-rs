//! Declarative docking layouts.
//!
//! A [`DockLayout`] describes the complete dock tree. Applying it through
//! [`Ui::dock_space_with_layout`](crate::Ui::dock_space_with_layout) or
//! [`Ui::dockspace_over_main_viewport_with_layout`](crate::Ui::dockspace_over_main_viewport_with_layout)
//! owns the full docking-layout lifecycle, including validation, splitting, window assignment, and
//! finalization.
//!
//! Advanced integrations that need Dear ImGui's unstable imperative docking API can use the
//! raw `dear_imgui_rs::sys::igDockBuilder*` functions explicitly.

mod compile;
mod model;

#[cfg(test)]
mod tests;

pub use model::{DockLayout, DockLayoutApply, DockLayoutError, DockSplit, DockspaceTarget};

pub(crate) use compile::{DockspaceSubmission, submit_and_apply};
