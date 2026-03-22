use crate::{ImDrawData, ImTextureData};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};

unsafe extern "C" {
    pub fn ImGui_ImplDX11_Init(device: ID3D11Device, device_context: ID3D11DeviceContext) -> bool;
    pub fn ImGui_ImplDX11_Shutdown();
    pub fn ImGui_ImplDX11_NewFrame();
    pub fn ImGui_ImplDX11_RenderDrawData(draw_data: *mut ImDrawData);
    pub fn ImGui_ImplDX11_CreateDeviceObjects();
    pub fn ImGui_ImplDX11_InvalidateDeviceObjects();
    pub fn ImGui_ImplDX11_UpdateTexture(texture: *mut ImTextureData);
}
