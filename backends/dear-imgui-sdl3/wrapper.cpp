// Thin C wrappers around the Dear ImGui SDL3 backend.
//
// This compiles against the upstream imgui sources provided by dear-imgui-sys
// and the SDL3 headers found via SDL3_INCLUDE_DIR or pkg-config.

#include "imgui.h"
#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
#include "imgui_internal.h"
#endif
#include "native_bridge.h"
#include "backends/imgui_impl_sdl3.h"
#if defined(DEAR_IMGUI_SDL3_ENABLE_SDLRENDERER3)
#include "backends/imgui_impl_sdlrenderer3.h"
#endif
#if defined(DEAR_IMGUI_SDL3_ENABLE_SDLGPU3)
#include "backends/imgui_impl_sdlgpu3.h"
#endif

#include <SDL3/SDL.h>
#include <cstddef>
#include <cstdint>
#include <vector>

namespace {

struct DearImguiSdl3NativeOps {
    decltype(&SDL_GetWindowFromID) get_window_from_id;
    decltype(&SDL_GL_GetCurrentWindow) gl_get_current_window;
    decltype(&SDL_GL_GetCurrentContext) gl_get_current_context;
    decltype(&SDL_GL_GetAttribute) gl_get_attribute;
    decltype(&SDL_GL_SetAttribute) gl_set_attribute;
    decltype(&SDL_GL_CreateContext) gl_create_context;
    decltype(&SDL_GL_MakeCurrent) gl_make_current;
    decltype(&SDL_GL_GetSwapInterval) gl_get_swap_interval;
    decltype(&SDL_GL_SetSwapInterval) gl_set_swap_interval;
    decltype(&SDL_GL_SwapWindow) gl_swap_window;
    decltype(&SDL_ClaimWindowForGPUDevice) claim_window_for_gpu_device;
    decltype(&SDL_SetGPUSwapchainParameters) set_gpu_swapchain_parameters;
    decltype(&SDL_ReleaseWindowFromGPUDevice) release_window_from_gpu_device;
};

const DearImguiSdl3NativeOps real_native_ops = {
    SDL_GetWindowFromID,
    SDL_GL_GetCurrentWindow,
    SDL_GL_GetCurrentContext,
    SDL_GL_GetAttribute,
    SDL_GL_SetAttribute,
    SDL_GL_CreateContext,
    SDL_GL_MakeCurrent,
    SDL_GL_GetSwapInterval,
    SDL_GL_SetSwapInterval,
    SDL_GL_SwapWindow,
    SDL_ClaimWindowForGPUDevice,
    SDL_SetGPUSwapchainParameters,
    SDL_ReleaseWindowFromGPUDevice,
};

thread_local const DearImguiSdl3NativeOps* native_ops = &real_native_ops;

struct DearImguiSdl3NativeTransaction {
    bool active = false;
    DearImguiSdl3NativePhase phase = DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE;
    bool expects_opengl = false;
    bool match_main_swap_interval = false;
    int explicit_swap_interval = 0;
    int resolved_swap_interval = 0;
    bool swap_interval_resolved = false;
    std::uint64_t faults = 0;
    SDL_Window* expected_window = nullptr;

    SDL_Window* previous_window = nullptr;
    SDL_GLContext previous_context = nullptr;
    int previous_share_attribute = 0;
    bool share_attribute_changed = false;

    SDL_GLContext main_context = nullptr;

    SDL_GLContext created_context = nullptr;
    unsigned int make_current_calls = 0;
    bool phase_make_current_succeeded = false;
    bool phase_swap_succeeded = false;

    SDL_GPUDevice* claimed_device = nullptr;
    SDL_Window* claimed_window = nullptr;
    bool gpu_claimed = false;
    bool gpu_configured = false;
};

thread_local DearImguiSdl3NativeTransaction native_transaction;

SDL_Window* dear_imgui_sdl3_window_from_viewport(ImGuiViewport* viewport) {
    if (viewport == nullptr || viewport->PlatformHandle == nullptr)
        return nullptr;
    SDL_WindowID window_id = (SDL_WindowID)(intptr_t)viewport->PlatformHandle;
    return native_ops->get_window_from_id(window_id);
}

void dear_imgui_sdl3_restore_create_state(DearImguiSdl3NativeTransaction& state) {
    if (state.share_attribute_changed && !native_ops->gl_set_attribute(
            SDL_GL_SHARE_WITH_CURRENT_CONTEXT,
            state.previous_share_attribute))
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_RESTORE_SHARE;

    if (native_ops->gl_get_current_window() != state.previous_window
        || native_ops->gl_get_current_context() != state.previous_context) {
        if (!native_ops->gl_make_current(state.previous_window, state.previous_context))
            state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_RESTORE_CONTEXT;
    }
}

void dear_imgui_sdl3_detach_failed_render_context(DearImguiSdl3NativeTransaction& state) {
    if (!native_ops->gl_make_current(nullptr, nullptr))
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_RESTORE_CONTEXT;
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
struct DearImguiSdl3FakeNativeState {
    SDL_Window* current_window = reinterpret_cast<SDL_Window*>(static_cast<std::uintptr_t>(0x101));
    SDL_GLContext current_context = reinterpret_cast<SDL_GLContext>(static_cast<std::uintptr_t>(0x102));
    SDL_Window* viewport_window = reinterpret_cast<SDL_Window*>(static_cast<std::uintptr_t>(0x201));
    SDL_GLContext created_context = reinterpret_cast<SDL_GLContext>(static_cast<std::uintptr_t>(0x202));
    SDL_GPUDevice* gpu_device = reinterpret_cast<SDL_GPUDevice*>(static_cast<std::uintptr_t>(0x301));
    int share_attribute = 0;
    int swap_interval = 1;
    int operation_index = 0;
    int main_make_current_index = 0;
    int get_swap_interval_index = 0;
    int create_context_index = 0;
    int make_current_calls = 0;
    int fail_make_current_call = 0;
    int set_swap_interval_calls = 0;
    int swap_window_calls = 0;
    int gpu_claim_calls = 0;
    int gpu_configure_calls = 0;
    int gpu_mailbox_configure_calls = 0;
    int gpu_release_calls = 0;
    SDL_GLContext set_swap_interval_context = nullptr;
    SDL_GPUPresentMode last_gpu_present_mode = SDL_GPU_PRESENTMODE_VSYNC;
    bool fail_get_attribute = false;
    bool fail_set_attribute = false;
    bool fail_get_swap_interval = false;
    bool fail_create_context = false;
    bool fail_set_swap_interval = false;
    bool fail_swap_window = false;
    bool fail_gpu_claim = false;
    bool fail_gpu_configure = false;
    bool fail_gpu_mailbox_configure = false;
};

thread_local DearImguiSdl3FakeNativeState fake_native_state;

SDL_Window* SDLCALL fake_get_window_from_id(SDL_WindowID) {
    return fake_native_state.viewport_window;
}

SDL_Window* SDLCALL fake_gl_get_current_window() {
    return fake_native_state.current_window;
}

SDL_GLContext SDLCALL fake_gl_get_current_context() {
    return fake_native_state.current_context;
}

bool SDLCALL fake_gl_get_attribute(SDL_GLAttr, int* value) {
    if (fake_native_state.fail_get_attribute)
        return false;
    *value = fake_native_state.share_attribute;
    return true;
}

bool SDLCALL fake_gl_set_attribute(SDL_GLAttr, int value) {
    if (fake_native_state.fail_set_attribute)
        return false;
    fake_native_state.share_attribute = value;
    return true;
}

SDL_GLContext SDLCALL fake_gl_create_context(SDL_Window* window) {
    fake_native_state.create_context_index = ++fake_native_state.operation_index;
    if (fake_native_state.fail_create_context)
        return nullptr;
    fake_native_state.current_window = window;
    fake_native_state.current_context = fake_native_state.created_context;
    return fake_native_state.created_context;
}

bool SDLCALL fake_gl_make_current(SDL_Window* window, SDL_GLContext context) {
    fake_native_state.make_current_calls++;
    int operation = ++fake_native_state.operation_index;
    if (fake_native_state.make_current_calls == 1)
        fake_native_state.main_make_current_index = operation;
    if (fake_native_state.fail_make_current_call == fake_native_state.make_current_calls)
        return false;
    fake_native_state.current_window = window;
    fake_native_state.current_context = context;
    return true;
}

bool SDLCALL fake_gl_get_swap_interval(int* interval) {
    fake_native_state.get_swap_interval_index = ++fake_native_state.operation_index;
    if (fake_native_state.fail_get_swap_interval)
        return false;
    *interval = fake_native_state.swap_interval;
    return true;
}

bool SDLCALL fake_gl_set_swap_interval(int interval) {
    fake_native_state.set_swap_interval_calls++;
    fake_native_state.set_swap_interval_context = fake_native_state.current_context;
    if (fake_native_state.fail_set_swap_interval)
        return false;
    fake_native_state.swap_interval = interval;
    return true;
}

bool SDLCALL fake_gl_swap_window(SDL_Window*) {
    fake_native_state.swap_window_calls++;
    return !fake_native_state.fail_swap_window;
}

bool SDLCALL fake_claim_window_for_gpu_device(SDL_GPUDevice*, SDL_Window*) {
    fake_native_state.gpu_claim_calls++;
    return !fake_native_state.fail_gpu_claim;
}

bool SDLCALL fake_set_gpu_swapchain_parameters(
    SDL_GPUDevice*,
    SDL_Window*,
    SDL_GPUSwapchainComposition,
    SDL_GPUPresentMode present_mode
) {
    fake_native_state.gpu_configure_calls++;
    fake_native_state.last_gpu_present_mode = present_mode;
    if (present_mode == SDL_GPU_PRESENTMODE_MAILBOX) {
        fake_native_state.gpu_mailbox_configure_calls++;
        return !fake_native_state.fail_gpu_mailbox_configure;
    }
    return !fake_native_state.fail_gpu_configure;
}

void SDLCALL fake_release_window_from_gpu_device(SDL_GPUDevice*, SDL_Window*) {
    fake_native_state.gpu_release_calls++;
}

const DearImguiSdl3NativeOps fake_native_ops = {
    fake_get_window_from_id,
    fake_gl_get_current_window,
    fake_gl_get_current_context,
    fake_gl_get_attribute,
    fake_gl_set_attribute,
    fake_gl_create_context,
    fake_gl_make_current,
    fake_gl_get_swap_interval,
    fake_gl_set_swap_interval,
    fake_gl_swap_window,
    fake_claim_window_for_gpu_device,
    fake_set_gpu_swapchain_parameters,
    fake_release_window_from_gpu_device,
};

struct DearImguiSdl3FakeOpsScope {
    DearImguiSdl3FakeOpsScope() {
        native_ops = &fake_native_ops;
        native_transaction = {};
        fake_native_state = DearImguiSdl3FakeNativeState{};
    }

    ~DearImguiSdl3FakeOpsScope() {
        native_transaction = {};
        native_ops = &real_native_ops;
    }
};
#endif

} // namespace

extern "C" {

void dear_imgui_sdl3_backend_clear_platform_monitors() {
    ImGui::GetPlatformIO().Monitors.clear();
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
void dear_imgui_sdl3_destroy_platform_windows_for_test(ImGuiViewportP* viewport) {
    IM_ASSERT(viewport != nullptr);
    ImGuiContext& context = *GImGui;
    context.Viewports.push_back(viewport);
    ImGui::DestroyPlatformWindows();
    IM_ASSERT(context.Viewports.back() == viewport);
    context.Viewports.pop_back();
}
#endif

std::uint64_t dear_imgui_sdl3_native_begin(
    std::uint32_t phase,
    std::uint32_t expects_opengl,
    std::uint32_t swap_interval_policy,
    std::int32_t explicit_swap_interval,
    ImGuiViewport* viewport
) {
    if (native_transaction.active)
        return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;

    native_transaction = {};
    native_transaction.active = true;
    native_transaction.phase = static_cast<DearImguiSdl3NativePhase>(phase);
    native_transaction.expects_opengl = expects_opengl != 0;
    native_transaction.match_main_swap_interval = swap_interval_policy == 1;
    native_transaction.explicit_swap_interval = explicit_swap_interval;
    native_transaction.expected_window = dear_imgui_sdl3_window_from_viewport(viewport);
    if (native_transaction.expects_opengl
        && native_transaction.phase == DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE) {
        native_transaction.previous_window = native_ops->gl_get_current_window();
        native_transaction.previous_context = native_ops->gl_get_current_context();
    }
    return 0;
}

std::uint64_t dear_imgui_sdl3_native_end() {
    if (!native_transaction.active)
        return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;

    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (state.expects_opengl) {
        if (state.phase == DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE) {
            if (state.created_context == nullptr || state.created_context == state.main_context)
                state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_CREATE_CONTEXT;
            dear_imgui_sdl3_restore_create_state(state);
        } else if (state.phase == DEAR_IMGUI_SDL3_PHASE_PLATFORM_RENDER) {
            bool current_matches = state.phase_make_current_succeeded
                && state.expected_window != nullptr
                && native_ops->gl_get_current_window() == state.expected_window
                && native_ops->gl_get_current_context() != nullptr;
            if (!current_matches) {
                state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_RENDER_CONTEXT;
                dear_imgui_sdl3_detach_failed_render_context(state);
            }
        } else if (state.phase == DEAR_IMGUI_SDL3_PHASE_PLATFORM_SWAP) {
            if (!state.phase_make_current_succeeded)
                state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SWAP_CONTEXT;
            if (!state.phase_swap_succeeded)
                state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SWAP_WINDOW;
        }
    }

    if (state.phase == DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE
        && state.gpu_claimed
        && !state.gpu_configured) {
        native_ops->release_window_from_gpu_device(state.claimed_device, state.claimed_window);
        state.gpu_claimed = false;
    }

    std::uint64_t faults = state.faults;
    state = {};
    return faults;
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
std::uint64_t dear_imgui_sdl3_native_contract_self_test() {
    DearImguiSdl3FakeOpsScope scope;
    std::uint64_t failures = 0;
    ImGuiViewport viewport{};
    viewport.PlatformHandle = reinterpret_cast<void*>(static_cast<std::uintptr_t>(0x55));

    SDL_Window* previous_window = fake_native_state.current_window;
    SDL_GLContext previous_context = fake_native_state.current_context;
    SDL_Window* main_window = reinterpret_cast<SDL_Window*>(static_cast<std::uintptr_t>(0x401));
    SDL_GLContext main_context = reinterpret_cast<SDL_GLContext>(static_cast<std::uintptr_t>(0x402));

    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE, 1, 1, 0, &viewport);
    dear_imgui_sdl3_hook_gl_set_attribute(SDL_GL_SHARE_WITH_CURRENT_CONTEXT, 1);
    dear_imgui_sdl3_hook_gl_make_current(main_window, main_context);
    SDL_GLContext created = dear_imgui_sdl3_hook_gl_create_context(fake_native_state.viewport_window);
    dear_imgui_sdl3_hook_gl_set_swap_interval(0);
    dear_imgui_sdl3_hook_gl_make_current(fake_native_state.viewport_window, previous_context);
    std::uint64_t faults = dear_imgui_sdl3_native_end();
    if (faults != 0
        || created != fake_native_state.created_context
        || fake_native_state.current_window != previous_window
        || fake_native_state.current_context != previous_context
        || fake_native_state.share_attribute != 0
        || fake_native_state.set_swap_interval_context != created
        || !(fake_native_state.main_make_current_index
            < fake_native_state.get_swap_interval_index
            && fake_native_state.get_swap_interval_index
                < fake_native_state.create_context_index))
        failures |= UINT64_C(1) << 0;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    previous_window = fake_native_state.current_window;
    previous_context = fake_native_state.current_context;
    fake_native_state.fail_set_swap_interval = true;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE, 1, 0, 0, &viewport);
    dear_imgui_sdl3_hook_gl_set_attribute(SDL_GL_SHARE_WITH_CURRENT_CONTEXT, 1);
    dear_imgui_sdl3_hook_gl_make_current(main_window, main_context);
    created = dear_imgui_sdl3_hook_gl_create_context(fake_native_state.viewport_window);
    bool accepted_interval = dear_imgui_sdl3_hook_gl_set_swap_interval(0);
    dear_imgui_sdl3_hook_gl_make_current(fake_native_state.viewport_window, previous_context);
    faults = dear_imgui_sdl3_native_end();
    if (!accepted_interval
        || faults != 0
        || created != fake_native_state.created_context
        || fake_native_state.set_swap_interval_calls != 1
        || fake_native_state.set_swap_interval_context != created
        || fake_native_state.current_window != previous_window
        || fake_native_state.current_context != previous_context
        || fake_native_state.share_attribute != 0)
        failures |= UINT64_C(1) << 8;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    previous_window = fake_native_state.current_window;
    previous_context = fake_native_state.current_context;
    fake_native_state.fail_get_swap_interval = true;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE, 1, 1, 0, &viewport);
    dear_imgui_sdl3_hook_gl_set_attribute(SDL_GL_SHARE_WITH_CURRENT_CONTEXT, 1);
    dear_imgui_sdl3_hook_gl_make_current(main_window, main_context);
    created = dear_imgui_sdl3_hook_gl_create_context(fake_native_state.viewport_window);
    accepted_interval = dear_imgui_sdl3_hook_gl_set_swap_interval(0);
    dear_imgui_sdl3_hook_gl_make_current(fake_native_state.viewport_window, previous_context);
    faults = dear_imgui_sdl3_native_end();
    if (!accepted_interval
        || faults != 0
        || created != fake_native_state.created_context
        || fake_native_state.get_swap_interval_index == 0
        || fake_native_state.set_swap_interval_calls != 1
        || fake_native_state.swap_interval != 0
        || fake_native_state.current_window != previous_window
        || fake_native_state.current_context != previous_context
        || fake_native_state.share_attribute != 0)
        failures |= UINT64_C(1) << 9;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    previous_window = fake_native_state.current_window;
    previous_context = fake_native_state.current_context;
    fake_native_state.fail_create_context = true;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE, 1, 0, 0, &viewport);
    dear_imgui_sdl3_hook_gl_set_attribute(SDL_GL_SHARE_WITH_CURRENT_CONTEXT, 1);
    dear_imgui_sdl3_hook_gl_make_current(main_window, main_context);
    dear_imgui_sdl3_hook_gl_create_context(fake_native_state.viewport_window);
    dear_imgui_sdl3_hook_gl_set_swap_interval(0);
    dear_imgui_sdl3_hook_gl_make_current(fake_native_state.viewport_window, previous_context);
    faults = dear_imgui_sdl3_native_end();
    if ((faults & DEAR_IMGUI_SDL3_FAULT_GL_CREATE_CONTEXT) == 0
        || fake_native_state.set_swap_interval_calls != 0
        || fake_native_state.get_swap_interval_index != 0
        || fake_native_state.current_window != previous_window
        || fake_native_state.current_context != previous_context
        || fake_native_state.share_attribute != 0)
        failures |= UINT64_C(1) << 1;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    fake_native_state.fail_make_current_call = 1;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_RENDER, 1, 0, 0, &viewport);
    dear_imgui_sdl3_hook_gl_make_current(
        fake_native_state.viewport_window,
        fake_native_state.created_context
    );
    faults = dear_imgui_sdl3_native_end();
    if ((faults & DEAR_IMGUI_SDL3_FAULT_GL_RENDER_CONTEXT) == 0
        || fake_native_state.current_window != nullptr
        || fake_native_state.current_context != nullptr)
        failures |= UINT64_C(1) << 2;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    fake_native_state.fail_make_current_call = 1;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_SWAP, 1, 0, 0, &viewport);
    dear_imgui_sdl3_hook_gl_make_current(
        fake_native_state.viewport_window,
        fake_native_state.created_context
    );
    dear_imgui_sdl3_hook_gl_swap_window(fake_native_state.viewport_window);
    faults = dear_imgui_sdl3_native_end();
    if ((faults & DEAR_IMGUI_SDL3_FAULT_GL_SWAP_CONTEXT) == 0
        || (faults & DEAR_IMGUI_SDL3_FAULT_GL_SWAP_WINDOW) == 0
        || fake_native_state.swap_window_calls != 0)
        failures |= UINT64_C(1) << 3;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    fake_native_state.fail_gpu_claim = true;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE, 0, 0, 0, &viewport);
    dear_imgui_sdl3_hook_claim_window_for_gpu_device(
        fake_native_state.gpu_device,
        fake_native_state.viewport_window
    );
    dear_imgui_sdl3_hook_set_gpu_swapchain_parameters(
        fake_native_state.gpu_device,
        fake_native_state.viewport_window,
        SDL_GPU_SWAPCHAINCOMPOSITION_SDR,
        SDL_GPU_PRESENTMODE_VSYNC
    );
    faults = dear_imgui_sdl3_native_end();
    if ((faults & DEAR_IMGUI_SDL3_FAULT_SDLGPU_CLAIM) == 0
        || fake_native_state.gpu_configure_calls != 0
        || fake_native_state.gpu_release_calls != 0)
        failures |= UINT64_C(1) << 4;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    fake_native_state.fail_gpu_configure = true;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE, 0, 0, 0, &viewport);
    dear_imgui_sdl3_hook_claim_window_for_gpu_device(
        fake_native_state.gpu_device,
        fake_native_state.viewport_window
    );
    dear_imgui_sdl3_hook_set_gpu_swapchain_parameters(
        fake_native_state.gpu_device,
        fake_native_state.viewport_window,
        SDL_GPU_SWAPCHAINCOMPOSITION_SDR,
        SDL_GPU_PRESENTMODE_VSYNC
    );
    faults = dear_imgui_sdl3_native_end();
    if ((faults & DEAR_IMGUI_SDL3_FAULT_SDLGPU_CONFIGURE) == 0
        || fake_native_state.gpu_claim_calls != 1
        || fake_native_state.gpu_configure_calls != 1
        || fake_native_state.gpu_release_calls != 1)
        failures |= UINT64_C(1) << 5;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    fake_native_state.fail_gpu_mailbox_configure = true;
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE, 0, 0, 0, &viewport);
    dear_imgui_sdl3_hook_claim_window_for_gpu_device(
        fake_native_state.gpu_device,
        fake_native_state.viewport_window
    );
    dear_imgui_sdl3_hook_set_gpu_swapchain_parameters(
        fake_native_state.gpu_device,
        fake_native_state.viewport_window,
        SDL_GPU_SWAPCHAINCOMPOSITION_SDR,
        SDL_GPU_PRESENTMODE_MAILBOX
    );
    faults = dear_imgui_sdl3_native_end();
    if (faults != 0
        || fake_native_state.gpu_claim_calls != 1
        || fake_native_state.gpu_configure_calls != 2
        || fake_native_state.gpu_mailbox_configure_calls != 1
        || fake_native_state.last_gpu_present_mode != SDL_GPU_PRESENTMODE_VSYNC
        || fake_native_state.gpu_release_calls != 0)
        failures |= UINT64_C(1) << 8;

    fake_native_state = DearImguiSdl3FakeNativeState{};
    dear_imgui_sdl3_native_begin(DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE, 0, 0, 0, &viewport);
    faults = dear_imgui_sdl3_native_begin(
        DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE,
        0,
        0,
        0,
        &viewport
    );
    if ((faults & DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL) == 0)
        failures |= UINT64_C(1) << 6;
    dear_imgui_sdl3_native_end();

#if defined(DEAR_IMGUI_SDL3_ENABLE_SDLGPU3)
    if (dear_imgui_sdl3_backend_sdlgpu3_render_contract_self_test() != 0)
        failures |= UINT64_C(1) << 7;
#endif
    return failures;
}
#endif

bool SDLCALL dear_imgui_sdl3_hook_gl_set_attribute(SDL_GLAttr attribute, int value) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active
        || !state.expects_opengl
        || state.phase != DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE
        || attribute != SDL_GL_SHARE_WITH_CURRENT_CONTEXT)
        return native_ops->gl_set_attribute(attribute, value);

    if (!native_ops->gl_get_attribute(attribute, &state.previous_share_attribute)) {
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SHARE_CAPTURE;
        return false;
    }
    if (!native_ops->gl_set_attribute(attribute, value)) {
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SHARE_SET;
        return false;
    }
    state.share_attribute_changed = true;
    return true;
}

SDL_GLContext SDLCALL dear_imgui_sdl3_hook_gl_create_context(SDL_Window* window) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active
        || !state.expects_opengl
        || state.phase != DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE)
        return native_ops->gl_create_context(window);

    if (state.faults != 0)
        return nullptr;
    state.created_context = native_ops->gl_create_context(window);
    if (state.created_context == nullptr || state.created_context == state.main_context)
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_CREATE_CONTEXT;
    return state.created_context;
}

bool SDLCALL dear_imgui_sdl3_hook_gl_make_current(SDL_Window* window, SDL_GLContext context) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active || !state.expects_opengl)
        return native_ops->gl_make_current(window, context);

    if (state.phase == DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE) {
        state.make_current_calls++;
        if (state.make_current_calls == 1) {
            state.main_context = context;
            if (!native_ops->gl_make_current(window, context)) {
                state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_MAIN_CONTEXT;
                return false;
            }
            if (state.match_main_swap_interval) {
                // Present timing is a preference, not a prerequisite for a valid shared GL
                // context. Drivers such as Xvfb/llvmpipe can reject this query; fall back to
                // their default timing instead of abandoning a safely-created viewport.
                if (!native_ops->gl_get_swap_interval(&state.resolved_swap_interval))
                    state.resolved_swap_interval = 0;
                state.swap_interval_resolved = true;
            } else {
                state.resolved_swap_interval = state.explicit_swap_interval;
                state.swap_interval_resolved = true;
            }
            return true;
        }
        return native_ops->gl_make_current(state.previous_window, state.previous_context);
    }

    bool succeeded = native_ops->gl_make_current(window, context);
    state.phase_make_current_succeeded = succeeded
        && window == state.expected_window
        && native_ops->gl_get_current_window() == state.expected_window
        && native_ops->gl_get_current_context() == context
        && context != nullptr;
    if (!state.phase_make_current_succeeded) {
        state.faults |= state.phase == DEAR_IMGUI_SDL3_PHASE_PLATFORM_RENDER
            ? DEAR_IMGUI_SDL3_FAULT_GL_RENDER_CONTEXT
            : DEAR_IMGUI_SDL3_FAULT_GL_SWAP_CONTEXT;
    }
    return state.phase_make_current_succeeded;
}

bool SDLCALL dear_imgui_sdl3_hook_gl_set_swap_interval(int interval) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active
        || !state.expects_opengl
        || state.phase != DEAR_IMGUI_SDL3_PHASE_PLATFORM_CREATE)
        return native_ops->gl_set_swap_interval(interval);

    (void)interval;

    if (state.created_context == nullptr
        || !state.swap_interval_resolved
        || native_ops->gl_get_current_context() != state.created_context) {
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SET_SWAP_INTERVAL;
        return false;
    }

    // SDL documents this as an optional presentation setting. A driver may refuse a valid
    // interval after the context exists, so retain the usable context and let the driver choose
    // its default rather than treating an unsupported VSync preference as a creation failure.
    (void)native_ops->gl_set_swap_interval(state.resolved_swap_interval);
    return true;
}

bool SDLCALL dear_imgui_sdl3_hook_gl_swap_window(SDL_Window* window) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active
        || !state.expects_opengl
        || state.phase != DEAR_IMGUI_SDL3_PHASE_PLATFORM_SWAP)
        return native_ops->gl_swap_window(window);

    if (!state.phase_make_current_succeeded || window != state.expected_window) {
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SWAP_WINDOW;
        return false;
    }
    state.phase_swap_succeeded = native_ops->gl_swap_window(window);
    if (!state.phase_swap_succeeded)
        state.faults |= DEAR_IMGUI_SDL3_FAULT_GL_SWAP_WINDOW;
    return state.phase_swap_succeeded;
}

bool SDLCALL dear_imgui_sdl3_hook_claim_window_for_gpu_device(
    SDL_GPUDevice* device,
    SDL_Window* window
) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active || state.phase != DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE)
        return native_ops->claim_window_for_gpu_device(device, window);

    state.claimed_device = device;
    state.claimed_window = window;
    state.gpu_claimed = native_ops->claim_window_for_gpu_device(device, window);
    if (!state.gpu_claimed)
        state.faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_CLAIM;
    return state.gpu_claimed;
}

bool SDLCALL dear_imgui_sdl3_hook_set_gpu_swapchain_parameters(
    SDL_GPUDevice* device,
    SDL_Window* window,
    SDL_GPUSwapchainComposition swapchain_composition,
    SDL_GPUPresentMode present_mode
) {
    DearImguiSdl3NativeTransaction& state = native_transaction;
    if (!state.active || state.phase != DEAR_IMGUI_SDL3_PHASE_SDLGPU_CREATE)
        return native_ops->set_gpu_swapchain_parameters(
            device,
            window,
            swapchain_composition,
            present_mode
        );

    if (!state.gpu_claimed)
        return false;
    state.gpu_configured = native_ops->set_gpu_swapchain_parameters(
        device,
        window,
        swapchain_composition,
        present_mode
    );
    // Mailbox support is window-specific. A secondary viewport may land on a display where the
    // low-latency mode selected for the main window is unavailable.
    if (!state.gpu_configured && present_mode == SDL_GPU_PRESENTMODE_MAILBOX) {
        state.gpu_configured = native_ops->set_gpu_swapchain_parameters(
            device,
            window,
            swapchain_composition,
            SDL_GPU_PRESENTMODE_VSYNC
        );
    }
    if (!state.gpu_configured)
        state.faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_CONFIGURE;
    return state.gpu_configured;
}

std::size_t dear_imgui_sdl3_backend_sizeof_imgui_io() {
    return sizeof(ImGuiIO);
}

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

void dear_imgui_sdl3_backend_set_texture_updates(
    ImTextureData* texture,
    const ImTextureRect* updates,
    int update_count
) {
    IM_ASSERT(texture != nullptr);
    IM_ASSERT(update_count >= 0);
    texture->Updates.resize(update_count);
    for (int index = 0; index < update_count; index++)
        texture->Updates[index] = updates[index];
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

#if defined(DEAR_IMGUI_SDL3_ENABLE_SDLRENDERER3)
bool dear_imgui_sdl3_backend_sdlrenderer3_init(SDL_Renderer* renderer) {
    return ImGui_ImplSDLRenderer3_Init(renderer);
}

void dear_imgui_sdl3_backend_sdlrenderer3_shutdown() {
    ImGui_ImplSDLRenderer3_Shutdown();
}

void dear_imgui_sdl3_backend_sdlrenderer3_new_frame() {
    ImGui_ImplSDLRenderer3_NewFrame();
}

void dear_imgui_sdl3_backend_sdlrenderer3_render_draw_data(ImDrawData* draw_data, SDL_Renderer* renderer) {
    ImGui_ImplSDLRenderer3_RenderDrawData(draw_data, renderer);
}

void dear_imgui_sdl3_backend_sdlrenderer3_create_device_objects() {
    ImGui_ImplSDLRenderer3_CreateDeviceObjects();
}

void dear_imgui_sdl3_backend_sdlrenderer3_destroy_device_objects() {
    ImGui_ImplSDLRenderer3_DestroyDeviceObjects();
}

void dear_imgui_sdl3_backend_sdlrenderer3_update_texture(ImTextureData* tex) {
    ImGui_ImplSDLRenderer3_UpdateTexture(tex);
}
#endif

#if defined(DEAR_IMGUI_SDL3_ENABLE_SDLGPU3)
bool dear_imgui_sdl3_backend_sdlgpu3_init(ImGui_ImplSDLGPU3_InitInfo* info) {
    return ImGui_ImplSDLGPU3_Init(info);
}

void dear_imgui_sdl3_backend_sdlgpu3_shutdown() {
    ImGui_ImplSDLGPU3_Shutdown();
}

void dear_imgui_sdl3_backend_sdlgpu3_new_frame() {
    ImGui_ImplSDLGPU3_NewFrame();
}

void dear_imgui_sdl3_backend_sdlgpu3_prepare_draw_data(ImDrawData* draw_data, SDL_GPUCommandBuffer* buffer) {
    ImGui_ImplSDLGPU3_PrepareDrawData(draw_data, buffer);
}

void dear_imgui_sdl3_backend_sdlgpu3_render_draw_data(
    ImDrawData* draw_data,
    SDL_GPUCommandBuffer* buffer,
    SDL_GPURenderPass* render_pass,
    SDL_GPUGraphicsPipeline* pipeline
) {
    ImGui_ImplSDLGPU3_RenderDrawData(draw_data, buffer, render_pass, pipeline);
}

void dear_imgui_sdl3_backend_sdlgpu3_create_device_objects() {
    ImGui_ImplSDLGPU3_CreateDeviceObjects();
}

void dear_imgui_sdl3_backend_sdlgpu3_destroy_device_objects() {
    ImGui_ImplSDLGPU3_DestroyDeviceObjects();
}

void dear_imgui_sdl3_backend_sdlgpu3_update_texture(ImTextureData* tex) {
    ImGui_ImplSDLGPU3_UpdateTexture(tex);
}
#endif

} // extern "C"
