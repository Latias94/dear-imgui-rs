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
//! # Example - Multi-Viewport Support
//!
//! ```rust,no_run
//! # #[cfg(feature = "multi-viewport")]
//! # {
//! use std::sync::Arc;
//! use dear_imgui_rs::Context;
//! use dear_imgui_winit::{WinitPlatform, multi_viewport::WinitPlatformRuntime};
//! use winit::{event_loop::ActiveEventLoop, window::Window};
//!
//! fn attach(
//!     imgui: &mut Context,
//!     window: Arc<Window>,
//!     platform: &mut WinitPlatform,
//! ) -> Result<WinitPlatformRuntime, Box<dyn std::error::Error>> {
//!     platform.attach_window(Arc::clone(&window), dear_imgui_winit::HiDpiMode::Default, imgui)?;
//!     imgui.enable_multi_viewport();
//!     Ok(WinitPlatformRuntime::new(imgui, platform)?)
//! }
//!
//! fn update(
//!     runtime: &WinitPlatformRuntime,
//!     imgui: &mut Context,
//!     event_loop: &ActiveEventLoop,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     runtime.with_event_loop(event_loop, |_| imgui.update_platform_windows())?;
//!     Ok(())
//! }
//! # }
//! ```

mod cursor;
mod events;
mod input;
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport;
mod platform;
mod sanitize;
#[cfg(test)]
mod test_util;

// Re-export main types
pub use platform::{HiDpiMode, WinitPlatform, WinitPlatformError};
