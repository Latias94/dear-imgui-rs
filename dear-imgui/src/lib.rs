//! # Dear ImGui for Rust
//!
//! Safe, idiomatic Rust bindings for the Dear ImGui immediate mode GUI library.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui::*;
//!
//! fn main() -> Result<()> {
//!     let mut ctx = Context::new()?;
//!     
//!     loop {
//!         let mut frame = ctx.frame();
//!         
//!         frame.window("Hello, World!")
//!             .size([400.0, 300.0])
//!             .show(|ui| {
//!                 ui.text("Hello, Dear ImGui!");
//!                 if ui.button("Exit") {
//!                     return false;
//!                 }
//!                 true
//!             });
//!             
//!         // Render frame...
//!         break; // Exit for example
//!     }
//!     
//!     Ok(())
//! }
//! ```

// Core modules
pub mod clipboard;
pub mod context;
pub mod demo;
pub mod draw_data;
pub mod error;
pub mod fonts;
pub mod frame;
pub mod io;
pub mod navigation;
pub mod style;
pub mod types;
pub mod ui;
pub mod utils;
pub mod widget;
pub mod window;

// Re-exports for convenience
pub use context::Context;
pub use draw_data::{DrawCmd, DrawData, DrawList};
pub use error::{ImGuiError, Result};
pub use frame::Frame;
pub use io::{Io, ConfigFlags, BackendFlags, MouseCursor};
pub use style::{Style, StyleColor, StyleVar, Direction};
pub use types::*;
pub use window::{Window, WindowFlags};

// Prelude for convenient imports
pub mod prelude;
