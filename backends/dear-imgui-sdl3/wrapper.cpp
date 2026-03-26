// Thin C wrappers around the Dear ImGui SDL3 backend.
//
// This compiles against the upstream imgui sources provided by dear-imgui-sys
// and the SDL3 headers found via SDL3_INCLUDE_DIR or pkg-config.

#include "imgui.h"
#include "backends/imgui_impl_sdl3.h"

#include <SDL3/SDL.h>
#include <vector>

extern "C" {

bool ImGui_ImplSDL3_InitForOpenGL_Rust(SDL_Window* window, void* sdl_gl_context) {
    return ImGui_ImplSDL3_InitForOpenGL(window, sdl_gl_context);
}

bool ImGui_ImplSDL3_InitForVulkan_Rust(SDL_Window* window) {
    return ImGui_ImplSDL3_InitForVulkan(window);
}

bool ImGui_ImplSDL3_InitForD3D_Rust(SDL_Window* window) {
    return ImGui_ImplSDL3_InitForD3D(window);
}

bool ImGui_ImplSDL3_InitForMetal_Rust(SDL_Window* window) {
    return ImGui_ImplSDL3_InitForMetal(window);
}

bool ImGui_ImplSDL3_InitForSDLRenderer_Rust(SDL_Window* window, SDL_Renderer* renderer) {
    return ImGui_ImplSDL3_InitForSDLRenderer(window, renderer);
}

bool ImGui_ImplSDL3_InitForSDLGPU_Rust(SDL_Window* window) {
    return ImGui_ImplSDL3_InitForSDLGPU(window);
}

bool ImGui_ImplSDL3_InitForOther_Rust(SDL_Window* window) {
    return ImGui_ImplSDL3_InitForOther(window);
}

void ImGui_ImplSDL3_Shutdown_Rust() {
    ImGui_ImplSDL3_Shutdown();
}

void ImGui_ImplSDL3_NewFrame_Rust() {
    ImGui_ImplSDL3_NewFrame();
}

bool ImGui_ImplSDL3_ProcessEvent_Rust(const SDL_Event* event) {
    return ImGui_ImplSDL3_ProcessEvent(event);
}

void ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust() {
    ImGui_ImplSDL3_SetGamepadMode(ImGui_ImplSDL3_GamepadMode_AutoFirst, nullptr, 0);
}

void ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust() {
    ImGui_ImplSDL3_SetGamepadMode(ImGui_ImplSDL3_GamepadMode_AutoAll, nullptr, 0);
}

void ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(SDL_Gamepad* const* manual_gamepads_array, int manual_gamepads_count) {
    // Dear ImGui SDL backends may keep a pointer to the passed-in array. Copy it into stable
    // storage so Rust callers don't need to keep their slice buffer alive.
    static std::vector<SDL_Gamepad*> manual_gamepads;
    manual_gamepads.clear();
    if (manual_gamepads_array != nullptr && manual_gamepads_count > 0) {
        manual_gamepads.assign(manual_gamepads_array, manual_gamepads_array + manual_gamepads_count);
    }
    ImGui_ImplSDL3_SetGamepadMode(
        ImGui_ImplSDL3_GamepadMode_Manual,
        manual_gamepads.empty() ? nullptr : manual_gamepads.data(),
        (int)manual_gamepads.size()
    );
}

} // extern "C"
