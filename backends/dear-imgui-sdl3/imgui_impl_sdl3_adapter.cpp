#include <SDL3/SDL.h>

#include "backends/imgui_impl_sdl3.h"
#include "native_bridge.h"

#include <climits>

namespace {
thread_local bool dear_imgui_sdl3_suppress_upstream_mouse_leave_check = false;

bool dear_imgui_sdl3_mouse_leave_due(int pending_frame, int current_frame, int buttons_down) {
    return pending_frame != 0 && pending_frame <= current_frame && buttons_down == 0;
}
} // namespace

namespace ImGui {
int DearImguiSdl3FrameCountForBackend() {
    return dear_imgui_sdl3_suppress_upstream_mouse_leave_check ? INT_MAX : ImGui::GetFrameCount();
}
} // namespace ImGui

// SDL declarations must be parsed before these call-site substitutions. Only the vendored backend
// implementation is intercepted; application and SDL symbols retain their normal ABI.
#define SDL_GL_SetAttribute dear_imgui_sdl3_hook_gl_set_attribute
#define SDL_GL_CreateContext dear_imgui_sdl3_hook_gl_create_context
#define SDL_GL_MakeCurrent dear_imgui_sdl3_hook_gl_make_current
#define SDL_GL_SetSwapInterval dear_imgui_sdl3_hook_gl_set_swap_interval
#define SDL_GL_SwapWindow dear_imgui_sdl3_hook_gl_swap_window
#define ImGui_ImplSDL3_NewFrame ImGui_ImplSDL3_NewFrame_Upstream
#define GetFrameCount() DearImguiSdl3FrameCountForBackend()

#include "backends/imgui_impl_sdl3.cpp"

#undef GetFrameCount
#undef ImGui_ImplSDL3_NewFrame
#undef SDL_GL_SwapWindow
#undef SDL_GL_SetSwapInterval
#undef SDL_GL_MakeCurrent
#undef SDL_GL_CreateContext
#undef SDL_GL_SetAttribute

void ImGui_ImplSDL3_NewFrame() {
    dear_imgui_sdl3_suppress_upstream_mouse_leave_check = true;
    ImGui_ImplSDL3_NewFrame_Upstream();
    dear_imgui_sdl3_suppress_upstream_mouse_leave_check = false;

    ImGui_ImplSDL3_Data* data = ImGui_ImplSDL3_GetBackendData();
    if (data != nullptr && dear_imgui_sdl3_mouse_leave_due(
            data->MousePendingLeaveFrame,
            ImGui::GetFrameCount(),
            data->MouseButtonsDown)) {
        data->MouseWindowID = 0;
        data->MousePendingLeaveFrame = 0;
        ImGui::GetIO().AddMousePosEvent(-FLT_MAX, -FLT_MAX);
    }
}

extern "C" bool dear_imgui_sdl3_mouse_leave_due_for_test(
    int pending_frame,
    int current_frame,
    int buttons_down
) {
    return dear_imgui_sdl3_mouse_leave_due(pending_frame, current_frame, buttons_down);
}
