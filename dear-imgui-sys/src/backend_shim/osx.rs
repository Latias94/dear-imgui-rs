//! Low-level official macOS Cocoa platform backend.
//!
//! # Safety
//!
//! Call on the Cocoa main thread with the intended Dear ImGui context current and a live
//! `NSView*` attached to the application's window. Keep the view/window alive until shutdown
//! returns, serialize backend calls, and shut down before destroying the context. The application
//! owns its event loop and autorelease pool. Input is handled by the upstream backend's view/event
//! hooks; no obsolete `ImGui_ImplOSX_HandleEvent` API is exposed.
//! Upstream multi-viewport limitations still apply (decorations and parent viewport handling).

use std::ffi::c_void;

unsafe extern "C" {
    pub fn dear_imgui_backend_osx_init(view: *mut c_void) -> bool;
    pub fn dear_imgui_backend_osx_shutdown();
    /// A null view is accepted by upstream when only updating timing/platform state.
    pub fn dear_imgui_backend_osx_new_frame(view: *mut c_void);
}
