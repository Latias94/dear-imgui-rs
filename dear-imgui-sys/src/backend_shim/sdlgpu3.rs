use crate::{ImDrawData, ImTextureData};
use std::ffi::{c_int, c_void};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ImGui_ImplSDLGPU3_InitInfo {
    pub device: *mut c_void,
    pub colorTargetFormat: c_int,
    pub MSAASamples: c_int,
    pub SwapchainComposition: c_int, // Only used in multi-viewports mode.
    pub PresentMode: c_int,          // Only used in multi-viewports mode.
}

unsafe extern "C" {
    pub fn dear_imgui_backend_sdlgpu3_init(info: *mut ImGui_ImplSDLGPU3_InitInfo) -> bool;
    pub fn dear_imgui_backend_sdlgpu3_shutdown();
    pub fn dear_imgui_backend_sdlgpu3_new_frame();
    pub fn dear_imgui_backend_sdlgpu3_prepare_draw_data(draw_data: *mut ImDrawData,
                                                        command_buffer: *mut c_void);
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
