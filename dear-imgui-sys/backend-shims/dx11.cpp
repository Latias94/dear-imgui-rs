#include "imgui.h"
#include "backends/imgui_impl_dx11.h"

extern "C" {

bool dear_imgui_backend_dx11_init(void* device, void* device_context) {
    return ImGui_ImplDX11_Init(
        static_cast<ID3D11Device*>(device),
        static_cast<ID3D11DeviceContext*>(device_context)
    );
}

void dear_imgui_backend_dx11_shutdown() {
    ImGui_ImplDX11_Shutdown();
}

void dear_imgui_backend_dx11_new_frame() {
    ImGui_ImplDX11_NewFrame();
}

void dear_imgui_backend_dx11_render_draw_data(const ImDrawData* draw_data) {
    ImGui_ImplDX11_RenderDrawData(const_cast<ImDrawData*>(draw_data));
}

bool dear_imgui_backend_dx11_create_device_objects() {
    return ImGui_ImplDX11_CreateDeviceObjects();
}

void dear_imgui_backend_dx11_invalidate_device_objects() {
    ImGui_ImplDX11_InvalidateDeviceObjects();
}

void dear_imgui_backend_dx11_update_texture(ImTextureData* texture) {
    ImGui_ImplDX11_UpdateTexture(texture);
}

} // extern "C"
