#include <SDL3/SDL.h>

#include "backends/imgui_impl_sdlgpu3.h"
#include "native_bridge.h"

#define SDL_ClaimWindowForGPUDevice dear_imgui_sdl3_hook_claim_window_for_gpu_device
#define SDL_SetGPUSwapchainParameters dear_imgui_sdl3_hook_set_gpu_swapchain_parameters

#include "backends/imgui_impl_sdlgpu3.cpp"

#undef SDL_SetGPUSwapchainParameters
#undef SDL_ClaimWindowForGPUDevice

namespace {

struct DearImguiSdlGpu3RenderOps {
    decltype(&SDL_AcquireGPUCommandBuffer) acquire_command_buffer;
    decltype(&SDL_AcquireGPUSwapchainTexture) acquire_swapchain_texture;
    decltype(&SDL_BeginGPURenderPass) begin_render_pass;
    decltype(&SDL_EndGPURenderPass) end_render_pass;
    decltype(&SDL_SubmitGPUCommandBuffer) submit_command_buffer;
    decltype(&SDL_CancelGPUCommandBuffer) cancel_command_buffer;
    void (*prepare_draw_data)(ImDrawData*, SDL_GPUCommandBuffer*);
    void (*render_draw_data)(
        ImDrawData*,
        SDL_GPUCommandBuffer*,
        SDL_GPURenderPass*,
        SDL_GPUGraphicsPipeline*
    );
};

const DearImguiSdlGpu3RenderOps real_render_ops = {
    SDL_AcquireGPUCommandBuffer,
    SDL_AcquireGPUSwapchainTexture,
    SDL_BeginGPURenderPass,
    SDL_EndGPURenderPass,
    SDL_SubmitGPUCommandBuffer,
    SDL_CancelGPUCommandBuffer,
    ImGui_ImplSDLGPU3_PrepareDrawData,
    ImGui_ImplSDLGPU3_RenderDrawData,
};

std::uint64_t dear_imgui_sdlgpu3_render_checked(
    const DearImguiSdlGpu3RenderOps& ops,
    SDL_GPUDevice* device,
    SDL_Window* window,
    ImDrawData* draw_data
) {
    if (device == nullptr || window == nullptr || draw_data == nullptr)
        return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;

    SDL_GPUCommandBuffer* command_buffer = ops.acquire_command_buffer(device);
    if (command_buffer == nullptr)
        return DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER;

    SDL_GPUTexture* swapchain_texture = nullptr;
    if (!ops.acquire_swapchain_texture(
            command_buffer,
            window,
            &swapchain_texture,
            nullptr,
            nullptr)) {
        ops.cancel_command_buffer(command_buffer);
        return DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN;
    }

    std::uint64_t faults = 0;
    if (swapchain_texture != nullptr) {
        ops.prepare_draw_data(draw_data, command_buffer);
        SDL_GPUColorTargetInfo target_info = {};
        target_info.texture = swapchain_texture;
        target_info.clear_color = SDL_FColor{ 0.0f, 0.0f, 0.0f, 1.0f };
        target_info.load_op = SDL_GPU_LOADOP_CLEAR;
        target_info.store_op = SDL_GPU_STOREOP_STORE;
        target_info.mip_level = 0;
        target_info.layer_or_depth_plane = 0;
        target_info.cycle = false;
        SDL_GPURenderPass* render_pass = ops.begin_render_pass(
            command_buffer,
            &target_info,
            1,
            nullptr
        );
        if (render_pass == nullptr) {
            faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS;
        } else {
            ops.render_draw_data(draw_data, command_buffer, render_pass, nullptr);
            ops.end_render_pass(render_pass);
        }
    }

    // A swapchain texture may have been acquired, so submit even after a render-pass failure.
    // SDL explicitly forbids cancelling a command buffer after swapchain acquisition.
    if (!ops.submit_command_buffer(command_buffer))
        faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT;
    return faults;
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
enum class DearImguiSdlGpu3FailStage {
    None,
    CommandBuffer,
    Swapchain,
    RenderPass,
    Submit,
};

struct DearImguiSdlGpu3FakeState {
    DearImguiSdlGpu3FailStage fail_stage = DearImguiSdlGpu3FailStage::None;
    int acquire_command_calls = 0;
    int acquire_swapchain_calls = 0;
    int begin_pass_calls = 0;
    int prepare_calls = 0;
    int render_calls = 0;
    int end_pass_calls = 0;
    int submit_calls = 0;
    int cancel_calls = 0;
};

thread_local DearImguiSdlGpu3FakeState fake_render_state;

SDL_GPUCommandBuffer* SDLCALL fake_acquire_command_buffer(SDL_GPUDevice*) {
    fake_render_state.acquire_command_calls++;
    return fake_render_state.fail_stage == DearImguiSdlGpu3FailStage::CommandBuffer
        ? nullptr
        : reinterpret_cast<SDL_GPUCommandBuffer*>(static_cast<std::uintptr_t>(0x501));
}

bool SDLCALL fake_acquire_swapchain_texture(
    SDL_GPUCommandBuffer*,
    SDL_Window*,
    SDL_GPUTexture** texture,
    std::uint32_t*,
    std::uint32_t*
) {
    fake_render_state.acquire_swapchain_calls++;
    if (fake_render_state.fail_stage == DearImguiSdlGpu3FailStage::Swapchain)
        return false;
    *texture = reinterpret_cast<SDL_GPUTexture*>(static_cast<std::uintptr_t>(0x502));
    return true;
}

SDL_GPURenderPass* SDLCALL fake_begin_render_pass(
    SDL_GPUCommandBuffer*,
    const SDL_GPUColorTargetInfo*,
    std::uint32_t,
    const SDL_GPUDepthStencilTargetInfo*
) {
    fake_render_state.begin_pass_calls++;
    return fake_render_state.fail_stage == DearImguiSdlGpu3FailStage::RenderPass
        ? nullptr
        : reinterpret_cast<SDL_GPURenderPass*>(static_cast<std::uintptr_t>(0x503));
}

void SDLCALL fake_end_render_pass(SDL_GPURenderPass*) {
    fake_render_state.end_pass_calls++;
}

bool SDLCALL fake_submit_command_buffer(SDL_GPUCommandBuffer*) {
    fake_render_state.submit_calls++;
    return fake_render_state.fail_stage != DearImguiSdlGpu3FailStage::Submit;
}

bool SDLCALL fake_cancel_command_buffer(SDL_GPUCommandBuffer*) {
    fake_render_state.cancel_calls++;
    return true;
}

void fake_prepare_draw_data(ImDrawData*, SDL_GPUCommandBuffer*) {
    fake_render_state.prepare_calls++;
}

void fake_render_draw_data(
    ImDrawData*,
    SDL_GPUCommandBuffer*,
    SDL_GPURenderPass*,
    SDL_GPUGraphicsPipeline*
) {
    fake_render_state.render_calls++;
}

const DearImguiSdlGpu3RenderOps fake_render_ops = {
    fake_acquire_command_buffer,
    fake_acquire_swapchain_texture,
    fake_begin_render_pass,
    fake_end_render_pass,
    fake_submit_command_buffer,
    fake_cancel_command_buffer,
    fake_prepare_draw_data,
    fake_render_draw_data,
};
#endif

} // namespace

extern "C" std::uint64_t dear_imgui_sdl3_backend_sdlgpu3_render_viewport(
    ImGuiViewport* viewport
) {
    ImGui_ImplSDLGPU3_Data* data = ImGui_ImplSDLGPU3_GetBackendData();
    if (viewport == nullptr || data == nullptr)
        return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
    SDL_Window* window = SDL_GetWindowFromID((SDL_WindowID)(intptr_t)viewport->PlatformHandle);
    return dear_imgui_sdlgpu3_render_checked(
        real_render_ops,
        data->InitInfo.Device,
        window,
        viewport->DrawData
    );
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
extern "C" std::uint64_t dear_imgui_sdl3_backend_sdlgpu3_render_contract_self_test() {
    std::uint64_t failures = 0;
    SDL_GPUDevice* device = reinterpret_cast<SDL_GPUDevice*>(static_cast<std::uintptr_t>(0x601));
    SDL_Window* window = reinterpret_cast<SDL_Window*>(static_cast<std::uintptr_t>(0x602));
    ImDrawData* draw_data = reinterpret_cast<ImDrawData*>(static_cast<std::uintptr_t>(0x603));

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::CommandBuffer;
    std::uint64_t faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data
    );
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER
        || fake_render_state.acquire_swapchain_calls != 0
        || fake_render_state.begin_pass_calls != 0
        || fake_render_state.submit_calls != 0)
        failures |= UINT64_C(1) << 0;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::Swapchain;
    faults = dear_imgui_sdlgpu3_render_checked(fake_render_ops, device, window, draw_data);
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN
        || fake_render_state.cancel_calls != 1
        || fake_render_state.prepare_calls != 0
        || fake_render_state.begin_pass_calls != 0
        || fake_render_state.submit_calls != 0)
        failures |= UINT64_C(1) << 1;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::RenderPass;
    faults = dear_imgui_sdlgpu3_render_checked(fake_render_ops, device, window, draw_data);
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS
        || fake_render_state.prepare_calls != 1
        || fake_render_state.render_calls != 0
        || fake_render_state.end_pass_calls != 0
        || fake_render_state.submit_calls != 1
        || fake_render_state.cancel_calls != 0)
        failures |= UINT64_C(1) << 2;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::Submit;
    faults = dear_imgui_sdlgpu3_render_checked(fake_render_ops, device, window, draw_data);
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT
        || fake_render_state.prepare_calls != 1
        || fake_render_state.render_calls != 1
        || fake_render_state.end_pass_calls != 1
        || fake_render_state.submit_calls != 1)
        failures |= UINT64_C(1) << 3;

    return failures;
}
#endif
