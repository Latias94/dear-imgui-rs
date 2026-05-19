//! Input types (mouse, keyboard, cursors)
//!
//! Strongly-typed identifiers for mouse buttons, mouse cursors and keyboard
//! keys used by Dear ImGui. Backends typically translate platform events into
//! these enums when feeding input into `Io`.
//!
//! See [`io`] for the per-frame input state and configuration.
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
    InputFlags, ItemKeyOwnerFlags, NextItemShortcutFlags, NextItemShortcutOptions, ShortcutFlags,
    ShortcutGlobalRouteFlags, ShortcutOptions, ShortcutRoute,
};
pub use text_flags::{InputScalarFlags, InputTextFlags, InputTextMultilineFlags};
