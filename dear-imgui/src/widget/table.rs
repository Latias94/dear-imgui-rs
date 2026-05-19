//! Tables
//!
//! Modern multi-column layout and data table API with rich configuration.
//! Use `TableBuilder` to declare columns and build rows/cells ergonomically.
//!
//! Quick example (builder):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.table("perf")
//!     .flags(TableFlags::RESIZABLE)
//!     .column("Name").width(120.0).done()
//!     .column("Value").weight(1.0).done()
//!     .headers(true)
//!     .build(|ui| {
//!         ui.table_next_row();
//!         ui.table_next_column(); ui.text("CPU");
//!         ui.table_next_column(); ui.text("Intel");
//!     });
//! ```
//!
//! Quick example (manual API):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! if let Some(_t) = ui.begin_table("t", 2) {
//!     ui.table_next_row();
//!     ui.table_next_column(); ui.text("A");
//!     ui.table_next_column(); ui.text("B");
//! }
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
mod builder;
mod core;
mod flags;
mod headers;
mod indices;
mod setup;
mod sort;
#[cfg(test)]
mod tests;
mod tokens;
mod validation;

pub use builder::{ColumnBuilder, TableBuilder};
pub use flags::{TableBgTarget, TableRowFlags};
pub use headers::TableHeaderData;
pub use indices::{
    TableColumnIndex, TableColumnRef, TableContextMenuTarget, TableHoveredColumn, TableHoveredRow,
    TableRowIndex,
};
pub use setup::TableColumnSetup;
pub use sort::{SortDirection, TableColumnSortSpec, TableSortSpecs, TableSortSpecsIter};
pub use tokens::{TableBackgroundChannelToken, TableColumnChannelToken, TableToken};

pub(crate) use indices::TABLE_MAX_COLUMNS;
pub(crate) use validation::{
    assert_current_table, assert_current_table_cell, assert_current_table_has_flags,
    assert_current_table_row, assert_explicit_user_id, assert_non_negative_finite_f32,
    assert_table_column_width_phase, assert_table_setup_phase, assert_valid_table_column,
    assert_valid_table_column_in, assert_valid_table_column_raw_in, current_table_if_any,
    optional_user_id_from_raw, optional_user_id_raw, resolve_table_column,
    table_column_count_to_i32, table_freeze_count_to_i32,
};
