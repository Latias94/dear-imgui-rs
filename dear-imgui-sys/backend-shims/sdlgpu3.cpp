#include "imgui.h"
#include "backends/imgui_impl_sdlgpu3.h"

#include <SDL3/SDL.h>

extern "C" {

bool dear_imgui_backend_sdlgpu3_init(ImGui_ImplSDLGPU3_InitInfo* info) {
    return ImGui_ImplSDLGPU3_Init(info);
}

void dear_imgui_backend_sdlgpu3_shutdown() {
    ImGui_ImplSDLGPU3_Shutdown();
}

void dear_imgui_backend_sdlgpu3_new_frame() {
    ImGui_ImplSDLGPU3_NewFrame();
}

void dear_imgui_backend_sdlgpu3_prepare_draw_data(ImDrawData* draw_data, SDL_GPUCommandBuffer* buffer) {
    ImGui_ImplSDLGPU3_PrepareDrawData(draw_data, buffer);
}

void dear_imgui_backend_sdlgpu3_render_draw_data(ImDrawData* draw_data, SDL_GPUCommandBuffer* buffer, SDL_GPURenderPass* render_pass, SDL_GPUGraphicsPipeline* pipeline) {
    ImGui_ImplSDLGPU3_RenderDrawData(draw_data, buffer, render_pass, pipeline);
}

void dear_imgui_backend_sdlgpu3_create_device_objects() {
    ImGui_ImplSDLGPU3_CreateDeviceObjects();
}

void dear_imgui_backend_sdlgpu3_destroy_device_objects() {
    ImGui_ImplSDLGPU3_DestroyDeviceObjects();
}

void dear_imgui_backend_sdlgpu3_update_texture(ImTextureData* tex) {
    ImGui_ImplSDLGPU3_UpdateTexture(tex);
}

} // extern "C"
