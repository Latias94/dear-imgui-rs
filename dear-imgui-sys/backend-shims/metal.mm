#import <Metal/Metal.h>
#include <cstdlib>
#include "imgui.h"
#include "backends/imgui_impl_metal.h"

extern "C" {
bool dear_imgui_backend_metal_init(void* device) noexcept {
    @try { return ImGui_ImplMetal_Init((__bridge id<MTLDevice>)device); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_metal_shutdown() noexcept {
    @try { ImGui_ImplMetal_Shutdown(); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_metal_new_frame(void* render_pass_descriptor) noexcept {
    @try { ImGui_ImplMetal_NewFrame((__bridge MTLRenderPassDescriptor*)render_pass_descriptor); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_metal_render_draw_data(ImDrawData* draw_data, void* command_buffer, void* command_encoder) noexcept {
    @try {
        ImGui_ImplMetal_RenderDrawData(draw_data, (__bridge id<MTLCommandBuffer>)command_buffer,
                                     (__bridge id<MTLRenderCommandEncoder>)command_encoder);
    }
    @catch (...) { std::abort(); }
}
bool dear_imgui_backend_metal_create_device_objects(void* device) noexcept {
    @try { return ImGui_ImplMetal_CreateDeviceObjects((__bridge id<MTLDevice>)device); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_metal_destroy_device_objects() noexcept {
    @try { ImGui_ImplMetal_DestroyDeviceObjects(); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_metal_update_texture(ImTextureData* texture) noexcept {
    @try { ImGui_ImplMetal_UpdateTexture(texture); }
    @catch (...) { std::abort(); }
}
} // extern "C"
