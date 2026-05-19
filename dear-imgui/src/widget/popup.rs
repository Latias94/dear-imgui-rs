//! Popups and modals
//!
//! Popup windows (context menus, modals) with builders and token helpers to
//! ensure balanced begin/end calls.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

mod context;
mod flags;
mod modal;
mod tokens;
mod ui;

pub use context::{PopupContextMouseButton, PopupContextOptions};
pub use flags::PopupFlags;
pub use modal::ModalPopup;
pub use tokens::{ModalPopupToken, PopupToken};
