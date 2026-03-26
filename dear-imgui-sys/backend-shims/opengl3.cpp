#include "imgui.h"
#include "backends/imgui_impl_opengl3.h"

extern "C" {

bool dear_imgui_backend_opengl3_init(const char* glsl_version) {
    return ImGui_ImplOpenGL3_Init(glsl_version);
}

void dear_imgui_backend_opengl3_shutdown() {
    ImGui_ImplOpenGL3_Shutdown();
}

void dear_imgui_backend_opengl3_new_frame() {
    ImGui_ImplOpenGL3_NewFrame();
}

void dear_imgui_backend_opengl3_render_draw_data(const ImDrawData* draw_data) {
    ImGui_ImplOpenGL3_RenderDrawData(const_cast<ImDrawData*>(draw_data));
}

bool dear_imgui_backend_opengl3_create_device_objects() {
    return ImGui_ImplOpenGL3_CreateDeviceObjects();
}

void dear_imgui_backend_opengl3_destroy_device_objects() {
    ImGui_ImplOpenGL3_DestroyDeviceObjects();
}

void dear_imgui_backend_opengl3_update_texture(ImTextureData* texture) {
    ImGui_ImplOpenGL3_UpdateTexture(texture);
}

} // extern "C"
