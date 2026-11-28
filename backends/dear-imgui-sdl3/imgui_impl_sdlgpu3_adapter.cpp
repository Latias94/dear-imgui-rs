#include <SDL3/SDL.h>

#include "backends/imgui_impl_sdlgpu3.h"
#include "native_bridge.h"

#include <cstdlib>

#define SDL_ClaimWindowForGPUDevice dear_imgui_sdl3_hook_claim_window_for_gpu_device
#define SDL_SetGPUSwapchainParameters dear_imgui_sdl3_hook_set_gpu_swapchain_parameters

#include "backends/imgui_impl_sdlgpu3.cpp"

#undef SDL_SetGPUSwapchainParameters
#undef SDL_ClaimWindowForGPUDevice

namespace {

// Keep the upstream reusable transfer buffer. Publish its new capacity only after allocation
// succeeds, and acknowledge the request only once SDL accepts the upload command buffer.
bool upload_texture_region(ImGui_ImplSDLGPU3_Data& bd, ImTextureData& tex, SDL_GPUTextureRegion region) {
    SDL_GPUDevice* device = bd.InitInfo.Device;
    const Uint32 pitch = region.w * tex.BytesPerPixel;
    const Uint32 bytes = pitch * region.h; // Rust validates the entire CPU image fits in i32.
    // DestroyDeviceObjects releases the transfer buffer but keeps its previous capacity.
    if (!bd.TexTransferBuffer || bd.TexTransferBufferSize < bytes) {
        SDL_GPUTransferBufferCreateInfo info = {};
        info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
        info.size = bytes + 1024;
        SDL_GPUTransferBuffer* buffer = SDL_CreateGPUTransferBuffer(device, &info);
        if (!buffer)
            return false;
        SDL_ReleaseGPUTransferBuffer(device, bd.TexTransferBuffer);
        bd.TexTransferBuffer = buffer;
        bd.TexTransferBufferSize = info.size;
    }

    auto* mapped = static_cast<Uint8*>(SDL_MapGPUTransferBuffer(device, bd.TexTransferBuffer, true));
    if (!mapped)
        return false;
    for (Uint32 y = 0; y < region.h; ++y)
        memcpy(mapped + y * pitch, tex.GetPixelsAt(region.x, region.y + y), pitch);
    SDL_UnmapGPUTransferBuffer(device, bd.TexTransferBuffer);

    SDL_GPUCommandBuffer* cmd = SDL_AcquireGPUCommandBuffer(device);
    if (!cmd)
        return false;
    SDL_GPUCopyPass* pass = SDL_BeginGPUCopyPass(cmd);
    if (!pass) {
        SDL_CancelGPUCommandBuffer(cmd);
        return false;
    }
    SDL_GPUTextureTransferInfo source = {};
    source.transfer_buffer = bd.TexTransferBuffer;
    SDL_UploadToGPUTexture(pass, &source, &region, false);
    SDL_EndGPUCopyPass(pass);
    return SDL_SubmitGPUCommandBuffer(cmd);
}

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

// The upstream renderer temporarily publishes a pointer to stack state. Preserve the
// exact incoming state even if the draw aborts with a C++ exception.
void dear_imgui_sdl3_render_sdlgpu3_draw_data(
    ImDrawData* draw_data,
    SDL_GPUCommandBuffer* command_buffer,
    SDL_GPURenderPass* render_pass,
    SDL_GPUGraphicsPipeline* pipeline
) {
    auto* data = ImGui_ImplSDLGPU3_GetBackendData();
    ImGuiPlatformIO& platform = ImGui::GetPlatformIO();
    void* previous_platform_state = platform.Renderer_RenderState;
    auto* previous_backend_state = data->RenderState;
    try {
        ImGui_ImplSDLGPU3_RenderDrawData(draw_data, command_buffer, render_pass, pipeline);
    } catch (...) {
        platform.Renderer_RenderState = previous_platform_state;
        data->RenderState = previous_backend_state;
        throw;
    }
}

const DearImguiSdlGpu3RenderOps real_render_ops = {
    SDL_AcquireGPUCommandBuffer,
    SDL_AcquireGPUSwapchainTexture,
    SDL_BeginGPURenderPass,
    SDL_EndGPURenderPass,
    SDL_SubmitGPUCommandBuffer,
    SDL_CancelGPUCommandBuffer,
    ImGui_ImplSDLGPU3_PrepareDrawData,
    dear_imgui_sdl3_render_sdlgpu3_draw_data,
};

std::uint64_t dear_imgui_sdlgpu3_render_checked(
    const DearImguiSdlGpu3RenderOps& ops,
    SDL_GPUDevice* device,
    SDL_Window* window,
    ImDrawData* draw_data,
    ImGuiViewportFlags viewport_flags,
    bool transparent,
    std::uint64_t* first_fault
) {
    if (first_fault != nullptr)
        *first_fault = 0;

    if (device == nullptr || window == nullptr || draw_data == nullptr) {
        if (first_fault != nullptr)
            *first_fault = DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
        return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
    }

    SDL_GPUCommandBuffer* command_buffer = ops.acquire_command_buffer(device);
    if (command_buffer == nullptr) {
        if (first_fault != nullptr)
            *first_fault = DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER;
        return DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER;
    }

    SDL_GPUTexture* swapchain_texture = nullptr;
    if (!ops.acquire_swapchain_texture(
            command_buffer,
            window,
            &swapchain_texture,
            nullptr,
            nullptr)) {
        std::uint64_t faults = DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN;
        if (!ops.cancel_command_buffer(command_buffer))
            faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_CANCEL;
        if (first_fault != nullptr)
            *first_fault = DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN;
        return faults;
    }

    std::uint64_t faults = 0;
    if (swapchain_texture != nullptr) {
        SDL_GPURenderPass* render_pass = nullptr;
        try {
            ops.prepare_draw_data(draw_data, command_buffer);
            SDL_GPUColorTargetInfo target_info = {};
            target_info.texture = swapchain_texture;
            target_info.clear_color = SDL_FColor{ 0.0f, 0.0f, 0.0f, transparent ? 0.0f : 1.0f };
            target_info.load_op = (viewport_flags & ImGuiViewportFlags_NoRendererClear)
                ? SDL_GPU_LOADOP_LOAD : SDL_GPU_LOADOP_CLEAR;
            target_info.store_op = SDL_GPU_STOREOP_STORE;
            render_pass = ops.begin_render_pass(command_buffer, &target_info, 1, nullptr);
            if (render_pass == nullptr) {
                faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS;
                if (first_fault != nullptr)
                    *first_fault = DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS;
            } else {
                ops.render_draw_data(draw_data, command_buffer, render_pass, nullptr);
            }
        } catch (...) {
            faults |= DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
            if (first_fault != nullptr)
                *first_fault = DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
        }
        if (render_pass != nullptr)
            ops.end_render_pass(render_pass);
    }

    // A swapchain texture may have been acquired, so submit even after a render-pass failure.
    // SDL explicitly forbids cancelling a command buffer after swapchain acquisition.
    if (!ops.submit_command_buffer(command_buffer)) {
        faults |= DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT;
        if (first_fault != nullptr && *first_fault == 0)
            *first_fault = DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT;
    }
    return faults;
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
enum class DearImguiSdlGpu3FailStage {
    None,
    CommandBuffer,
    Swapchain,
    RenderPass,
    DrawException,
    Submit,
};

struct DearImguiSdlGpu3FakeState {
    DearImguiSdlGpu3FailStage fail_stage = DearImguiSdlGpu3FailStage::None;
    bool cancel_fails = false;
    int acquire_command_calls = 0;
    int acquire_swapchain_calls = 0;
    int begin_pass_calls = 0;
    int prepare_calls = 0;
    int render_calls = 0;
    int end_pass_calls = 0;
    int submit_calls = 0;
    int cancel_calls = 0;
    SDL_GPULoadOp load_op = SDL_GPU_LOADOP_DONT_CARE;
    float clear_alpha = -1.0f;
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
    const SDL_GPUColorTargetInfo* target,
    std::uint32_t,
    const SDL_GPUDepthStencilTargetInfo*
) {
    fake_render_state.begin_pass_calls++;
    fake_render_state.load_op = target->load_op;
    fake_render_state.clear_alpha = target->clear_color.a;
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
    return !fake_render_state.cancel_fails;
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
    if (fake_render_state.fail_stage == DearImguiSdlGpu3FailStage::DrawException)
        throw DearImguiSdlGpu3FailStage::DrawException;
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

static void update_gpu_texture(ImGui_ImplSDLGPU3_Data* bd, ImTextureData* tex, decltype(&upload_texture_region) upload) {
    if (tex->Status != ImTextureStatus_WantCreate && tex->Status != ImTextureStatus_WantUpdates) {
        ImGui_ImplSDLGPU3_UpdateTexture(tex);
        return;
    }

    const bool creating = tex->Status == ImTextureStatus_WantCreate;
    SDL_GPUTextureRegion region = {};
    region.d = 1;
    if (creating) {
        SDL_GPUTextureCreateInfo info = {};
        info.type = SDL_GPU_TEXTURETYPE_2D;
        info.format = SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM;
        info.usage = SDL_GPU_TEXTUREUSAGE_SAMPLER;
        region.w = tex->Width;
        region.h = tex->Height;
        info.width = region.w;
        info.height = region.h;
        info.layer_count_or_depth = 1;
        info.num_levels = 1;
        info.sample_count = SDL_GPU_SAMPLECOUNT_1;
        region.texture = SDL_CreateGPUTexture(bd->InitInfo.Device, &info);
        if (!region.texture)
            return;
    } else {
        region.texture = (SDL_GPUTexture*)(intptr_t)tex->GetTexID();
        region.x = tex->UpdateRect.x;
        region.y = tex->UpdateRect.y;
        region.w = tex->UpdateRect.w;
        region.h = tex->UpdateRect.h;
    }
    if (!upload(*bd, *tex, region)) {
        if (creating)
            SDL_ReleaseGPUTexture(bd->InitInfo.Device, region.texture);
        return;
    }
    if (creating)
        tex->SetTexID((ImTextureID)(intptr_t)region.texture);
    tex->SetStatus(ImTextureStatus_OK);
}

extern "C" void dear_imgui_sdl3_backend_sdlgpu3_update_texture(ImTextureData* tex) try {
    update_gpu_texture(ImGui_ImplSDLGPU3_GetBackendData(), tex, upload_texture_region);
} catch (...) { std::abort(); }

extern "C" std::uint64_t dear_imgui_sdl3_backend_sdlgpu3_render_viewport(
    ImGuiViewport* viewport,
    std::uint64_t* first_fault
) try {
    ImGui_ImplSDLGPU3_Data* data = ImGui_ImplSDLGPU3_GetBackendData();
    if (viewport == nullptr || data == nullptr) {
        if (first_fault != nullptr)
            *first_fault = DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
        return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
    }
    SDL_Window* window = SDL_GetWindowFromID((SDL_WindowID)(intptr_t)viewport->PlatformHandle);
    return dear_imgui_sdlgpu3_render_checked(
        real_render_ops,
        data->InitInfo.Device,
        window,
        viewport->DrawData,
        viewport->Flags,
        window != nullptr && (SDL_GetWindowFlags(window) & SDL_WINDOW_TRANSPARENT) != 0,
        first_fault
    );
} catch (...) {
    if (first_fault != nullptr)
        *first_fault = DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
    return DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL;
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
extern "C" bool dear_imgui_sdl3_backend_sdlgpu3_texture_self_test() try {
    ImGui_ImplSDLGPU3_Data bd{};
    ImTextureData tex;
    tex.Create(ImTextureFormat_RGBA32, 4, 2);
    tex.UpdateRect = {2, 1, 1, 1};
    tex.SetTexID(77);
    tex.SetStatus(ImTextureStatus_WantUpdates);
    update_gpu_texture(&bd, &tex, [](ImGui_ImplSDLGPU3_Data&, ImTextureData&, SDL_GPUTextureRegion) { return false; });
    if (tex.Status != ImTextureStatus_WantUpdates || tex.GetTexID() != 77)
        return false;
    update_gpu_texture(&bd, &tex, [](ImGui_ImplSDLGPU3_Data&, ImTextureData&, SDL_GPUTextureRegion region) {
        return region.x == 2 && region.y == 1 && region.w == 1 && region.h == 1;
    });
    return tex.Status == ImTextureStatus_OK && tex.GetTexID() == 77;
} catch (...) { return false; }

extern "C" std::uint64_t dear_imgui_sdl3_backend_sdlgpu3_render_contract_self_test() try {
    std::uint64_t failures = 0;
    SDL_GPUDevice* device = reinterpret_cast<SDL_GPUDevice*>(static_cast<std::uintptr_t>(0x601));
    SDL_Window* window = reinterpret_cast<SDL_Window*>(static_cast<std::uintptr_t>(0x602));
    ImDrawData* draw_data = reinterpret_cast<ImDrawData*>(static_cast<std::uintptr_t>(0x603));
    std::uint64_t first_fault = 0;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::CommandBuffer;
    std::uint64_t faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER
        || first_fault != DEAR_IMGUI_SDL3_FAULT_SDLGPU_COMMAND_BUFFER
        || fake_render_state.acquire_swapchain_calls != 0
        || fake_render_state.begin_pass_calls != 0
        || fake_render_state.submit_calls != 0)
        failures |= UINT64_C(1) << 0;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::Swapchain;
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN
        || first_fault != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN
        || fake_render_state.cancel_calls != 1
        || fake_render_state.prepare_calls != 0
        || fake_render_state.begin_pass_calls != 0
        || fake_render_state.submit_calls != 0)
        failures |= UINT64_C(1) << 1;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::Swapchain;
    fake_render_state.cancel_fails = true;
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != (DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN
            | DEAR_IMGUI_SDL3_FAULT_SDLGPU_CANCEL)
        || first_fault != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SWAPCHAIN
        || fake_render_state.cancel_calls != 1
        || fake_render_state.prepare_calls != 0
        || fake_render_state.begin_pass_calls != 0
        || fake_render_state.submit_calls != 0)
        failures |= UINT64_C(1) << 4;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::RenderPass;
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS
        || first_fault != DEAR_IMGUI_SDL3_FAULT_SDLGPU_RENDER_PASS
        || fake_render_state.prepare_calls != 1
        || fake_render_state.render_calls != 0
        || fake_render_state.end_pass_calls != 0
        || fake_render_state.submit_calls != 1
        || fake_render_state.cancel_calls != 0)
        failures |= UINT64_C(1) << 2;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::Submit;
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT
        || first_fault != DEAR_IMGUI_SDL3_FAULT_SDLGPU_SUBMIT
        || fake_render_state.prepare_calls != 1
        || fake_render_state.render_calls != 1
        || fake_render_state.end_pass_calls != 1
        || fake_render_state.submit_calls != 1)
        failures |= UINT64_C(1) << 3;

    fake_render_state = {};
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != 0 || first_fault != 0 || fake_render_state.load_op != SDL_GPU_LOADOP_CLEAR
        || fake_render_state.clear_alpha != 1.0f)
        failures |= UINT64_C(1) << 5;

    fake_render_state = {};
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, true, &first_fault
    );
    if (faults != 0 || first_fault != 0 || fake_render_state.load_op != SDL_GPU_LOADOP_CLEAR
        || fake_render_state.clear_alpha != 0.0f)
        failures |= UINT64_C(1) << 6;

    fake_render_state = {};
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, ImGuiViewportFlags_NoRendererClear, true,
        &first_fault
    );
    if (faults != 0 || first_fault != 0 || fake_render_state.load_op != SDL_GPU_LOADOP_LOAD)
        failures |= UINT64_C(1) << 7;

    fake_render_state = {};
    fake_render_state.fail_stage = DearImguiSdlGpu3FailStage::DrawException;
    faults = dear_imgui_sdlgpu3_render_checked(
        fake_render_ops, device, window, draw_data, 0, false, &first_fault
    );
    if (faults != DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL
        || first_fault != DEAR_IMGUI_SDL3_FAULT_NATIVE_PROTOCOL
        || fake_render_state.render_calls != 1 || fake_render_state.end_pass_calls != 1
        || fake_render_state.submit_calls != 1 || fake_render_state.cancel_calls != 0)
        failures |= UINT64_C(1) << 8;

    return failures;
} catch (...) { return UINT64_MAX; }
#endif
