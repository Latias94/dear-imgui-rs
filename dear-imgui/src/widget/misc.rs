//! Miscellaneous widgets
//!
//! Small convenience widgets that don’t fit elsewhere (e.g. bullets, help
//! markers). See functions on `Ui` for details.
//!
//! Arrow buttons use [`crate::Direction`] directly. The `ArrowDirection` alias is intentionally
//! unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::ArrowDirection;
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

mod basic;
mod button_repeat;
mod disabled;
mod invisible_button;
mod item_key;
mod validation;

#[cfg(test)]
mod tests;

pub use button_repeat::ButtonRepeatToken;
pub use disabled::DisabledToken;
pub use invisible_button::{ButtonFlags, InvisibleButtonMouseButtons, InvisibleButtonOptions};
