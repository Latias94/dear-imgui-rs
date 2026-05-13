#include "../third-party/cimgui/imgui/imgui.h"

#ifdef IMGUI_HAS_DOCK

struct DearImguiRsPlatformIoHookStorage
{
    ImGuiPlatformIO* PlatformIO;
    void (*Platform_GetWindowPos)(ImGuiViewport* vp, ImVec2* out_pos);
    void (*Platform_GetWindowSize)(ImGuiViewport* vp, ImVec2* out_size);
    void (*Platform_GetWindowFramebufferScale)(ImGuiViewport* vp, ImVec2* out_scale);
    void (*Platform_GetWindowWorkAreaInsets)(ImGuiViewport* vp, ImVec4* out_insets);
};

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
    if (storage->Platform_GetWindowPos != nullptr || storage->Platform_GetWindowSize != nullptr || storage->Platform_GetWindowFramebufferScale != nullptr || storage->Platform_GetWindowWorkAreaInsets != nullptr)
        return;
    G_DearImguiRsPlatformIoHookStorage.erase(storage);
}

static ImVec2 DearImguiRsPlatformGetWindowPosHook(ImGuiViewport* vp)
{
    ImVec2 pos(0.0f, 0.0f);
    if (DearImguiRsPlatformIoHookStorage* storage = DearImguiRsGetCurrentPlatformIoHookStorage())
        if (storage->Platform_GetWindowPos != nullptr)
            storage->Platform_GetWindowPos(vp, &pos);
    return pos;
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

#else

extern "C" void dear_imgui_rs_platform_io_set_platform_get_window_pos(
    ImGuiPlatformIO*,
    void (*)(ImGuiViewport*, ImVec2*))
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

#endif
