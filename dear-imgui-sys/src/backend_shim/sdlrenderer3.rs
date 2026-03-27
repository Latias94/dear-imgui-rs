//! Low-level SDLRenderer3 backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_sdlrenderer3` backend.

use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn dear_imgui_backend_sdlrenderer3_init(renderer: *mut c_void) -> bool;
    pub fn dear_imgui_backend_sdlrenderer3_shutdown();
    pub fn dear_imgui_backend_sdlrenderer3_new_frame();
    pub fn dear_imgui_backend_sdlrenderer3_render_draw_data(
        draw_data: *const ImDrawData,
        render: *mut c_void,
    );
    pub fn dear_imgui_backend_sdlrenderer3_create_device_objects();
    pub fn dear_imgui_backend_sdlrenderer3_destroy_device_objects();
    pub fn dear_imgui_backend_sdlrenderer3_update_texture(texture: *mut ImTextureData);
}
