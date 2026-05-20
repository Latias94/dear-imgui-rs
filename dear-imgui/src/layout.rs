//! Layout and cursor helpers
//!
//! Spacing, separators, horizontal layout (`same_line`), grouping, cursor
//! positioning and clipping helpers. These functions help arrange widgets and
//! content within windows.
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.text("Left");
//! ui.same_line();
//! ui.text("Right");
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]

mod clip_rect;
mod cursor;
mod group;
mod metrics;
mod separator;
mod spacing;
mod stack_layout;
mod validation;

pub use clip_rect::ClipRectToken;
pub use group::GroupToken;
pub use stack_layout::{
    HorizontalStackLayoutToken, StackLayoutId, StackLayoutSuspensionToken, VerticalStackLayoutToken,
};
