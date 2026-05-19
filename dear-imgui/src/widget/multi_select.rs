//! Multi-select helpers (BeginMultiSelect/EndMultiSelect)
//!
//! This module provides a small, safe wrapper around Dear ImGui's multi-select
//! API introduced in 1.92 (`BeginMultiSelect` / `EndMultiSelect`), following
//! the "external storage" pattern described in the official docs:
//! https://github.com/ocornut/imgui/wiki/Multi-Select
//!
//! The main entry point is [`Ui::multi_select_indexed`], which:
//! - wraps `BeginMultiSelect()` / `EndMultiSelect()`
//! - wires `SetNextItemSelectionUserData()` for each item (index-based)
//! - applies selection requests to your storage using a simple trait.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

mod basic_selection;
mod options;
mod requests;
mod scope;
mod storage;
#[cfg(test)]
mod tests;
mod ui;

pub use basic_selection::{BasicSelection, BasicSelectionIter};
pub use options::{
    MultiSelectBoxSelect, MultiSelectClickPolicy, MultiSelectFlags, MultiSelectOptions,
    MultiSelectScopeKind,
};
pub use scope::{MultiSelectEnd, MultiSelectScope};
pub use storage::{KeySetSelection, MultiSelectIndexStorage};
