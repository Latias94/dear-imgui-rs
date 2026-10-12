#import <Cocoa/Cocoa.h>
#include "imgui.h"
#include "backends/imgui_impl_osx.h"

extern "C" {
bool dear_imgui_backend_osx_init(void* view) { return ImGui_ImplOSX_Init((__bridge NSView*)view); }
void dear_imgui_backend_osx_shutdown() { ImGui_ImplOSX_Shutdown(); }
void dear_imgui_backend_osx_new_frame(void* view) { ImGui_ImplOSX_NewFrame((__bridge NSView*)view); }
} // extern "C"
