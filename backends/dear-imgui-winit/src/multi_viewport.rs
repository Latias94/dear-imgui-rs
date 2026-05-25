//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

#![allow(unsafe_op_in_unsafe_fn)]

mod callbacks;
mod context_binding;
mod event_loop;
mod events;
mod registry;
#[cfg(test)]
mod tests;
mod viewport_data;

use dear_imgui_rs::Context;
use std::cell::RefCell;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use self::callbacks::{install_platform_callbacks, setup_monitors_with_window};
use self::context_binding::CurrentContextGuard;
pub use self::event_loop::{
    EventLoopFrameGuard, clear_event_loop, set_event_loop, set_event_loop_for_frame,
};
pub use self::events::{handle_event_with_multi_viewport, route_event_to_viewports};
use self::registry::{
    drop_viewport_data, is_winit_viewport_data, register_viewport_data, viewport_data_ref,
};
use self::viewport_data::{
    ViewportData, clear_main_viewport_data_for_current_context, init_main_viewport,
};

thread_local! {
    static EVENT_LOOP: RefCell<Option<*const ActiveEventLoop>> = const { RefCell::new(None) };
    static VIEWPORT_DATA: RefCell<Vec<(*mut dear_imgui_rs::sys::ImGuiContext, *mut ViewportData)>> = const { RefCell::new(Vec::new()) };
}

// Debug logging helper (off by default). Enable by building this crate with
// `--features mv-log`.
#[allow(unused_variables)]
fn mvlog(message: impl std::fmt::Display) {
    if cfg!(feature = "mv-log") {
        eprintln!("{message}");
    }
}

fn abort_on_panic<R>(name: &str, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-imgui-winit: panic in {}", name);
            std::process::abort();
        }
    }
}

/// Initialize multi-viewport support following official ImGui backend pattern
pub fn init_multi_viewport_support(ctx: &mut Context, main_window: &Window) {
    let _context_guard = unsafe { CurrentContextGuard::bind(ctx.as_raw()) };

    install_platform_callbacks(ctx);

    // Set up the main viewport
    init_main_viewport(ctx, main_window);

    // Set up monitors - required for multi-viewport (after main viewport exists)
    unsafe {
        setup_monitors_with_window(main_window, ctx);
    }
}

/// Shutdown multi-viewport support for `ctx`.
pub fn shutdown_multi_viewport_support(ctx: &mut Context) {
    // Clean up any remaining viewports
    unsafe {
        let _context_guard = CurrentContextGuard::bind(ctx.as_raw());

        // The main viewport is owned by the application, not by winit. Clear its winit-owned
        // sidecar data before asking Dear ImGui to destroy platform windows so upstream shutdown
        // assertions don't depend on Platform_DestroyWindow being installed for the main viewport.
        clear_main_viewport_data_for_current_context();
        ctx.destroy_platform_windows();
        clear_main_viewport_data_for_current_context();

        ctx.platform_io_mut().clear_platform_handlers();
    }
}

pub(crate) unsafe fn window_ptr_for_viewport(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> *const Window {
    if viewport.is_null() {
        return std::ptr::null();
    }

    let _context_guard = if ctx.is_null() {
        None
    } else {
        Some(unsafe { CurrentContextGuard::bind(ctx) })
    };

    unsafe { viewport_data_ref(viewport) }
        .map(|vd| vd.window as *const Window)
        .unwrap_or(std::ptr::null())
}
