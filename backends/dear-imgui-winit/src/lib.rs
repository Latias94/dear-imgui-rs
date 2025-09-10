//! Winit platform backend for Dear ImGui
//!
//! This crate provides a platform backend for Dear ImGui that integrates with
//! the winit windowing library. It handles window events, input processing,
//! and platform-specific functionality including multi-viewport support.
//!
//! # Features
//!
//! - **Basic Platform Support**: Window events, input handling, cursor management
//! - **Multi-Viewport Support**: Create and manage multiple OS windows (requires `multi-viewport` feature)
//! - **DPI Awareness**: Proper handling of high-DPI displays
//!
//! # Example - Basic Usage
//!
//! ```rust,no_run
//! use dear_imgui::Context;
//! use dear_imgui_winit::WinitPlatform;
//! use winit::event_loop::EventLoop;
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut imgui_ctx = Context::create_or_panic();
//! let mut platform = WinitPlatform::new(&mut imgui_ctx);
//!
//! // Use in your event loop...
//! ```
//!
//! # Example - Multi-Viewport Support
//!
//! ```rust,no_run
//! # #[cfg(feature = "multi-viewport")]
//! # {
//! use dear_imgui::Context;
//! use dear_imgui_winit::{WinitPlatform, multi_viewport};
//! use winit::event_loop::EventLoop;
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut imgui_ctx = Context::create_or_panic();
//! imgui_ctx.enable_multi_viewport();
//!
//! let mut platform = WinitPlatform::new(&mut imgui_ctx);
//!
//! // In your event loop:
//! // multi_viewport::set_event_loop(&event_loop);
//! // multi_viewport::init_multi_viewport_support(&mut imgui_ctx, &window);
//! # }
//! ```

mod cursor;
mod events;
mod input;
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport;
mod platform;

// Re-export main types
pub use platform::{HiDpiMode, WinitPlatform};
