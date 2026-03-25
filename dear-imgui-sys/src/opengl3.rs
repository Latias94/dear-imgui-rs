use std::ffi::c_char;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn ImGui_ImplOpenGL3_Init(glsl_version: *const c_char) -> bool;
    pub fn ImGui_ImplOpenGL3_Shutdown();
    pub fn ImGui_ImplOpenGL3_NewFrame();
    pub fn ImGui_ImplOpenGL3_RenderDrawData(draw_data: *mut ImDrawData);
    pub fn ImGui_ImplOpenGL3_CreateDeviceObjects() -> bool;
    pub fn ImGui_ImplOpenGL3_DestroyDeviceObjects();
    pub fn ImGui_ImplOpenGL3_UpdateTexture(texture: *mut ImTextureData);
}
