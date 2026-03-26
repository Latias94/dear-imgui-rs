#include <cstdint>
#include <windows.h>

#include "imgui.h"
#include "backends/imgui_impl_win32.h"

extern IMGUI_IMPL_API LRESULT ImGui_ImplWin32_WndProcHandler(
    HWND hWnd,
    UINT msg,
    WPARAM wParam,
    LPARAM lParam
);

extern "C" {

bool dear_imgui_backend_win32_init(void* hwnd) {
    return ImGui_ImplWin32_Init(hwnd);
}

bool dear_imgui_backend_win32_init_for_opengl(void* hwnd) {
    return ImGui_ImplWin32_InitForOpenGL(hwnd);
}

void dear_imgui_backend_win32_shutdown() {
    ImGui_ImplWin32_Shutdown();
}

void dear_imgui_backend_win32_new_frame() {
    ImGui_ImplWin32_NewFrame();
}

intptr_t dear_imgui_backend_win32_wnd_proc_handler(
    void* hwnd,
    unsigned int msg,
    uintptr_t wparam,
    intptr_t lparam
) {
    return static_cast<intptr_t>(ImGui_ImplWin32_WndProcHandler(
        static_cast<HWND>(hwnd),
        static_cast<UINT>(msg),
        static_cast<WPARAM>(wparam),
        static_cast<LPARAM>(lparam)
    ));
}

void dear_imgui_backend_win32_enable_dpi_awareness() {
    ImGui_ImplWin32_EnableDpiAwareness();
}

float dear_imgui_backend_win32_get_dpi_scale_for_hwnd(void* hwnd) {
    return ImGui_ImplWin32_GetDpiScaleForHwnd(hwnd);
}

float dear_imgui_backend_win32_get_dpi_scale_for_monitor(void* monitor) {
    return ImGui_ImplWin32_GetDpiScaleForMonitor(monitor);
}

void dear_imgui_backend_win32_enable_alpha_compositing(void* hwnd) {
    ImGui_ImplWin32_EnableAlphaCompositing(hwnd);
}

} // extern "C"
