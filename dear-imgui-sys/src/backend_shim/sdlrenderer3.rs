//! Low-level SDLRenderer3 backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_sdlrenderer3` backend.
//!
//! # Safety
//!
//! Callers must provide a valid `SDL_Renderer*` associated with the active SDL3
//! window, keep the intended Dear ImGui context current, and pair init/new-frame
//!/render/shutdown calls in the order required by the official backend.
//!
//! Render and texture update entry points may mutate Dear ImGui texture feedback
//! stored in `ImDrawData` / `ImTextureData`, so Rust callers must pass uniquely
//! mutable pointers.

use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn dear_imgui_backend_sdlrenderer3_init(renderer: *mut c_void) -> bool;
    pub fn dear_imgui_backend_sdlrenderer3_shutdown();
    pub fn dear_imgui_backend_sdlrenderer3_new_frame();
    pub fn dear_imgui_backend_sdlrenderer3_render_draw_data(
        draw_data: *mut ImDrawData,
        renderer: *mut c_void,
    );
    pub fn dear_imgui_backend_sdlrenderer3_create_device_objects();
    pub fn dear_imgui_backend_sdlrenderer3_destroy_device_objects();
    pub fn dear_imgui_backend_sdlrenderer3_update_texture(texture: *mut ImTextureData);
}
