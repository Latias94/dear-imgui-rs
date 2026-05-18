//! Low-level Win32 platform backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_win32` backend.
//!
//! # Safety
//!
//! Callers must provide valid Win32 `HWND` / message handles, keep the intended
//! Dear ImGui context current, and pair init/new-frame/message/shutdown calls in
//! the order required by the official backend. The DPI helpers forward directly
//! to upstream Win32 backend functions and require the corresponding raw Win32
//! handles to be valid for the duration of each call.

use std::ffi::c_void;

pub type Hwnd = *mut c_void;
pub type Wparam = usize;
pub type Lparam = isize;
pub type Lresult = isize;

unsafe extern "C" {
    pub fn dear_imgui_backend_win32_init(hwnd: Hwnd) -> bool;
    pub fn dear_imgui_backend_win32_init_for_opengl(hwnd: Hwnd) -> bool;
    pub fn dear_imgui_backend_win32_shutdown();
    pub fn dear_imgui_backend_win32_new_frame();
    pub fn dear_imgui_backend_win32_wnd_proc_handler(
        hwnd: Hwnd,
        msg: u32,
        wparam: Wparam,
        lparam: Lparam,
    ) -> Lresult;
    pub fn dear_imgui_backend_win32_enable_dpi_awareness();
    pub fn dear_imgui_backend_win32_get_dpi_scale_for_hwnd(hwnd: Hwnd) -> f32;
    pub fn dear_imgui_backend_win32_get_dpi_scale_for_monitor(monitor: *mut c_void) -> f32;
    pub fn dear_imgui_backend_win32_enable_alpha_compositing(hwnd: Hwnd);
}
