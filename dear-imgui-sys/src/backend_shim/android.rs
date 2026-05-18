//! Low-level Android backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_android` backend.
//!
//! # Safety
//!
//! Callers must provide valid `ANativeWindow*` and `AInputEvent*` pointers, keep
//! the intended Dear ImGui context current, and pair init/new-frame/input/shutdown
//! calls in the order required by the official backend. Android activity, EGL /
//! GLES context creation, and APK packaging remain owned by the application.
//!
//! Typical usage:
//!
//! - initialize with an `ANativeWindow*`
//! - call `dear_imgui_backend_android_new_frame()` once per frame
//! - optionally forward raw `AInputEvent*` values through
//!   `dear_imgui_backend_android_handle_input_event()`
//! - shut down before destroying the associated window / activity state
//!
//! If your Android stack uses a higher-level input wrapper such as
//! `android-activity`, you may instead translate input into `dear-imgui-rs::Io`
//! manually and only reuse the window + frame lifecycle parts here.

use std::ffi::c_void;

unsafe extern "C" {
    pub fn dear_imgui_backend_android_init(window: *mut c_void) -> bool;
    pub fn dear_imgui_backend_android_handle_input_event(input_event: *const c_void) -> i32;
    pub fn dear_imgui_backend_android_shutdown();
    pub fn dear_imgui_backend_android_new_frame();
}
