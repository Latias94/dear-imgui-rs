// Thread-local context for Dear ImGui
// This allows for better Rust integration and potential multi-threading support
struct ImGuiContext;
thread_local ImGuiContext* MyImGuiTLS;

// Define configuration before including imgui sources
#define IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_KEYIO
#define IMGUI_USE_WCHAR32
#define IMGUI_DEFINE_MATH_OPERATORS

// Include all Dear ImGui source files
// This approach compiles everything into a single translation unit
#include "third-party/imgui/imgui.cpp"
#include "third-party/imgui/imgui_widgets.cpp"
#include "third-party/imgui/imgui_draw.cpp"
#include "third-party/imgui/imgui_tables.cpp"
#include "third-party/imgui/imgui_demo.cpp"

// Include freetype support if enabled
#ifdef IMGUI_ENABLE_FREETYPE
    #include "imgui/misc/freetype/imgui_freetype.cpp"
#endif

// Include MSVC ABI fix for Windows MSVC builds
#ifdef _MSC_VER
    #include "hack_msvc.cpp"
#endif

// C wrapper functions for Dear ImGui docking functionality
extern "C" {
    // Docking functions
    ImGuiID ImGui_DockSpace(ImGuiID dockspace_id, const ImVec2* size, ImGuiDockNodeFlags flags, const ImGuiWindowClass* window_class) {
        return ImGui::DockSpace(dockspace_id, size ? *size : ImVec2(0, 0), flags, window_class);
    }

    ImGuiID ImGui_DockSpaceOverViewport(ImGuiID dockspace_id, const ImGuiViewport* viewport, ImGuiDockNodeFlags flags, const ImGuiWindowClass* window_class) {
        return ImGui::DockSpaceOverViewport(dockspace_id, viewport, flags, window_class);
    }

    void ImGui_SetNextWindowDockID(ImGuiID dock_id, ImGuiCond cond) {
        ImGui::SetNextWindowDockID(dock_id, cond);
    }

    ImGuiID ImGui_GetWindowDockID() {
        return ImGui::GetWindowDockID();
    }

    bool ImGui_IsWindowDocked() {
        return ImGui::IsWindowDocked();
    }

    const ImGuiViewport* ImGui_GetMainViewport() {
        return ImGui::GetMainViewport();
    }

    const ImGuiViewport* ImGui_GetWindowViewport() {
        return ImGui::GetWindowViewport();
    }

    // DockBuilder functions
    void ImGui_DockBuilderRemoveNode(ImGuiID node_id) {
        ImGui::DockBuilderRemoveNode(node_id);
    }

    ImGuiID ImGui_DockBuilderAddNode(ImGuiID node_id, ImGuiDockNodeFlags flags) {
        return ImGui::DockBuilderAddNode(node_id, flags);
    }

    void ImGui_DockBuilderSetNodePos(ImGuiID node_id, const ImVec2* pos) {
        ImGui::DockBuilderSetNodePos(node_id, pos ? *pos : ImVec2(0, 0));
    }

    void ImGui_DockBuilderSetNodeSize(ImGuiID node_id, const ImVec2* size) {
        ImGui::DockBuilderSetNodeSize(node_id, size ? *size : ImVec2(0, 0));
    }

    ImGuiID ImGui_DockBuilderSplitNode(ImGuiID node_id, ImGuiDir split_dir, float size_ratio_for_node_at_dir, ImGuiID* out_id_at_dir, ImGuiID* out_id_at_opposite_dir) {
        return ImGui::DockBuilderSplitNode(node_id, split_dir, size_ratio_for_node_at_dir, out_id_at_dir, out_id_at_opposite_dir);
    }

    void ImGui_DockBuilderDockWindow(const char* window_name, ImGuiID node_id) {
        ImGui::DockBuilderDockWindow(window_name, node_id);
    }

    void ImGui_DockBuilderFinish(ImGuiID node_id) {
        ImGui::DockBuilderFinish(node_id);
    }

    // Window functions
    bool ImGui_Begin(const char* name, bool* p_open, ImGuiWindowFlags flags) {
        return ImGui::Begin(name, p_open, flags);
    }

    void ImGui_End() {
        ImGui::End();
    }

    // Window property functions
    void ImGui_SetNextWindowSize(const ImVec2* size, ImGuiCond cond) {
        ImGui::SetNextWindowSize(size ? *size : ImVec2(0, 0), cond);
    }

    void ImGui_SetNextWindowPos(const ImVec2* pos, ImGuiCond cond, const ImVec2* pivot) {
        ImGui::SetNextWindowPos(pos ? *pos : ImVec2(0, 0), cond, pivot ? *pivot : ImVec2(0, 0));
    }

    void ImGui_SetNextWindowContentSize(const ImVec2* size) {
        ImGui::SetNextWindowContentSize(size ? *size : ImVec2(0, 0));
    }

    void ImGui_SetNextWindowCollapsed(bool collapsed, ImGuiCond cond) {
        ImGui::SetNextWindowCollapsed(collapsed, cond);
    }

    void ImGui_SetNextWindowFocus() {
        ImGui::SetNextWindowFocus();
    }

    void ImGui_SetNextWindowBgAlpha(float alpha) {
        ImGui::SetNextWindowBgAlpha(alpha);
    }

    // Popup functions
    bool ImGui_BeginPopup(const char* str_id, ImGuiWindowFlags flags) {
        return ImGui::BeginPopup(str_id, flags);
    }

    void ImGui_EndPopup() {
        ImGui::EndPopup();
    }

    bool ImGui_BeginPopupModal(const char* name, bool* p_open, ImGuiWindowFlags flags) {
        return ImGui::BeginPopupModal(name, p_open, flags);
    }

    bool ImGui_BeginPopupContextItem(const char* str_id, ImGuiPopupFlags popup_flags) {
        return ImGui::BeginPopupContextItem(str_id, popup_flags);
    }

    // Child window functions
    bool ImGui_BeginChild(const char* str_id, const ImVec2* size, ImGuiChildFlags child_flags, ImGuiWindowFlags window_flags) {
        return ImGui::BeginChild(str_id, size ? *size : ImVec2(0, 0), child_flags, window_flags);
    }

    void ImGui_EndChild() {
        ImGui::EndChild();
    }

    // Tree node and collapsing header functions
    bool ImGui_CollapsingHeader(const char* label, ImGuiTreeNodeFlags flags) {
        return ImGui::CollapsingHeader(label, flags);
    }

    // Draw list functions
    ImDrawList* ImGui_GetWindowDrawList() {
        return ImGui::GetWindowDrawList();
    }

    ImDrawList* ImGui_GetBackgroundDrawList() {
        return ImGui::GetBackgroundDrawList();
    }

    ImDrawList* ImGui_GetForegroundDrawList() {
        return ImGui::GetForegroundDrawList();
    }
}
