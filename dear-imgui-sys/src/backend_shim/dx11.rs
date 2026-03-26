use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn dear_imgui_backend_dx11_init(device: *mut c_void, device_context: *mut c_void) -> bool;
    pub fn dear_imgui_backend_dx11_shutdown();
    pub fn dear_imgui_backend_dx11_new_frame();
    pub fn dear_imgui_backend_dx11_render_draw_data(draw_data: *const ImDrawData);
    pub fn dear_imgui_backend_dx11_create_device_objects() -> bool;
    pub fn dear_imgui_backend_dx11_invalidate_device_objects();
    pub fn dear_imgui_backend_dx11_update_texture(texture: *mut ImTextureData);
}
