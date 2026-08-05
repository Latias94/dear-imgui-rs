#include "../third-party/cimgui/imgui/imgui.h"
#include "../third-party/cimgui/imgui/imgui_internal.h"

#include <stddef.h>
#include <stdint.h>

#ifdef IMGUI_HAS_DOCK

struct DearImguiRsPlatformIoHookStorage
{
    ImGuiPlatformIO* PlatformIO;
    void (*Platform_SetWindowPos)(ImGuiViewport* vp, const ImVec2* pos);
    void (*Platform_GetWindowPos)(ImGuiViewport* vp, ImVec2* out_pos);
    void (*Platform_SetWindowSize)(ImGuiViewport* vp, const ImVec2* size);
    void (*Platform_GetWindowSize)(ImGuiViewport* vp, ImVec2* out_size);
    void (*Platform_GetWindowFramebufferScale)(ImGuiViewport* vp, ImVec2* out_scale);
    void (*Platform_GetWindowWorkAreaInsets)(ImGuiViewport* vp, ImVec4* out_insets);
    void (*Renderer_SetWindowSize)(ImGuiViewport* vp, const ImVec2* size);
};

struct DearImguiRsPlatformIoAggregateProbeResult
{
    ImVec2 PlatformGetWindowPos;
    ImVec2 PlatformGetWindowSize;
    ImVec2 PlatformGetWindowFramebufferScale;
    ImVec4 PlatformGetWindowWorkAreaInsets;
};

#if defined(DEAR_IMGUI_RS_ABI_PROBE)
struct DearImguiRsLayoutProbe
{
    size_t ImDrawDataSize;
    size_t ImDrawDataAlign;
    size_t ImDrawDataFrameCountOffset;
    size_t ImDrawDataCmdListsOffset;
    size_t ImDrawDataTexturesOffset;
    size_t ImTextureDataSize;
    size_t ImTextureDataAlign;
    size_t ImTextureDataUniqueIDOffset;
    size_t ImTextureDataQueueUserDataOffset;
    size_t ImTextureDataTexIDOffset;
    size_t ImTextureDataUpdatesOffset;
    size_t ImTextureDataWantDestroyNextFrameOffset;
    size_t ImGuiPlatformIOSize;
    size_t ImGuiPlatformIOAlign;
    size_t ImGuiPlatformIOSessionDateOffset;
    size_t ImGuiPlatformIORenderStateOffset;
    size_t ImGuiPlatformIOPlatformCreateWindowOffset;
    size_t ImGuiPlatformIORendererCreateWindowOffset;
    size_t ImGuiPlatformIOMonitorsOffset;
    size_t ImGuiPlatformIOTexturesOffset;
    size_t ImGuiPlatformIOViewportsOffset;
};

extern "C" int dear_imgui_rs_probe_public_aggregate_layouts(DearImguiRsLayoutProbe* out_probe)
{
    if (out_probe == nullptr)
        return 0;
    *out_probe = {};
    out_probe->ImDrawDataSize = sizeof(ImDrawData);
    out_probe->ImDrawDataAlign = alignof(ImDrawData);
    out_probe->ImDrawDataFrameCountOffset = offsetof(ImDrawData, FrameCount);
    out_probe->ImDrawDataCmdListsOffset = offsetof(ImDrawData, CmdLists);
    out_probe->ImDrawDataTexturesOffset = offsetof(ImDrawData, Textures);
    out_probe->ImTextureDataSize = sizeof(ImTextureData);
    out_probe->ImTextureDataAlign = alignof(ImTextureData);
    out_probe->ImTextureDataUniqueIDOffset = offsetof(ImTextureData, UniqueID);
    out_probe->ImTextureDataQueueUserDataOffset = offsetof(ImTextureData, QueueUserData);
    out_probe->ImTextureDataTexIDOffset = offsetof(ImTextureData, TexID);
    out_probe->ImTextureDataUpdatesOffset = offsetof(ImTextureData, Updates);
    out_probe->ImTextureDataWantDestroyNextFrameOffset = offsetof(ImTextureData, WantDestroyNextFrame);
    out_probe->ImGuiPlatformIOSize = sizeof(ImGuiPlatformIO);
    out_probe->ImGuiPlatformIOAlign = alignof(ImGuiPlatformIO);
    out_probe->ImGuiPlatformIOSessionDateOffset = offsetof(ImGuiPlatformIO, Platform_SessionDate);
    out_probe->ImGuiPlatformIORenderStateOffset = offsetof(ImGuiPlatformIO, Renderer_RenderState);
    out_probe->ImGuiPlatformIOPlatformCreateWindowOffset = offsetof(ImGuiPlatformIO, Platform_CreateWindow);
    out_probe->ImGuiPlatformIORendererCreateWindowOffset = offsetof(ImGuiPlatformIO, Renderer_CreateWindow);
    out_probe->ImGuiPlatformIOMonitorsOffset = offsetof(ImGuiPlatformIO, Monitors);
    out_probe->ImGuiPlatformIOTexturesOffset = offsetof(ImGuiPlatformIO, Textures);
    out_probe->ImGuiPlatformIOViewportsOffset = offsetof(ImGuiPlatformIO, Viewports);
    return 1;
}
#endif

static ImVector<DearImguiRsPlatformIoHookStorage> G_DearImguiRsPlatformIoHookStorage;

static DearImguiRsPlatformIoHookStorage* DearImguiRsFindPlatformIoHookStorage(ImGuiPlatformIO* platform_io)
{
    if (platform_io == nullptr)
        return nullptr;
    for (int n = 0; n < G_DearImguiRsPlatformIoHookStorage.Size; n++)
        if (G_DearImguiRsPlatformIoHookStorage[n].PlatformIO == platform_io)
            return &G_DearImguiRsPlatformIoHookStorage[n];
    return nullptr;
}

static DearImguiRsPlatformIoHookStorage& DearImguiRsGetPlatformIoHookStorage(ImGuiPlatformIO* platform_io)
{
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
        return *storage;

    DearImguiRsPlatformIoHookStorage storage = {};
    storage.PlatformIO = platform_io;
    G_DearImguiRsPlatformIoHookStorage.push_back(storage);
    return G_DearImguiRsPlatformIoHookStorage[G_DearImguiRsPlatformIoHookStorage.Size - 1];
}

static DearImguiRsPlatformIoHookStorage* DearImguiRsGetCurrentPlatformIoHookStorage()
{
    if (ImGui::GetCurrentContext() == nullptr)
        return nullptr;
    return DearImguiRsFindPlatformIoHookStorage(&ImGui::GetPlatformIO());
}

static void DearImguiRsPrunePlatformIoHookStorageIfEmpty(ImGuiPlatformIO* platform_io)
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (storage == nullptr)
        return;
    if (storage->Platform_SetWindowPos != nullptr || storage->Platform_GetWindowPos != nullptr || storage->Platform_SetWindowSize != nullptr || storage->Platform_GetWindowSize != nullptr || storage->Platform_GetWindowFramebufferScale != nullptr || storage->Platform_GetWindowWorkAreaInsets != nullptr || storage->Renderer_SetWindowSize != nullptr)
        return;
    G_DearImguiRsPlatformIoHookStorage.erase(storage);
}

static void DearImguiRsPlatformSetWindowPosHook(ImGuiViewport* vp, ImVec2 pos)
{
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_SetWindowPos != nullptr)
            storage->Platform_SetWindowPos(vp, &pos);
}

static ImVec2 DearImguiRsPlatformGetWindowPosHook(ImGuiViewport* vp)
{
    ImVec2 pos(0.0f, 0.0f);
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_GetWindowPos != nullptr)
            storage->Platform_GetWindowPos(vp, &pos);
    return pos;
}

static void DearImguiRsPlatformSetWindowSizeHook(ImGuiViewport* vp, ImVec2 size)
{
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_SetWindowSize != nullptr)
            storage->Platform_SetWindowSize(vp, &size);
}

static ImVec2 DearImguiRsPlatformGetWindowSizeHook(ImGuiViewport* vp)
{
    ImVec2 size(0.0f, 0.0f);
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_GetWindowSize != nullptr)
            storage->Platform_GetWindowSize(vp, &size);
    return size;
}

static ImVec2 DearImguiRsPlatformGetWindowFramebufferScaleHook(ImGuiViewport* vp)
{
    ImVec2 scale(1.0f, 1.0f);
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_GetWindowFramebufferScale != nullptr)
            storage->Platform_GetWindowFramebufferScale(vp, &scale);
    return scale;
}

static ImVec4 DearImguiRsPlatformGetWindowWorkAreaInsetsHook(ImGuiViewport* vp)
{
    ImVec4 insets(0.0f, 0.0f, 0.0f, 0.0f);
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_GetWindowWorkAreaInsets != nullptr)
            storage->Platform_GetWindowWorkAreaInsets(vp, &insets);
    return insets;
}

static void DearImguiRsRendererSetWindowSizeHook(ImGuiViewport* vp, ImVec2 size)
{
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Renderer_SetWindowSize != nullptr)
            storage->Renderer_SetWindowSize(vp, &size);
}

extern "C" void dear_imgui_rs_platform_io_set_platform_set_window_pos(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* pos))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Platform_SetWindowPos = nullptr;
        platform_io->Platform_SetWindowPos = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Platform_SetWindowPos = user_callback;
    platform_io->Platform_SetWindowPos = DearImguiRsPlatformSetWindowPosHook;
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_pos(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec2* out_pos))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Platform_GetWindowPos = nullptr;
        platform_io->Platform_GetWindowPos = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Platform_GetWindowPos = user_callback;
    platform_io->Platform_GetWindowPos = DearImguiRsPlatformGetWindowPosHook;
}

extern "C" void dear_imgui_rs_platform_io_set_platform_set_window_size(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* size))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Platform_SetWindowSize = nullptr;
        platform_io->Platform_SetWindowSize = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Platform_SetWindowSize = user_callback;
    platform_io->Platform_SetWindowSize = DearImguiRsPlatformSetWindowSizeHook;
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_size(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec2* out_size))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Platform_GetWindowSize = nullptr;
        platform_io->Platform_GetWindowSize = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Platform_GetWindowSize = user_callback;
    platform_io->Platform_GetWindowSize = DearImguiRsPlatformGetWindowSizeHook;
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_framebuffer_scale(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec2* out_scale))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Platform_GetWindowFramebufferScale = nullptr;
        platform_io->Platform_GetWindowFramebufferScale = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Platform_GetWindowFramebufferScale = user_callback;
    platform_io->Platform_GetWindowFramebufferScale = DearImguiRsPlatformGetWindowFramebufferScaleHook;
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_work_area_insets(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec4* out_insets))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Platform_GetWindowWorkAreaInsets = nullptr;
        platform_io->Platform_GetWindowWorkAreaInsets = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Platform_GetWindowWorkAreaInsets = user_callback;
    platform_io->Platform_GetWindowWorkAreaInsets = DearImguiRsPlatformGetWindowWorkAreaInsetsHook;
}

extern "C" int dear_imgui_rs_platform_io_clear_platform_set_window_pos_if_pointer_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* pos))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Platform_SetWindowPos != user_callback)
        return 0;

    const bool owned_slot = platform_io->Platform_SetWindowPos == DearImguiRsPlatformSetWindowPosHook;
    storage->Platform_SetWindowPos = nullptr;
    if (owned_slot)
        platform_io->Platform_SetWindowPos = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return owned_slot ? 1 : 0;
}

extern "C" int dear_imgui_rs_platform_io_clear_platform_set_window_size_if_pointer_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* size))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Platform_SetWindowSize != user_callback)
        return 0;

    const bool owned_slot = platform_io->Platform_SetWindowSize == DearImguiRsPlatformSetWindowSizeHook;
    storage->Platform_SetWindowSize = nullptr;
    if (owned_slot)
        platform_io->Platform_SetWindowSize = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return owned_slot ? 1 : 0;
}

extern "C" int dear_imgui_rs_platform_io_clear_platform_get_window_pos_if_out_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec2* out_pos))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Platform_GetWindowPos != user_callback)
        return 0;

    const bool owned_slot = platform_io->Platform_GetWindowPos == DearImguiRsPlatformGetWindowPosHook;
    storage->Platform_GetWindowPos = nullptr;
    if (owned_slot)
        platform_io->Platform_GetWindowPos = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return owned_slot ? 1 : 0;
}

extern "C" int dear_imgui_rs_platform_io_clear_platform_get_window_size_if_out_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec2* out_size))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Platform_GetWindowSize != user_callback)
        return 0;

    const bool owned_slot = platform_io->Platform_GetWindowSize == DearImguiRsPlatformGetWindowSizeHook;
    storage->Platform_GetWindowSize = nullptr;
    if (owned_slot)
        platform_io->Platform_GetWindowSize = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return owned_slot ? 1 : 0;
}

extern "C" int dear_imgui_rs_platform_io_clear_platform_get_window_framebuffer_scale_if_out_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec2* out_scale))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Platform_GetWindowFramebufferScale != user_callback)
        return 0;

    const bool owned_slot = platform_io->Platform_GetWindowFramebufferScale == DearImguiRsPlatformGetWindowFramebufferScaleHook;
    storage->Platform_GetWindowFramebufferScale = nullptr;
    if (owned_slot)
        platform_io->Platform_GetWindowFramebufferScale = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return owned_slot ? 1 : 0;
}

extern "C" int dear_imgui_rs_platform_io_clear_platform_get_window_work_area_insets_if_out_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, ImVec4* out_insets))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Platform_GetWindowWorkAreaInsets != user_callback)
        return 0;

    const bool owned_slot = platform_io->Platform_GetWindowWorkAreaInsets == DearImguiRsPlatformGetWindowWorkAreaInsetsHook;
    storage->Platform_GetWindowWorkAreaInsets = nullptr;
    if (owned_slot)
        platform_io->Platform_GetWindowWorkAreaInsets = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return owned_slot ? 1 : 0;
}

extern "C" void dear_imgui_rs_platform_io_set_renderer_set_window_size(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* size))
{
    if (platform_io == nullptr)
        return;

    if (user_callback == nullptr)
    {
        if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io))
            storage->Renderer_SetWindowSize = nullptr;
        platform_io->Renderer_SetWindowSize = nullptr;
        DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
        return;
    }

    DearImguiRsPlatformIoHookStorage& storage = DearImguiRsGetPlatformIoHookStorage(platform_io);
    storage.Renderer_SetWindowSize = user_callback;
    platform_io->Renderer_SetWindowSize = DearImguiRsRendererSetWindowSizeHook;
}

extern "C" int dear_imgui_rs_platform_io_renderer_set_window_size_matches_pointer_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* size))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    return platform_io != nullptr
        && storage != nullptr
        && platform_io->Renderer_SetWindowSize == DearImguiRsRendererSetWindowSizeHook
        && storage->Renderer_SetWindowSize == user_callback;
}

extern "C" int dear_imgui_rs_platform_io_get_renderer_set_window_size_pointer_param(
    ImGuiPlatformIO* platform_io,
    void (**out_callback)(ImGuiViewport* vp, const ImVec2* size))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr
        || storage == nullptr
        || out_callback == nullptr
        || platform_io->Renderer_SetWindowSize != DearImguiRsRendererSetWindowSizeHook
        || storage->Renderer_SetWindowSize == nullptr)
        return 0;
    *out_callback = storage->Renderer_SetWindowSize;
    return 1;
}

extern "C" int dear_imgui_rs_platform_io_clear_renderer_set_window_size_if_pointer_param(
    ImGuiPlatformIO* platform_io,
    void (*user_callback)(ImGuiViewport* vp, const ImVec2* size))
{
    DearImguiRsPlatformIoHookStorage* storage = DearImguiRsFindPlatformIoHookStorage(platform_io);
    if (platform_io == nullptr || storage == nullptr || storage->Renderer_SetWindowSize != user_callback)
        return 0;

    storage->Renderer_SetWindowSize = nullptr;
    if (platform_io->Renderer_SetWindowSize == DearImguiRsRendererSetWindowSizeHook)
        platform_io->Renderer_SetWindowSize = nullptr;
    DearImguiRsPrunePlatformIoHookStorageIfEmpty(platform_io);
    return 1;
}

extern "C" int dear_imgui_rs_platform_io_probe_aggregate_callbacks(
    ImGuiPlatformIO* platform_io,
    DearImguiRsPlatformIoAggregateProbeResult* out_result)
{
    if (platform_io == nullptr || out_result == nullptr || platform_io->Platform_SetWindowPos == nullptr || platform_io->Platform_GetWindowPos == nullptr || platform_io->Platform_SetWindowSize == nullptr || platform_io->Platform_GetWindowSize == nullptr || platform_io->Platform_GetWindowFramebufferScale == nullptr || platform_io->Platform_GetWindowWorkAreaInsets == nullptr || platform_io->Renderer_SetWindowSize == nullptr)
        return 0;

    ImGuiViewport viewport = {};
    platform_io->Platform_SetWindowPos(&viewport, ImVec2(1.0f, 2.0f));
    platform_io->Platform_SetWindowSize(&viewport, ImVec2(3.0f, 4.0f));
    out_result->PlatformGetWindowPos = platform_io->Platform_GetWindowPos(&viewport);
    out_result->PlatformGetWindowSize = platform_io->Platform_GetWindowSize(&viewport);
    out_result->PlatformGetWindowFramebufferScale = platform_io->Platform_GetWindowFramebufferScale(&viewport);
    out_result->PlatformGetWindowWorkAreaInsets = platform_io->Platform_GetWindowWorkAreaInsets(&viewport);
    platform_io->Renderer_SetWindowSize(&viewport, ImVec2(15.0f, 16.0f));
    return 1;
}

extern "C" int dear_imgui_rs_platform_io_invoke_platform_set_window_pos(
    const ImGuiPlatformIO* callbacks,
    ImGuiViewport* viewport,
    const ImVec2* pos)
{
    if (callbacks == nullptr || viewport == nullptr || pos == nullptr || callbacks->Platform_SetWindowPos == nullptr)
        return 0;
    callbacks->Platform_SetWindowPos(viewport, *pos);
    return 1;
}

extern "C" int dear_imgui_rs_platform_io_invoke_renderer_set_window_size(
    const ImGuiPlatformIO* callbacks,
    ImGuiViewport* viewport,
    const ImVec2* size)
{
    if (callbacks == nullptr || viewport == nullptr || size == nullptr || callbacks->Renderer_SetWindowSize == nullptr)
        return 0;
    callbacks->Renderer_SetWindowSize(viewport, *size);
    return 1;
}

extern "C" int dear_imgui_rs_platform_io_aggregate_callback_storage_count()
{
    return G_DearImguiRsPlatformIoHookStorage.Size;
}

extern "C" ImGuiViewport* dear_imgui_rs_find_live_viewport_by_address(
    ImGuiContext* context,
    uintptr_t address)
{
    if (context == nullptr || context != ImGui::GetCurrentContext() || address == 0)
        return nullptr;

    for (ImGuiViewportP* viewport : context->Viewports)
        if (reinterpret_cast<uintptr_t>(viewport) == address)
            return viewport;
    return nullptr;
}

#else

struct DearImguiRsPlatformIoAggregateProbeResult
{
    ImVec2 PlatformGetWindowPos;
    ImVec2 PlatformGetWindowSize;
    ImVec2 PlatformGetWindowFramebufferScale;
    ImVec4 PlatformGetWindowWorkAreaInsets;
};

extern "C" void dear_imgui_rs_platform_io_set_platform_set_window_pos(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, const ImVec2*))
{
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_pos(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, ImVec2*))
{
}

extern "C" void dear_imgui_rs_platform_io_set_platform_set_window_size(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, const ImVec2*))
{
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_size(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, ImVec2*))
{
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_framebuffer_scale(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, ImVec2*))
{
}

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_work_area_insets(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, ImVec4*))
{
}

extern "C" void dear_imgui_rs_platform_io_set_renderer_set_window_size(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, const ImVec2*))
{
}

extern "C" int dear_imgui_rs_platform_io_renderer_set_window_size_matches_pointer_param(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, const ImVec2*))
{
    return 0;
}

extern "C" int dear_imgui_rs_platform_io_clear_renderer_set_window_size_if_pointer_param(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, const ImVec2*))
{
    return 0;
}

extern "C" int dear_imgui_rs_platform_io_probe_aggregate_callbacks(
    ImGuiPlatformIO*,
    DearImguiRsPlatformIoAggregateProbeResult*)
{
    return 0;
}

extern "C" int dear_imgui_rs_platform_io_invoke_platform_set_window_pos(
    const ImGuiPlatformIO*,
    ImGuiViewport*,
    const ImVec2*)
{
    return 0;
}

extern "C" int dear_imgui_rs_platform_io_get_renderer_set_window_size_pointer_param(
    ImGuiPlatformIO*,
    void (**)(ImGuiViewport*, const ImVec2*))
{
    return 0;
}

extern "C" int dear_imgui_rs_platform_io_invoke_renderer_set_window_size(
    const ImGuiPlatformIO*,
    ImGuiViewport*,
    const ImVec2*)
{
    return 0;
}

extern "C" int dear_imgui_rs_platform_io_aggregate_callback_storage_count()
{
    return 0;
}

extern "C" ImGuiViewport* dear_imgui_rs_find_live_viewport_by_address(
    ImGuiContext*,
    uintptr_t)
{
    return nullptr;
}

#endif
