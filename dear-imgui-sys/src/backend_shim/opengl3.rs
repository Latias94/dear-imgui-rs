//! Low-level OpenGL3 backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_opengl3` backend.
//!
//! Callers must provide and own a current OpenGL / OpenGL ES context before
//! invoking initialization or per-frame rendering entry points here.
//!
//! On Android, this is commonly paired with `backend_shim::android` plus
//! application-owned EGL setup.

use std::ffi::c_char;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn dear_imgui_backend_opengl3_init(glsl_version: *const c_char) -> bool;
    pub fn dear_imgui_backend_opengl3_shutdown();
    pub fn dear_imgui_backend_opengl3_new_frame();
    pub fn dear_imgui_backend_opengl3_render_draw_data(draw_data: *const ImDrawData);
    pub fn dear_imgui_backend_opengl3_create_device_objects() -> bool;
    pub fn dear_imgui_backend_opengl3_destroy_device_objects();
    pub fn dear_imgui_backend_opengl3_update_texture(texture: *mut ImTextureData);
}
