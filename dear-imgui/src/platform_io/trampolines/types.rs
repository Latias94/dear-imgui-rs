use super::super::Viewport;
use crate::sys;
use core::ffi::c_char;
use std::ffi::c_void;

pub(in crate::platform_io) type ViewportCb = unsafe extern "C" fn(*mut Viewport);
pub(in crate::platform_io) type ViewportVec2Cb = unsafe extern "C" fn(*mut Viewport, sys::ImVec2);
pub(in crate::platform_io) type RawViewportVec2OutCb =
    unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2);
pub(in crate::platform_io) type ViewportVec2OutCb =
    unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec2);
pub(in crate::platform_io) type RawViewportVec4OutCb =
    unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4);
pub(in crate::platform_io) type ViewportVec4OutCb =
    unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec4);
pub(in crate::platform_io) type ViewportF32RetCb = unsafe extern "C" fn(*mut Viewport) -> f32;
pub(in crate::platform_io) type ViewportBoolRetCb = unsafe extern "C" fn(*mut Viewport) -> bool;
pub(in crate::platform_io) type ViewportTitleCb =
    unsafe extern "C" fn(*mut Viewport, *const c_char);
pub(in crate::platform_io) type ViewportF32Cb = unsafe extern "C" fn(*mut Viewport, f32);
pub(in crate::platform_io) type ViewportRenderCb = unsafe extern "C" fn(*mut Viewport, *mut c_void);
