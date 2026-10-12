#import <Metal/Metal.h>
#include "imgui.h"
#include "backends/imgui_impl_metal.h"

extern "C" {
bool dear_imgui_backend_metal_init(void* device) {
    return ImGui_ImplMetal_Init((__bridge id<MTLDevice>)device);
}
void dear_imgui_backend_metal_shutdown() { ImGui_ImplMetal_Shutdown(); }
void dear_imgui_backend_metal_new_frame(void* render_pass_descriptor) {
    ImGui_ImplMetal_NewFrame((__bridge MTLRenderPassDescriptor*)render_pass_descriptor);
}
void dear_imgui_backend_metal_render_draw_data(ImDrawData* draw_data, void* command_buffer, void* command_encoder) {
    ImGui_ImplMetal_RenderDrawData(draw_data, (__bridge id<MTLCommandBuffer>)command_buffer,
                                 (__bridge id<MTLRenderCommandEncoder>)command_encoder);
}
bool dear_imgui_backend_metal_create_device_objects(void* device) {
    return ImGui_ImplMetal_CreateDeviceObjects((__bridge id<MTLDevice>)device);
}
void dear_imgui_backend_metal_destroy_device_objects() { ImGui_ImplMetal_DestroyDeviceObjects(); }
void dear_imgui_backend_metal_update_texture(ImTextureData* texture) { ImGui_ImplMetal_UpdateTexture(texture); }
} // extern "C"
