#ifndef DEAR_IMGUI_WRAPPER_H
#define DEAR_IMGUI_WRAPPER_H

// Include Dear ImGui headers
#include "third-party/imgui/imgui.h"

// Include optional features based on compile-time flags
#ifdef IMGUI_ENABLE_FREETYPE
#include "third-party/imgui/misc/freetype/imgui_freetype.h"
#endif

// Include internal headers for advanced features
#ifdef IMGUI_ENABLE_DOCKING
#include "third-party/imgui/imgui_internal.h"
#endif

#ifdef IMGUI_ENABLE_VIEWPORTS
#include "third-party/imgui/imgui_internal.h"
#endif

// We don't include imgui_internal.h by default to keep the API surface smaller
// Users can access internal APIs through the sys crate if needed

#ifdef __cplusplus
extern "C" {
#endif

// Docking function wrappers (only when docking is enabled)
#ifdef IMGUI_ENABLE_DOCKING
// Note: These functions are only available in the docking branch of Dear ImGui
// Current master branch implementation may not have all docking functions
// For full docking support, use the docking branch of Dear ImGui

// Placeholder for docking functions that would be available in docking branch:
// ImGuiID ImGui_DockSpace(ImGuiID id, const ImVec2& size, ImGuiDockNodeFlags flags, const ImGuiWindowClass* window_class);
// ImGuiID ImGui_DockSpaceOverViewport(ImGuiID id, const ImGuiViewport* viewport, ImGuiDockNodeFlags flags, const ImGuiWindowClass* window_class);
// void ImGui_SetNextWindowDockID(ImGuiID dock_id, ImGuiCond cond);
// ImGuiID ImGui_GetWindowDockID(void);
// bool ImGui_IsWindowDocked(void);

#endif // IMGUI_ENABLE_DOCKING

#ifdef __cplusplus
}
#endif

#endif // DEAR_IMGUI_WRAPPER_H
