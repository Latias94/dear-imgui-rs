use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn ImGui_ImplDX11_Init(device: *mut c_void, device_context: *mut c_void) -> bool;
    pub fn ImGui_ImplDX11_Shutdown();
    pub fn ImGui_ImplDX11_NewFrame();
    pub fn ImGui_ImplDX11_RenderDrawData(draw_data: *mut ImDrawData);
    pub fn ImGui_ImplDX11_CreateDeviceObjects();
    pub fn ImGui_ImplDX11_InvalidateDeviceObjects();
    pub fn ImGui_ImplDX11_UpdateTexture(texture: *mut ImTextureData);
}
