// Wrapper header for Dear ImGui FFI bindings
// This file includes all necessary Dear ImGui headers for bindgen

#pragma once

// Define configuration before including imgui.h
#define IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_KEYIO
#define IMGUI_USE_WCHAR32
#define IMGUI_DEFINE_MATH_OPERATORS

// Thread-local context for better Rust integration
struct ImGuiContext;
extern thread_local ImGuiContext* MyImGuiTLS;
#define GImGui MyImGuiTLS

// Include Dear ImGui headers
#include "third-party/imgui/imgui.h"
#include "third-party/imgui/imgui_internal.h"

// Include freetype support if enabled
#ifdef IMGUI_ENABLE_FREETYPE
#include "third-party/imgui/misc/freetype/imgui_freetype.h"
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Docking functions
ImGuiID ImGui_DockSpace(ImGuiID dockspace_id, const ImVec2* size, ImGuiDockNodeFlags flags, const ImGuiWindowClass* window_class);
ImGuiID ImGui_DockSpaceOverViewport(ImGuiID dockspace_id, const ImGuiViewport* viewport, ImGuiDockNodeFlags flags, const ImGuiWindowClass* window_class);
void ImGui_SetNextWindowDockID(ImGuiID dock_id, ImGuiCond cond);
ImGuiID ImGui_GetWindowDockID(void);
bool ImGui_IsWindowDocked(void);
const ImGuiViewport* ImGui_GetMainViewport(void);
const ImGuiViewport* ImGui_GetWindowViewport(void);

// DockBuilder functions
void ImGui_DockBuilderRemoveNode(ImGuiID node_id);
ImGuiID ImGui_DockBuilderAddNode(ImGuiID node_id, ImGuiDockNodeFlags flags);
void ImGui_DockBuilderSetNodePos(ImGuiID node_id, const ImVec2* pos);
void ImGui_DockBuilderSetNodeSize(ImGuiID node_id, const ImVec2* size);
ImGuiID ImGui_DockBuilderSplitNode(ImGuiID node_id, ImGuiDir split_dir, float size_ratio_for_node_at_dir, ImGuiID* out_id_at_dir, ImGuiID* out_id_at_opposite_dir);
void ImGui_DockBuilderDockWindow(const char* window_name, ImGuiID node_id);
void ImGui_DockBuilderFinish(ImGuiID node_id);

// Window functions
bool ImGui_Begin(const char* name, bool* p_open, ImGuiWindowFlags flags);
void ImGui_End(void);

// Window property functions
void ImGui_SetNextWindowSize(const ImVec2* size, ImGuiCond cond);
void ImGui_SetNextWindowPos(const ImVec2* pos, ImGuiCond cond, const ImVec2* pivot);
void ImGui_SetNextWindowContentSize(const ImVec2* size);
void ImGui_SetNextWindowCollapsed(bool collapsed, ImGuiCond cond);
void ImGui_SetNextWindowFocus(void);
void ImGui_SetNextWindowBgAlpha(float alpha);

// Popup functions
bool ImGui_BeginPopup(const char* str_id, ImGuiWindowFlags flags);
void ImGui_EndPopup(void);
bool ImGui_BeginPopupModal(const char* name, bool* p_open, ImGuiWindowFlags flags);
bool ImGui_BeginPopupContextItem(const char* str_id, ImGuiPopupFlags popup_flags);

// Child window functions
bool ImGui_BeginChild(const char* str_id, const ImVec2* size, ImGuiChildFlags child_flags, ImGuiWindowFlags window_flags);
void ImGui_EndChild(void);

// Tree node and collapsing header functions
bool ImGui_CollapsingHeader(const char* label, ImGuiTreeNodeFlags flags);

// Draw list functions
ImDrawList* ImGui_GetWindowDrawList(void);
ImDrawList* ImGui_GetBackgroundDrawList(void);
ImDrawList* ImGui_GetForegroundDrawList(void);

#ifdef __cplusplus
}
#endif
