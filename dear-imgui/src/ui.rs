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
use crate::context::ContextBinding;
use crate::context::SharedTextureRegistry;
use crate::draw::DrawListMut;
use crate::input::MouseCursor;
use crate::internal::RawWrapper;
use crate::scope::NativeScopeTracker;
use crate::string::UiBuffer;
use crate::sys;
use crate::texture::TextureRef;
use std::cell::{RefCell, UnsafeCell};

/// Represents the Dear ImGui user interface for one frame
#[derive(Debug)]
pub struct Ui {
    /// Dear ImGui context that owns this per-frame UI entry point.
    pub(crate) ctx: *mut sys::ImGuiContext,
    pub(crate) ctx_binding: ContextBinding,
    pub(crate) texture_registry: SharedTextureRegistry,
    pub(crate) native_scopes: RefCell<NativeScopeTracker>,
    /// Internal buffer for string operations
    buffer: UnsafeCell<UiBuffer>,
}
