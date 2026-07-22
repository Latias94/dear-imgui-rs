//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

#![allow(unsafe_op_in_unsafe_fn)]

mod callbacks;
mod events;
mod registry;
mod runtime;
#[cfg(test)]
mod tests;
mod viewport_data;

use std::sync::Arc;
use winit::window::Window;

pub(crate) use self::runtime::RuntimeControl;
pub use self::runtime::{EventLoopScope, WinitPlatformRuntime};
pub(crate) use self::viewport_data::client_to_screen_pos;
pub use crate::WinitPlatformError;

// Debug logging helper (off by default). Enable by building this crate with
// `--features mv-log`.
#[allow(unused_variables)]
fn mvlog(message: impl std::fmt::Display) {
    if cfg!(feature = "mv-log") {
        eprintln!("{message}");
    }
}

pub(crate) fn window_for_viewport(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Option<Arc<Window>> {
    if viewport.is_null() {
        return None;
    }
    self::registry::runtime_for_context(ctx)
        .and_then(|control| self::registry::window_for_viewport(&control, viewport))
}
