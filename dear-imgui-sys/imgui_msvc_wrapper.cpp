// imgui_msvc_wrapper.cpp
// Unified MSVC ABI fixes and wrappers for Dear ImGui Rust bindings
// This file contains all workarounds for MSVC ABI compatibility issues

#include "imgui.h"

// ============================================================================
// FFI-Safe POD Types
// ============================================================================

// FFI-safe POD type equivalent to ImVec2
// Used to avoid MSVC ABI issues with returning small C++ classes by value
// This MUST be a simple C-style struct with no constructors or operators
struct ImVec2_Pod {
    float x, y;
};

// FFI-safe POD type equivalent to ImVec4
struct ImVec4_Pod {
    float x, y, z, w;
};

// Helper function for conversion
static inline ImVec2_Pod to_pod(const ImVec2& v) {
    ImVec2_Pod result;
    result.x = v.x;
    result.y = v.y;
    return result;
}

// ============================================================================
// MSVC ABI Fix: Regular ImGui Functions  
// ============================================================================
// These functions return ImVec2/ImVec4 by value, which has ABI issues on MSVC
// We provide wrapper functions that return FFI-safe POD types

extern "C" {

#ifdef _MSC_VER
// Only needed for MSVC - these wrappers return POD types instead of ImVec2
ImVec2_Pod ImGui_GetWindowPos() { 
    return to_pod(ImGui::GetWindowPos()); 
}

ImVec2_Pod ImGui_GetWindowSize() { 
    return to_pod(ImGui::GetWindowSize()); 
}

ImVec2_Pod ImGui_GetContentRegionAvail() {
    return to_pod(ImGui::GetContentRegionAvail());
}

ImVec2_Pod ImGui_GetFontTexUvWhitePixel() { 
    return to_pod(ImGui::GetFontTexUvWhitePixel()); 
}

ImVec2_Pod ImGui_GetCursorScreenPos() { 
    return to_pod(ImGui::GetCursorScreenPos()); 
}

ImVec2_Pod ImGui_GetCursorPos() { 
    return to_pod(ImGui::GetCursorPos()); 
}

ImVec2_Pod ImGui_GetCursorStartPos() { 
    return to_pod(ImGui::GetCursorStartPos()); 
}

ImVec2_Pod ImGui_GetItemRectMin() { 
    return to_pod(ImGui::GetItemRectMin()); 
}

ImVec2_Pod ImGui_GetItemRectMax() { 
    return to_pod(ImGui::GetItemRectMax()); 
}

ImVec2_Pod ImGui_GetItemRectSize() { 
    return to_pod(ImGui::GetItemRectSize()); 
}

ImVec2_Pod ImGui_CalcTextSize(const char* text, const char* text_end, bool hide_text_after_double_hash, float wrap_width) { 
    return to_pod(ImGui::CalcTextSize(text, text_end, hide_text_after_double_hash, wrap_width)); 
}

ImVec2_Pod ImGui_GetMousePos() { 
    return to_pod(ImGui::GetMousePos()); 
}

ImVec2_Pod ImGui_GetMousePosOnOpeningCurrentPopup() { 
    return to_pod(ImGui::GetMousePosOnOpeningCurrentPopup()); 
}

ImVec2_Pod ImGui_GetMouseDragDelta(ImGuiMouseButton button, float lock_threshold) { 
    return to_pod(ImGui::GetMouseDragDelta(button, lock_threshold)); 
}
#endif // _MSC_VER

// ============================================================================
// Multi-Viewport Callback Support
// ============================================================================
// Platform callbacks that return ImVec2 also have ABI issues
// We use a different approach: callbacks use out-parameters instead of return values

// Storage for our safe callbacks that use out parameters
static void (*g_Platform_GetWindowPos_OutParam)(ImGuiViewport*, ImVec2*) = nullptr;
static void (*g_Platform_GetWindowSize_OutParam)(ImGuiViewport*, ImVec2*) = nullptr;
static void (*g_Platform_GetWindowFramebufferScale_OutParam)(ImGuiViewport*, ImVec2*) = nullptr;
static void (*g_Platform_GetWindowWorkAreaInsets_OutParam)(ImGuiViewport*, ImVec4*) = nullptr;

// Thunk functions that convert from out-parameter style to return-by-value style
static ImVec2 Platform_GetWindowPos_Thunk(ImGuiViewport* viewport) {
    ImVec2 result = ImVec2(0, 0);
    if (g_Platform_GetWindowPos_OutParam) {
        g_Platform_GetWindowPos_OutParam(viewport, &result);
    }
    return result;
}

static ImVec2 Platform_GetWindowSize_Thunk(ImGuiViewport* viewport) {
    ImVec2 result = ImVec2(800, 600);
    if (g_Platform_GetWindowSize_OutParam) {
        g_Platform_GetWindowSize_OutParam(viewport, &result);
    }
    return result;
}

static ImVec2 Platform_GetWindowFramebufferScale_Thunk(ImGuiViewport* viewport) {
    ImVec2 result = ImVec2(1.0f, 1.0f);
    if (g_Platform_GetWindowFramebufferScale_OutParam) {
        g_Platform_GetWindowFramebufferScale_OutParam(viewport, &result);
    }
    return result;
}

static ImVec4 Platform_GetWindowWorkAreaInsets_Thunk(ImGuiViewport* viewport) {
    ImVec4 result = ImVec4(0.0f, 0.0f, 0.0f, 0.0f);
    if (g_Platform_GetWindowWorkAreaInsets_OutParam) {
        g_Platform_GetWindowWorkAreaInsets_OutParam(viewport, &result);
    }
    return result;
}

// Set the Platform_GetWindowPos callback using an out-parameter style
// This avoids ABI issues with returning ImVec2 by value
void ImGui_Platform_SetGetWindowPosCallback(void (*callback)(ImGuiViewport*, ImVec2*)) {
    g_Platform_GetWindowPos_OutParam = callback;
    if (callback) {
        ImGui::GetPlatformIO().Platform_GetWindowPos = Platform_GetWindowPos_Thunk;
    } else {
        ImGui::GetPlatformIO().Platform_GetWindowPos = nullptr;
    }
}

// Set the Platform_GetWindowSize callback using an out-parameter style
// This avoids ABI issues with returning ImVec2 by value
void ImGui_Platform_SetGetWindowSizeCallback(void (*callback)(ImGuiViewport*, ImVec2*)) {
    g_Platform_GetWindowSize_OutParam = callback;
    if (callback) {
        ImGui::GetPlatformIO().Platform_GetWindowSize = Platform_GetWindowSize_Thunk;
    } else {
        ImGui::GetPlatformIO().Platform_GetWindowSize = nullptr;
    }
}

// Set the Platform_GetWindowFramebufferScale callback using an out-parameter style
// This avoids ABI issues with returning ImVec2 by value
void ImGui_Platform_SetGetWindowFramebufferScaleCallback(void (*callback)(ImGuiViewport*, ImVec2*)) {
    g_Platform_GetWindowFramebufferScale_OutParam = callback;
    if (callback) {
        ImGui::GetPlatformIO().Platform_GetWindowFramebufferScale = Platform_GetWindowFramebufferScale_Thunk;
    } else {
        ImGui::GetPlatformIO().Platform_GetWindowFramebufferScale = nullptr;
    }
}

// Set the Platform_GetWindowWorkAreaInsets callback using an out-parameter style
// This avoids ABI issues with returning ImVec4 by value
void ImGui_Platform_SetGetWindowWorkAreaInsetsCallback(void (*callback)(ImGuiViewport*, ImVec4*)) {
    g_Platform_GetWindowWorkAreaInsets_OutParam = callback;
    if (callback) {
        ImGui::GetPlatformIO().Platform_GetWindowWorkAreaInsets = Platform_GetWindowWorkAreaInsets_Thunk;
    } else {
        ImGui::GetPlatformIO().Platform_GetWindowWorkAreaInsets = nullptr;
    }
}

} // extern "C"
