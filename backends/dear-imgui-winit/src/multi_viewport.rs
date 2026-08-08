//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(doctest)]
mod removed_runtime_facade_contract {
    /// ```compile_fail
    /// use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;
    /// ```
    struct RemovedRuntimeFacade;

    /// Renderer ownership is captured atomically by the route adapter rather than preflighted
    /// through a public phase method.
    ///
    /// ```compile_fail
    /// use dear_imgui_winit::WinitPlatform;
    /// let _ = WinitPlatform::validate_renderer_owner;
    /// ```
    struct RemovedRendererOwnerPreflight;
}

mod callbacks;
mod coordinates;
pub(crate) mod events;
mod focus;
mod native_cursor_hittest;
mod registry;
mod runtime;
#[cfg(test)]
mod tests;
mod viewport_data;

use std::sync::Arc;
use winit::window::Window;

pub(crate) use self::coordinates::{
    client_physical_to_screen_pos, desktop_size_for_window, framebuffer_scale_for_window,
    ime_cursor_area_for_viewport, single_window_display_metrics, window_position_from_desktop,
    window_size_from_desktop,
};
pub(crate) use self::runtime::RuntimeControl;
pub use self::runtime::{EventLoopScope, WinitViewportAttempt, WinitViewportRendererAdapter};
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
