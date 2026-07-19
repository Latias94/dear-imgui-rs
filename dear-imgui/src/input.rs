//! Input types (mouse, keyboard, cursors)
//!
//! Strongly-typed identifiers for mouse buttons, mouse cursors and keyboard
//! keys used by Dear ImGui. Backends typically translate platform events into
//! these enums when feeding input into `Io`.
//!
//! See [`crate::Io`] for the per-frame input state and configuration.
//!
//! Shortcut configuration uses [`ShortcutOptions`]. The ambiguous `InputFlags` alias is
//! intentionally unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::InputFlags;
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]
mod keyboard;
mod mouse;
mod shortcut;
mod text_flags;
mod ui;

pub use keyboard::{Key, KeyChord, KeyMods};
pub use mouse::{MouseButton, MouseCursor, MouseSource};
pub use shortcut::{
    ItemKeyOwnerFlags, NextItemShortcutFlags, NextItemShortcutOptions, ShortcutFlags,
    ShortcutGlobalRouteFlags, ShortcutOptions, ShortcutRoute,
};
pub use text_flags::{InputScalarFlags, InputTextFlags, InputTextMultilineFlags};
