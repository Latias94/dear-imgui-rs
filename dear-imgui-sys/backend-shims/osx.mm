#import <Cocoa/Cocoa.h>
#include <cstdlib>
#include "imgui.h"
#include "backends/imgui_impl_osx.h"

extern "C" {
bool dear_imgui_backend_osx_init(void* view) noexcept {
    @try { return ImGui_ImplOSX_Init((__bridge NSView*)view); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_osx_shutdown() noexcept {
    @try { ImGui_ImplOSX_Shutdown(); }
    @catch (...) { std::abort(); }
}
void dear_imgui_backend_osx_new_frame(void* view) noexcept {
    @try { ImGui_ImplOSX_NewFrame((__bridge NSView*)view); }
    @catch (...) { std::abort(); }
}
} // extern "C"
