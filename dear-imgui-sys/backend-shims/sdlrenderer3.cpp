#include "imgui.h"
#include "backends/imgui_impl_sdlrenderer3.h"

#include <SDL3/SDL.h>

extern "C" {

bool dear_imgui_backend_sdlrenderer3_init(void* renderer) {
    return ImGui_ImplSDLRenderer3_Init(static_cast<SDL_Renderer*>(renderer));
}

void dear_imgui_backend_sdlrenderer3_shutdown() {
    ImGui_ImplSDLRenderer3_Shutdown();
}

void dear_imgui_backend_sdlrenderer3_new_frame() {
    ImGui_ImplSDLRenderer3_NewFrame();
}

void dear_imgui_backend_sdlrenderer3_render_draw_data(const ImDrawData* draw_data, void* renderer) {
    ImGui_ImplSDLRenderer3_RenderDrawData(const_cast<ImDrawData*>(draw_data), static_cast<SDL_Renderer*>(renderer));
}

void dear_imgui_backend_sdlrenderer3_create_device_objects() {
    ImGui_ImplSDLRenderer3_CreateDeviceObjects();
}

void dear_imgui_backend_sdlrenderer3_destroy_device_objects() {
    ImGui_ImplSDLRenderer3_DestroyDeviceObjects();
}

void dear_imgui_backend_sdlrenderer3_update_texture(ImTextureData* tex) {
    ImGui_ImplSDLRenderer3_UpdateTexture(tex);
}

} // extern "C"
