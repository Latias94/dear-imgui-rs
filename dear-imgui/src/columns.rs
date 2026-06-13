//! Legacy columns API
//!
//! Thin wrappers for the old Columns layout system. New code should prefer
//! the `table` API (`widget::table`) which supersedes Columns with more
//! features and better user experience.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]

mod counts;
mod flags;
mod index;
mod numeric;
mod resolve;
mod state;
#[cfg(test)]
mod tests;
mod token;
mod ui;

pub use flags::OldColumnFlags;
pub use index::{OldColumnIndex, OldColumnOffsetRef, OldColumnRef};
pub use token::{ColumnsBackgroundToken, ColumnsToken};
