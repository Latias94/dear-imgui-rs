// Thin C wrappers around Dear ImGui SDL3 + OpenGL3 backends.
//
// This compiles against the upstream imgui sources provided by dear-imgui-sys
// and the SDL3 headers found via SDL3_INCLUDE_DIR or pkg-config.

#include "imgui.h"
#include "backends/imgui_impl_sdl3.h"
#ifdef DEAR_IMGUI_SDL3_OPENGL3_RENDERER
#include "backends/imgui_impl_opengl3.h"
#endif

#include <SDL3/SDL.h>

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

#ifdef DEAR_IMGUI_SDL3_OPENGL3_RENDERER
	bool ImGui_ImplOpenGL3_Init_Rust(const char* glsl_version) {
	    return ImGui_ImplOpenGL3_Init(glsl_version);
	}

bool ImGui_ImplOpenGL3_CreateDeviceObjects_Rust() {
    return ImGui_ImplOpenGL3_CreateDeviceObjects();
}

void ImGui_ImplOpenGL3_DestroyDeviceObjects_Rust() {
    ImGui_ImplOpenGL3_DestroyDeviceObjects();
}

void ImGui_ImplOpenGL3_Shutdown_Rust() {
    ImGui_ImplOpenGL3_Shutdown();
}

void ImGui_ImplOpenGL3_NewFrame_Rust() {
    ImGui_ImplOpenGL3_NewFrame();
}

void ImGui_ImplOpenGL3_RenderDrawData_Rust(ImDrawData* draw_data) {
    ImGui_ImplOpenGL3_RenderDrawData(draw_data);
}

	void ImGui_ImplOpenGL3_UpdateTexture_Rust(ImTextureData* tex) {
	    ImGui_ImplOpenGL3_UpdateTexture(tex);
	}
#endif

	void ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust() {
	    ImGui_ImplSDL3_SetGamepadMode(ImGui_ImplSDL3_GamepadMode_AutoFirst, nullptr, 0);
	}

void ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust() {
    ImGui_ImplSDL3_SetGamepadMode(ImGui_ImplSDL3_GamepadMode_AutoAll, nullptr, 0);
}

void ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(SDL_Gamepad** manual_gamepads_array, int manual_gamepads_count) {
    ImGui_ImplSDL3_SetGamepadMode(ImGui_ImplSDL3_GamepadMode_Manual, manual_gamepads_array, manual_gamepads_count);
}

} // extern "C"
