#include "imgui.h"
#include "backends/imgui_impl_android.h"

extern "C" {

bool dear_imgui_backend_android_init(void* window) {
    return ImGui_ImplAndroid_Init(static_cast<ANativeWindow*>(window));
}

int32_t dear_imgui_backend_android_handle_input_event(const void* input_event) {
    return ImGui_ImplAndroid_HandleInputEvent(static_cast<const AInputEvent*>(input_event));
}

void dear_imgui_backend_android_shutdown() {
    ImGui_ImplAndroid_Shutdown();
}

void dear_imgui_backend_android_new_frame() {
    ImGui_ImplAndroid_NewFrame();
}

} // extern "C"
