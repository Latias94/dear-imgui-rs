// MSVC ABI fix for Dear ImGui functions that return ImVec2
// 
// MSVC has a special ABI for functions that return small C++ types (sizeof <= 8).
// ImVec2 is 8 bytes and has a default constructor, which triggers this special handling.
// This causes FFI binding issues when calling from Rust.
//
// Solution: Create FFI-safe POD wrapper type and wrapper functions.

#include "third-party/imgui/imgui.h"

// FFI-safe POD type equivalent to ImVec2
struct ImVec2_rr { 
    float x, y; 
};

// Helper function to convert ImVec2 to ImVec2_rr
static inline ImVec2_rr _rr(ImVec2 v) { 
    return ImVec2_rr { v.x, v.y }; 
}

// Wrapper functions for ImVec2-returning functions
// These use the ImGui_ prefix to match our bindgen naming convention

extern "C" {

ImVec2_rr ImGui_GetWindowPos() { 
    return _rr(ImGui::GetWindowPos()); 
}

ImVec2_rr ImGui_GetWindowSize() { 
    return _rr(ImGui::GetWindowSize()); 
}

ImVec2_rr ImGui_GetContentRegionAvail() { 
    return _rr(ImGui::GetContentRegionAvail()); 
}

// Note: GetContentRegionMax and GetWindowContentRegion* functions
// may not exist in all Dear ImGui versions, commenting out for now

ImVec2_rr ImGui_GetFontTexUvWhitePixel() { 
    return _rr(ImGui::GetFontTexUvWhitePixel()); 
}

ImVec2_rr ImGui_GetCursorScreenPos() { 
    return _rr(ImGui::GetCursorScreenPos()); 
}

ImVec2_rr ImGui_GetCursorPos() { 
    return _rr(ImGui::GetCursorPos()); 
}

ImVec2_rr ImGui_GetCursorStartPos() { 
    return _rr(ImGui::GetCursorStartPos()); 
}

ImVec2_rr ImGui_GetItemRectMin() { 
    return _rr(ImGui::GetItemRectMin()); 
}

ImVec2_rr ImGui_GetItemRectMax() { 
    return _rr(ImGui::GetItemRectMax()); 
}

ImVec2_rr ImGui_GetItemRectSize() { 
    return _rr(ImGui::GetItemRectSize()); 
}

ImVec2_rr ImGui_CalcTextSize(const char* text, const char* text_end, bool hide_text_after_double_hash, float wrap_width) { 
    return _rr(ImGui::CalcTextSize(text, text_end, hide_text_after_double_hash, wrap_width)); 
}

ImVec2_rr ImGui_GetMousePos() { 
    return _rr(ImGui::GetMousePos()); 
}

ImVec2_rr ImGui_GetMousePosOnOpeningCurrentPopup() { 
    return _rr(ImGui::GetMousePosOnOpeningCurrentPopup()); 
}

ImVec2_rr ImGui_GetMouseDragDelta(ImGuiMouseButton button, float lock_threshold) { 
    return _rr(ImGui::GetMouseDragDelta(button, lock_threshold)); 
}

// Validation function to ensure our ABI fixes work correctly
// This can be called from Rust tests to verify the wrapper functions
ImVec2_rr ImGui_ValidateABIFix() {
    // Return a known value that can be verified from Rust
    return ImVec2_rr { 42.0f, 24.0f };
}

// Multi-viewport callback function wrappers
// These are needed because our callback functions return ImVec2, which has MSVC ABI issues

// Function pointer types for the callbacks
typedef ImVec2_rr (*PlatformGetWindowPosCallback)(void* viewport);
typedef ImVec2_rr (*PlatformGetWindowSizeCallback)(void* viewport);

// Global function pointers to store the Rust callbacks
static PlatformGetWindowPosCallback g_platform_get_window_pos_callback = nullptr;
static PlatformGetWindowSizeCallback g_platform_get_window_size_callback = nullptr;

// C++ wrapper functions that call the Rust callbacks and convert the result
// These use C++ linkage since they return ImVec2
ImVec2 Platform_GetWindowPos_Wrapper(ImGuiViewport* viewport) {
    if (g_platform_get_window_pos_callback) {
        ImVec2_rr result = g_platform_get_window_pos_callback(viewport);
        return ImVec2(result.x, result.y);
    }
    return ImVec2(0.0f, 0.0f);
}

ImVec2 Platform_GetWindowSize_Wrapper(ImGuiViewport* viewport) {
    if (g_platform_get_window_size_callback) {
        ImVec2_rr result = g_platform_get_window_size_callback(viewport);
        return ImVec2(result.x, result.y);
    }
    return ImVec2(800.0f, 600.0f);
}

// Functions to set the callback pointers from Rust
extern "C" void ImGui_SetPlatformGetWindowPosCallback(PlatformGetWindowPosCallback callback) {
    g_platform_get_window_pos_callback = callback;
}

extern "C" void ImGui_SetPlatformGetWindowSizeCallback(PlatformGetWindowSizeCallback callback) {
    g_platform_get_window_size_callback = callback;
}

// Functions to get the wrapper function pointers (to be used in PlatformIO)
extern "C" void* ImGui_GetPlatformGetWindowPosWrapper() {
    return (void*)Platform_GetWindowPos_Wrapper;
}

extern "C" void* ImGui_GetPlatformGetWindowSizeWrapper() {
    return (void*)Platform_GetWindowSize_Wrapper;
}

} // extern "C"
