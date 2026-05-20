//! Styling and colors
//!
//! High-level access to Dear ImGui style parameters and color table. Use this
//! module to read or tweak padding, rounding, sizes and retrieve or modify
//! named colors via [`StyleColor`].
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! // Adjust style before building a frame
//! {
//!     let style = ctx.style_mut();
//!     style.set_window_rounding(6.0);
//!     style.set_color(StyleColor::WindowBg, [0.10, 0.10, 0.12, 1.0]);
//! }
//! // Optionally show the style editor for the current style
//! # let ui = ctx.frame();
//! ui.show_default_style_editor();
//! ```
//!
//! Quick example (temporary style color):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let c = ui.push_style_color(StyleColor::Text, [0.2, 1.0, 0.2, 1.0]);
//! ui.text("green text");
//! c.pop();
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
mod color;
mod core;
mod direction;
mod docking;
mod drag_drop;
mod font;
mod hover;
mod layout;
mod rendering;
mod separators;
mod spacing;
mod tabs_tables;
mod theme;
mod tree;
mod validation;
mod var;

pub use color::StyleColor;
pub use core::Style;
pub use direction::Direction;
pub use theme::{ColorOverride, StyleTweaks, TableTheme, Theme, ThemePreset, WindowTheme};
pub use tree::TreeLineMode;
pub use var::StyleVar;

pub(crate) use validation::{validate_style_color, validate_style_var};
