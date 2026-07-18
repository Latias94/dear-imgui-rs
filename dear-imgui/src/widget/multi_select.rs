//! Multi-select helpers (BeginMultiSelect/EndMultiSelect)
//!
//! This module provides a small, safe wrapper around Dear ImGui's multi-select
//! API introduced in 1.92 (`BeginMultiSelect` / `EndMultiSelect`), following
//! the "external storage" pattern described in the official docs:
//! <https://github.com/ocornut/imgui/wiki/Multi-Select>
//!
//! The main entry point is [`crate::Ui::multi_select_indexed`], which:
//! - wraps `BeginMultiSelect()` / `EndMultiSelect()`
//! - wires `SetNextItemSelectionUserData()` for each item (index-based)
//! - applies selection requests to your storage using a simple trait.
//!
//! Native begin/end IO is deliberately not exposed by the safe API:
//!
//! ```compile_fail
//! use dear_imgui_rs::MultiSelectEnd;
//! ```
//!
//! ```compile_fail
//! use dear_imgui_rs::MultiSelectScope;
//!
//! fn mutate_native_io(scope: &mut MultiSelectScope<'_>) {
//!     scope.begin_io_mut().RangeSrcReset = true;
//! }
//! ```

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
pub use requests::{
    MultiSelectRangeDirection, MultiSelectRequest, MultiSelectResult, MultiSelectUserData,
};
pub use scope::MultiSelectScope;
pub use storage::{KeySetSelection, MultiSelectIndexStorage};
