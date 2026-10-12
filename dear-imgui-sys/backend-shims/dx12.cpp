#include "dx12.h"
#include "imgui.h"
#include "backends/imgui_impl_dx12.h"

extern "C" {

bool dear_imgui_backend_dx12_init(const DearImGuiDx12InitInfo* info) {
    if (!info || !ImGui::GetCurrentContext() || ImGui::GetIO().BackendRendererUserData ||
        !info->device || !info->command_queue || info->num_frames_in_flight <= 0 ||
        info->rtv_format == DXGI_FORMAT_UNKNOWN || !info->srv_descriptor_heap ||
        !info->descriptor_allocator || !info->descriptor_allocator->allocate ||
        !info->descriptor_allocator->free) {
        return false;
    }

    ImGui_ImplDX12_InitInfo native_info;
    native_info.Device = static_cast<ID3D12Device*>(info->device);
    native_info.CommandQueue = static_cast<ID3D12CommandQueue*>(info->command_queue);
    native_info.NumFramesInFlight = info->num_frames_in_flight;
    native_info.RTVFormat = static_cast<DXGI_FORMAT>(info->rtv_format);
    native_info.DSVFormat = static_cast<DXGI_FORMAT>(info->dsv_format);
    native_info.SrvDescriptorHeap = static_cast<ID3D12DescriptorHeap*>(info->srv_descriptor_heap);
    // Upstream copies native_info. Only the caller-owned allocator must remain at a stable address.
    native_info.UserData = const_cast<DearImGuiDx12DescriptorAllocator*>(info->descriptor_allocator);
    native_info.SrvDescriptorAllocFn = [](ImGui_ImplDX12_InitInfo* native,
                                        D3D12_CPU_DESCRIPTOR_HANDLE* cpu,
                                        D3D12_GPU_DESCRIPTOR_HANDLE* gpu) {
        const auto* allocator = static_cast<const DearImGuiDx12DescriptorAllocator*>(native->UserData);
        uintptr_t cpu_handle = 0;
        uint64_t gpu_handle = 0;
        allocator->allocate(allocator->user_data, &cpu_handle, &gpu_handle);
        cpu->ptr = cpu_handle;
        gpu->ptr = gpu_handle;
    };
    native_info.SrvDescriptorFreeFn = [](ImGui_ImplDX12_InitInfo* native,
                                       D3D12_CPU_DESCRIPTOR_HANDLE cpu,
                                       D3D12_GPU_DESCRIPTOR_HANDLE gpu) {
        const auto* allocator = static_cast<const DearImGuiDx12DescriptorAllocator*>(native->UserData);
        allocator->free(allocator->user_data, cpu.ptr, gpu.ptr);
    };
    return ImGui_ImplDX12_Init(&native_info);
}

void dear_imgui_backend_dx12_shutdown() { ImGui_ImplDX12_Shutdown(); }
void dear_imgui_backend_dx12_new_frame() { ImGui_ImplDX12_NewFrame(); }
void dear_imgui_backend_dx12_render_draw_data(ImDrawData* draw_data, void* command_list) {
    ImGui_ImplDX12_RenderDrawData(draw_data, static_cast<ID3D12GraphicsCommandList*>(command_list));
}
bool dear_imgui_backend_dx12_create_device_objects() { return ImGui_ImplDX12_CreateDeviceObjects(); }
void dear_imgui_backend_dx12_invalidate_device_objects() { ImGui_ImplDX12_InvalidateDeviceObjects(); }
void dear_imgui_backend_dx12_update_texture(ImTextureData* texture) { ImGui_ImplDX12_UpdateTexture(texture); }

} // extern "C"
