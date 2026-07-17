//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

#![allow(unsafe_op_in_unsafe_fn)]

mod callbacks;
mod event_loop;
mod events;
mod registry;
#[cfg(test)]
mod tests;
mod viewport_data;

use dear_imgui_rs::{Context, ContextBinding};
use std::cell::RefCell;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use self::callbacks::{install_platform_callbacks, setup_monitors_with_window};
pub use self::event_loop::{
    EventLoopFrameGuard, clear_event_loop, set_event_loop, set_event_loop_for_frame,
};
pub use self::events::{handle_event_with_multi_viewport, route_event_to_viewports};
#[cfg(test)]
use self::registry::unregister_viewport_data;
use self::registry::{
    drop_viewport_data, is_winit_viewport_data, register_viewport_data, viewport_data_ref,
    with_registered_context,
};
use self::viewport_data::{
    ViewportData, clear_main_viewport_data_for_current_context, init_main_viewport,
};

thread_local! {
    static EVENT_LOOP: RefCell<Option<*const ActiveEventLoop>> = const { RefCell::new(None) };
}

// Debug logging helper (off by default). Enable by building this crate with
// `--features mv-log`.
#[allow(unused_variables)]
fn mvlog(message: impl std::fmt::Display) {
    if cfg!(feature = "mv-log") {
        eprintln!("{message}");
    }
}

fn abort_on_panic<R>(name: &str, fallback: R, f: impl FnOnce(&ContextBinding) -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        with_registered_context(raw, f).unwrap_or(fallback)
    })) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-imgui-winit: panic in {}", name);
            std::process::abort();
        }
    }
}

/// Initialize multi-viewport support following official ImGui backend pattern
pub fn init_multi_viewport_support(ctx: &mut Context, main_window: &Window) {
    install_platform_callbacks(ctx);

    // Set up the main viewport
    init_main_viewport(ctx, main_window);

    // Set up monitors - required for multi-viewport (after main viewport exists)
    setup_monitors_with_window(main_window, ctx);
}

/// Shutdown multi-viewport support for `ctx`.
pub fn shutdown_multi_viewport_support(ctx: &mut Context) {
    let binding = ctx.binding();
    binding.with_bound_context(|| unsafe {
        // The main viewport is owned by the application, not by winit. Clear its winit-owned
        // sidecar data before asking Dear ImGui to destroy platform windows so upstream shutdown
        // assertions don't depend on Platform_DestroyWindow being installed for the main viewport.
        clear_main_viewport_data_for_current_context();
        ctx.destroy_platform_windows();
        clear_main_viewport_data_for_current_context();

        ctx.platform_io_mut().clear_platform_handlers();
    });
}

pub(crate) unsafe fn window_ptr_for_viewport(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> *const Window {
    if viewport.is_null() {
        return std::ptr::null();
    }

    with_registered_context(ctx, |_| {
        unsafe { viewport_data_ref(viewport) }
            .map(|vd| vd.window as *const Window)
            .unwrap_or(std::ptr::null())
    })
    .unwrap_or(std::ptr::null())
}
