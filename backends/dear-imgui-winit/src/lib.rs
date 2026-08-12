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
//! use dear_imgui_rs::Context;
//! use dear_imgui_winit::WinitPlatform;
//! use winit::event_loop::EventLoop;
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut imgui_ctx = Context::create();
//! let mut platform = WinitPlatform::new(&mut imgui_ctx).expect("create Winit platform");
//!
//! // Use in your event loop...
//! ```
//!
//! # Multi-Viewport Support
//!
//! `WinitPlatform` owns native windows and event routing, while the selected renderer route owns
//! the complete viewport-frame transaction. Use that route's `prepare(...)` entry instead of
//! calling Dear ImGui's platform-window phases directly. See the repository's
//! `multi_viewport_wgpu` and `multi_viewport_ash` examples for complete integrations.

mod cursor;
mod events;
mod input;
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport;
#[cfg(feature = "native-platform-support")]
pub mod native_support;
mod platform;
mod sanitize;
#[cfg(test)]
mod test_util;

// Re-export main types
pub use platform::{HiDpiMode, WinitPlatform, WinitPlatformError};
