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
//! let mut ctx = Context::create();
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
    Always = sys::ImGuiCond_Always,
    /// Set the variable once per runtime session (only the first call will succeed)
    Once = sys::ImGuiCond_Once,
    /// Set the variable if the object/window has no persistently saved data (no entry in .ini file)
    FirstUseEver = sys::ImGuiCond_FirstUseEver,
    /// Set the variable if the object/window is appearing after being hidden/inactive (or the first time)
    Appearing = sys::ImGuiCond_Appearing,
}

// use std::cell;
// use std::os::raw::c_char;

// Core modules
pub use self::context::*;
pub use self::draw::*;
pub use self::fonts::*;
pub use self::input::*;
pub use self::io::*;
pub use self::string::*;
pub use self::style::*;
pub use self::ui::*;

// Utility modules
pub use self::list_clipper::*;
// pub use self::math::*;

// Widget modules
pub use self::widget::*;
pub use self::window::*;

// Internal modules
mod color;
mod context;
mod draw;
mod fonts;
mod input;
pub mod internal;
mod io;
mod list_clipper;
mod string;
mod style;
mod ui;
// mod math;
mod widget;
mod window;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if docking features are available
pub const HAS_DOCKING: bool = sys::HAS_DOCKING;

/// Returns the underlying Dear ImGui library version
#[doc(alias = "GetVersion")]
pub fn dear_imgui_version() -> &'static str {
    unsafe {
        let bytes =
            std::ffi::CStr::from_ptr(sys::wrapper_functions::windows::get_version()).to_bytes();
        std::str::from_utf8_unchecked(bytes)
    }
}
