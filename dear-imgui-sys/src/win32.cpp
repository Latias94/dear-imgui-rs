// source code in imgui_impl_win32.cpp:
// ```
// extern IMGUI_IMPL_API LRESULT ImGui_ImplWin32_WndProcHandler(HWND hWnd, UINT
// msg, WPARAM wParam, LPARAM lParam); extern IMGUI_IMPL_API LRESULT
// ImGui_ImplWin32_WndProcHandlerEx(HWND hWnd, UINT msg, WPARAM wParam, LPARAM
// lParam, ImGuiIO& io);
// ```
// If IMGUI_IMPL_API is defined as `extern "C"`, using `extern IMGUI_IMPL_API`
// in these declarations would expand to `extern extern "C"`, which is illegal
// in C++. This file provides `extern "C"` wrappers to avoid the issue.

#ifdef IMGUI_IMPL_API
#undef IMGUI_IMPL_API
#define IMGUI_IMPL_API
#endif

#include "imgui_impl_win32.cpp"

extern "C" bool ImGui_ImplWin32Fix_Init(void *hwnd) {
  return ImGui_ImplWin32_Init(hwnd);
}

extern "C" bool ImGui_ImplWin32Fix_InitForOpenGL(void *hwnd) {
  return ImGui_ImplWin32_InitForOpenGL(hwnd);
}

extern "C" void ImGui_ImplWin32Fix_Shutdown() {
  return ImGui_ImplWin32_Shutdown();
}

extern "C" void ImGui_ImplWin32Fix_NewFrame() {
  return ImGui_ImplWin32_NewFrame();
}

extern "C" LRESULT ImGui_ImplWin32Fix_WndProcHandler(HWND hWnd, UINT msg,
                                                     WPARAM wParam,
                                                     LPARAM lParam) {
  return ImGui_ImplWin32_WndProcHandler(hWnd, msg, wParam, lParam);
}

extern "C" void ImGui_ImplWin32Fix_EnableDpiAwareness() {
  return ImGui_ImplWin32_EnableDpiAwareness();
}

extern "C" float ImGui_ImplWin32Fix_GetDpiScaleForHwnd(void *hwnd) {
  return ImGui_ImplWin32_GetDpiScaleForHwnd(hwnd);
}

extern "C" float ImGui_ImplWin32Fix_GetDpiScaleForMonitor(void *monitor) {
  return ImGui_ImplWin32_GetDpiScaleForMonitor(monitor);
}

extern "C" void ImGui_ImplWin32Fix_EnableAlphaCompositing(void *hwnd) {
  return ImGui_ImplWin32_EnableAlphaCompositing(hwnd);
}