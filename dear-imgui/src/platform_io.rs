//! Platform IO functionality for Dear ImGui
//!
//! This module provides access to Dear ImGui's platform IO system, which handles
//! multi-viewport and platform-specific functionality.

// Multi-viewport requires platform callbacks (C calling back into Rust). On the
// web (wasm32 import-style build), this is not supported yet, so we currently
// disable the `multi-viewport` feature for wasm at compile time.
#[cfg(all(target_arch = "wasm32", feature = "multi-viewport"))]
compile_error!("The `multi-viewport` feature is not supported on wasm32 targets yet.");

mod core;
#[cfg(feature = "multi-viewport")]
mod trampolines;
mod viewport;

#[cfg(feature = "multi-viewport")]
mod accessors;
#[cfg(feature = "multi-viewport")]
mod platform_callbacks;
mod render_state;
#[cfg(feature = "multi-viewport")]
mod renderer_callbacks;
#[cfg(test)]
mod tests;
mod textures;

pub use core::PlatformIo;
pub use viewport::Viewport;

pub(crate) use core::{
    clear_out_param_callbacks_for_current_context, clear_typed_callbacks_for_context,
};
