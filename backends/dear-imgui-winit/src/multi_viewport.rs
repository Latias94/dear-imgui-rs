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

use winit::window::Window;

pub use self::runtime::{EventLoopScope, WinitPlatformError, WinitPlatformRuntime};

pub(crate) fn record_callback_panic(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    callback: &'static str,
) {
    if let Some(control) = self::registry::runtime_for_context(context) {
        control.record_fault(WinitPlatformError::CallbackPanicked { callback });
    }
}

// Debug logging helper (off by default). Enable by building this crate with
// `--features mv-log`.
#[allow(unused_variables)]
fn mvlog(message: impl std::fmt::Display) {
    if cfg!(feature = "mv-log") {
        eprintln!("{message}");
    }
}

pub(crate) unsafe fn window_ptr_for_viewport(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> *const Window {
    if viewport.is_null() {
        return std::ptr::null();
    }

    self::registry::runtime_for_context(ctx)
        .and_then(|control| self::registry::window_for_viewport(&control, viewport))
        .map_or(std::ptr::null(), |window| std::sync::Arc::as_ptr(&window))
}
