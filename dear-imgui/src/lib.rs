//! # Dear ImGui - Rust Bindings with Docking Support
//!
//! High-level Rust bindings for Dear ImGui, the immediate mode GUI library.
//! This crate provides safe, idiomatic Rust bindings with full support for
//! docking and multi-viewport features.
//!
//! ## Features
//!
//! - Safe, idiomatic Rust API
//! - Full docking and multi-viewport support
//! - Builder pattern for widgets
//! - Memory-safe string handling
//! - Integration with modern Rust graphics ecosystems
//!
//! ## Quick Start
//!
//! ```no_run
//! use dear_imgui::*;
//!
//! let mut ctx = Context::create_or_panic();
//! let ui = ctx.frame();
//!
//! ui.window("Hello World")
//!     .size([300.0, 100.0], Condition::FirstUseEver)
//!     .build(|| {
//!         ui.text("Hello, world!");
//!         ui.text("This is Dear ImGui with docking support!");
//!     });
//! ```

#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::float_cmp))]

// Re-export the sys crate for advanced users
pub extern crate dear_imgui_sys as sys;

/// Condition for setting window/widget properties
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
pub enum Condition {
    /// Never apply the setting
    Never = -1,
    /// Set the variable always
    Always = sys::ImGuiCond_Always as i32,
    /// Set the variable once per runtime session (only the first call will succeed)
    Once = sys::ImGuiCond_Once as i32,
    /// Set the variable if the object/window has no persistently saved data (no entry in .ini file)
    FirstUseEver = sys::ImGuiCond_FirstUseEver as i32,
    /// Set the variable if the object/window is appearing after being hidden/inactive (or the first time)
    Appearing = sys::ImGuiCond_Appearing as i32,
}

// use std::cell;
// use std::os::raw::c_char;

// Core modules
pub use self::clipboard::{ClipboardBackend, DummyClipboardBackend};
pub use self::context::*;
// Note: draw types are now in render module
pub use self::fonts::*;
pub use self::input::*;
pub use self::io::*;
pub use self::platform_io::*;
pub use self::string::*;
pub use self::style::*;
pub use self::ui::*;

// Utility modules
pub use self::list_clipper::*;
// pub use self::math::*;

// Widget modules
pub use self::widget::*;
pub use self::window::*;

// Stack management
pub use self::stacks::*;

// Layout and cursor control
pub use self::layout::*;

// Drag and drop system
pub use self::drag_drop::*;

// Text filtering system
pub use self::text_filter::*;

// Column layout system (included in layout module)
pub use self::columns::*;

// Internal modules
mod clipboard;
mod color;
mod context;
#[cfg(feature = "docking")]
mod dock_builder;
mod dock_space;
mod draw;
mod error;
mod fonts;
pub mod input;
pub mod internal;
mod io;
mod list_clipper;
pub mod platform_io;
pub mod render;
mod string;
mod style;
pub mod texture;
mod ui;
mod utils;
#[cfg(feature = "multi-viewport")]
pub mod viewport_backend;
// mod math;
mod widget;
mod window;

// Token system for resource management
#[macro_use]
mod tokens;

// Stack management
mod stacks;

// Layout and cursor control
mod layout;

// Drag and drop system
mod drag_drop;

// Text filtering system
mod text_filter;

// Column layout system
mod columns;

// Logging utilities
pub mod logging;

// Re-export public API
pub use color::*;
#[cfg(feature = "docking")]
pub use dock_builder::*;
pub use dock_space::*;
// Export DrawListMut for extensions
pub use draw::DrawListMut;
pub use error::*;
// Note: draw types are now in render module, no need to export draw::*
pub use render::*;
pub use texture::*;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if docking features are available
pub const HAS_DOCKING: bool = sys::HAS_DOCKING;

/// Returns the underlying Dear ImGui library version
#[doc(alias = "GetVersion")]
pub fn dear_imgui_version() -> &'static str {
    unsafe {
        let version_ptr = sys::igGetVersion();

        let bytes = std::ffi::CStr::from_ptr(version_ptr).to_bytes();
        std::str::from_utf8_unchecked(bytes)
    }
}
