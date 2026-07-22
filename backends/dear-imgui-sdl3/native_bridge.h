#pragma once

#include <SDL3/SDL.h>

#include <cstdint>

struct ImGuiViewport;

enum DearImguiSdl3NativePhase : std::uint32_t {
    DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE = 1,
    DEAR_IMGUI_SDL3_PHASE_PLATFORM_RENDER = 2,
    DEAR_IMGUI_SDL3_PHASE_PLATFORM_SWAP = 3,
    DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE = 4,
};

enum DearImguiSdl3NativeFault : std::uint64_t {
    DEAR_IMGUI_SDL3_FAULT_GL_SHARE_CAPTURE = UINT64_C(1) << 0,
    DEAR_IMGUI_SDL3_FAULT_GL_SHARE_SET = UINT64_C(1) << 1,
    DEAR_IMGUI_SDL3_FAULT_GL_MAIN_CONTEXT = UINT64_C(1) << 2,
    DEAR_IMGUI_SDL3_FAULT_GL_MAIN_SWAP_INTERVAL = UINT64_C(1) << 3,
    DEAR_IMGUI_SDL3_FAULT_GL_CREATE_CONTEXT = UINT64_C(1) << 4,
    DEAR_IMGUI_SDL3_FAULT_GL_SET_SWAP_INTERVAL = UINT64_C(1) << 5,
    DEAR_IMGUI_SDL3_FAULT_GL_RESTORE_CONTEXT = UINT64_C(1) << 6,
    DEAR_IMGUI_SDL3_FAULT_GL_RESTORE_SHARE = UINT64_C(1) << 7,
    DEAR_IMGUI_SDL3_FAULT_GL_RENDER_CONTEXT = UINT64_C(1) << 8,
    DEAR_IMGUI_SDL3_FAULT_GL_SWAP_CONTEXT = UINT64_C(1) << 9,
    DEAR_IMGUI_SDL3_FAULT_GL_SWAP_WINDOW = UINT64_C(1) << 10,
    DEAR_IMGUI_SDL3_FAULT_SDLGPU_CLAIM = UINT64_C(1) << 11,
    DEAR_IMGUI_SDL3_FAULT_SDLGPU_CONFIGURE = UINT64_C(1) << 12,
    DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL = UINT64_C(1) << 13,
    DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER = UINT64_C(1) << 14,
    DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN = UINT64_C(1) << 15,
    DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS = UINT64_C(1) << 16,
    DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT = UINT64_C(1) << 17,
};

extern "C" {

std::uint64_t dear_imgui_sdl3_native_begin(
    std::uint32_t phase,
    std::uint32_t expects_opengl,
    std::uint32_t swap_interval_policy,
    std::int32_t explicit_swap_interval,
    ImGuiViewport* viewport
);

std::uint64_t dear_imgui_sdl3_native_end();

bool SDLCALL dear_imgui_sdl3_hook_gl_set_attribute(SDL_GLAttr attribute, int value);
SDL_GLContext SDLCALL dear_imgui_sdl3_hook_gl_create_context(SDL_Window* window);
bool SDLCALL dear_imgui_sdl3_hook_gl_make_current(SDL_Window* window, SDL_GLContext context);
bool SDLCALL dear_imgui_sdl3_hook_gl_set_swap_interval(int interval);
bool SDLCALL dear_imgui_sdl3_hook_gl_swap_window(SDL_Window* window);

bool SDLCALL dear_imgui_sdl3_hook_claim_window_for_gpu_device(
    SDL_GPUDevice* device,
    SDL_Window* window
);

bool SDLCALL dear_imgui_sdl3_hook_set_gpu_swapchain_parameters(
    SDL_GPUDevice* device,
    SDL_Window* window,
    SDL_GPUSwapchainComposition swapchain_composition,
    SDL_GPUPresentMode present_mode
);

std::uint64_t dear_imgui_sdl3_backend_sdlgpu3_render_viewport(ImGuiViewport* viewport);
#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
std::uint64_t dear_imgui_sdl3_backend_sdlgpu3_render_contract_self_test();
#endif

} // extern "C"
