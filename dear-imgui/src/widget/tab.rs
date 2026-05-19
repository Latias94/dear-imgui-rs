//! Tabs
//!
//! Tab bars and tab items for organizing content. Builders manage begin/end
//! lifetimes to help keep API usage balanced.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

mod bar;
mod bar_options;
mod item;
mod item_options;
mod tokens;
mod ui;

pub use bar::TabBar;
pub use bar_options::{TabBarFittingPolicy, TabBarFlags, TabBarOptions};
pub use item::TabItem;
pub use item_options::{TabItemFlags, TabItemOptions, TabItemPlacement};
pub use tokens::{TabBarToken, TabItemToken};
