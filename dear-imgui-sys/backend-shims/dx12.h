#pragma once
#include <cstdint>

// Repository-owned C ABI. Descriptor structs never cross this boundary by value.
struct DearImGuiDx12DescriptorAllocator {
    void* user_data;
    void (*allocate)(void* user_data, uintptr_t* cpu_handle, uint64_t* gpu_handle);
    void (*free)(void* user_data, uintptr_t cpu_handle, uint64_t gpu_handle);
};

struct DearImGuiDx12InitInfo {
    void* device;
    void* command_queue;
    int num_frames_in_flight;
    uint32_t rtv_format;
    uint32_t dsv_format;
    void* srv_descriptor_heap;
    const DearImGuiDx12DescriptorAllocator* descriptor_allocator;
};
