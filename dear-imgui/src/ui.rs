//! Per-frame UI entry point
//!
//! The `Ui` type exposes most user-facing Dear ImGui APIs for a single frame:
//! creating windows, drawing widgets, accessing draw lists, showing built-in
//! tools and more. Obtain it from [`Context::frame`].
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! let mut ctx = Context::create();
//! let ui = ctx.frame();
//! ui.text("Hello, world!");
//! ```
//!
mod core;
mod debug_tools;
mod draw;
mod navigation;
mod style;
mod viewport;
mod widgets;
mod window;

use crate::Id;
use crate::draw::DrawListMut;
use crate::input::MouseCursor;
use crate::internal::RawWrapper;
use crate::string::UiBuffer;
use crate::sys;
use crate::texture::TextureRef;
use std::cell::UnsafeCell;

/// Represents the Dear ImGui user interface for one frame
#[derive(Debug)]
pub struct Ui {
    /// Internal buffer for string operations
    buffer: UnsafeCell<UiBuffer>,
}
