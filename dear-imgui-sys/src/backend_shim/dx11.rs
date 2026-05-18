//! Low-level DirectX 11 backend shim.
//!
//! This module wraps the repository-owned C shim around Dear ImGui's official
//! `imgui_impl_dx11` backend.
//!
//! # Safety
//!
//! Callers must provide valid `ID3D11Device*` and `ID3D11DeviceContext*`
//! handles, keep the intended Dear ImGui context current, and pair
//! init/new-frame/render/shutdown calls in the order required by the official
//! backend.
//!
//! Render and texture update entry points may mutate Dear ImGui texture feedback
//! stored in `ImDrawData` / `ImTextureData`, so Rust callers must pass uniquely
//! mutable pointers.

use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn dear_imgui_backend_dx11_init(device: *mut c_void, device_context: *mut c_void) -> bool;
    pub fn dear_imgui_backend_dx11_shutdown();
    pub fn dear_imgui_backend_dx11_new_frame();
    pub fn dear_imgui_backend_dx11_render_draw_data(draw_data: *mut ImDrawData);
    pub fn dear_imgui_backend_dx11_create_device_objects() -> bool;
    pub fn dear_imgui_backend_dx11_invalidate_device_objects();
    pub fn dear_imgui_backend_dx11_update_texture(texture: *mut ImTextureData);
}
