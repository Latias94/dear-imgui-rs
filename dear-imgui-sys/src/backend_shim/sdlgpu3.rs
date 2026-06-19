//! Low-level SDLGPU3 backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_sdlgpu3` backend.
//!
//! # Safety
//!
//! Callers must provide a valid `SDL_GPUDevice*`, command buffers and render
//! passes owned by that device, keep the intended Dear ImGui context current,
//! and pair init/new-frame/prepare/render/shutdown calls in the order required
//! by the official backend.

use std::ffi::{c_int, c_void};

use crate::{ImDrawData, ImTextureData};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ImGui_ImplSDLGPU3_InitInfo {
    pub device: *mut c_void,
    pub color_target_format: c_int,
    pub msaa_samples: c_int,
    pub swapchain_composition: c_int,
    pub present_mode: c_int,
}

unsafe extern "C" {
    pub fn dear_imgui_backend_sdlgpu3_init(info: *mut ImGui_ImplSDLGPU3_InitInfo) -> bool;
    pub fn dear_imgui_backend_sdlgpu3_shutdown();
    pub fn dear_imgui_backend_sdlgpu3_new_frame();
    pub fn dear_imgui_backend_sdlgpu3_prepare_draw_data(
        draw_data: *mut ImDrawData,
        command_buffer: *mut c_void,
    );
    pub fn dear_imgui_backend_sdlgpu3_render_draw_data(
        draw_data: *mut ImDrawData,
        command_buffer: *mut c_void,
        render_pass: *mut c_void,
        pipeline: *mut c_void,
    );
    pub fn dear_imgui_backend_sdlgpu3_create_device_objects();
    pub fn dear_imgui_backend_sdlgpu3_destroy_device_objects();
    pub fn dear_imgui_backend_sdlgpu3_update_texture(texture: *mut ImTextureData);
}
