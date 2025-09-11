// ImGuizmo C++ wrapper for dear-imgui-sys compatibility
// This file includes ImGuizmo sources and provides C++ bindings

// Define required macros before including headers
#define IMGUI_DEFINE_MATH_OPERATORS

// Include Dear ImGui headers only (implementation comes from dear-imgui-sys)
#include "imgui.h"
#include "imgui_internal.h"

// Include ImGuizmo implementation
#include "third-party/ImGuizmo/ImGuizmo.cpp"

// C-style wrapper functions
extern "C" {
    // Basic manipulation functions
    bool ImGuizmo_Manipulate(const float* view, const float* projection, int operation, int mode, 
                            float* matrix, float* deltaMatrix, const float* snap, 
                            const float* localBounds, const float* boundsSnap) {
        return ImGuizmo::Manipulate(view, projection, 
                                   static_cast<ImGuizmo::OPERATION>(operation),
                                   static_cast<ImGuizmo::MODE>(mode),
                                   matrix, deltaMatrix, snap, localBounds, boundsSnap);
    }

    // View manipulation
    void ImGuizmo_ViewManipulate(float* view, float length, float pos_x, float pos_y, 
                                float size_x, float size_y, unsigned int backgroundColor) {
        ImGuizmo::ViewManipulate(view, length, ImVec2(pos_x, pos_y), ImVec2(size_x, size_y), backgroundColor);
    }

    void ImGuizmo_ViewManipulate_Extended(float* view, const float* projection, int operation, int mode, 
                                         float* matrix, float length, float pos_x, float pos_y, 
                                         float size_x, float size_y, unsigned int backgroundColor) {
        ImGuizmo::ViewManipulate(view, projection, 
                                static_cast<ImGuizmo::OPERATION>(operation),
                                static_cast<ImGuizmo::MODE>(mode),
                                matrix, length, ImVec2(pos_x, pos_y), ImVec2(size_x, size_y), backgroundColor);
    }

    // Matrix decomposition and recomposition
    void ImGuizmo_DecomposeMatrixToComponents(const float* matrix, float* translation, 
                                             float* rotation, float* scale) {
        ImGuizmo::DecomposeMatrixToComponents(matrix, translation, rotation, scale);
    }

    void ImGuizmo_RecomposeMatrixFromComponents(const float* translation, const float* rotation, 
                                               const float* scale, float* matrix) {
        ImGuizmo::RecomposeMatrixFromComponents(translation, rotation, scale, matrix);
    }

    // Configuration functions
    void ImGuizmo_SetRect(float x, float y, float width, float height) {
        ImGuizmo::SetRect(x, y, width, height);
    }

    void ImGuizmo_SetOrthographic(bool isOrthographic) {
        ImGuizmo::SetOrthographic(isOrthographic);
    }

    void ImGuizmo_Enable(bool enable) {
        ImGuizmo::Enable(enable);
    }

    // Drawing functions
    void ImGuizmo_DrawCubes(const float* view, const float* projection, const float* matrices, int matrixCount) {
        ImGuizmo::DrawCubes(view, projection, matrices, matrixCount);
    }

    void ImGuizmo_DrawGrid(const float* view, const float* projection, const float* matrix, float gridSize) {
        ImGuizmo::DrawGrid(view, projection, matrix, gridSize);
    }

    // Query functions
    bool ImGuizmo_IsOver_Operation(int op) {
        return ImGuizmo::IsOver(static_cast<ImGuizmo::OPERATION>(op));
    }

    bool ImGuizmo_IsOver_Position(float* position, float pixelRadius) {
        return ImGuizmo::IsOver(position, pixelRadius);
    }

    bool ImGuizmo_IsUsing() {
        return ImGuizmo::IsUsing();
    }

    // Style and appearance
    void ImGuizmo_SetGizmoSizeClipSpace(float value) {
        ImGuizmo::SetGizmoSizeClipSpace(value);
    }

    void ImGuizmo_AllowAxisFlip(bool value) {
        ImGuizmo::AllowAxisFlip(value);
    }

    void ImGuizmo_SetAxisLimit(float value) {
        ImGuizmo::SetAxisLimit(value);
    }

    void ImGuizmo_SetAxisMask(bool x, bool y, bool z) {
        ImGuizmo::SetAxisMask(x, y, z);
    }

    void ImGuizmo_SetPlaneLimit(float value) {
        ImGuizmo::SetPlaneLimit(value);
    }

    // ID management
    void ImGuizmo_PushID_Str(const char* str_id) {
        ImGuizmo::PushID(str_id);
    }

    void ImGuizmo_PushID_StrRange(const char* str_id_begin, const char* str_id_end) {
        ImGuizmo::PushID(str_id_begin, str_id_end);
    }

    void ImGuizmo_PushID_Ptr(const void* ptr_id) {
        ImGuizmo::PushID(ptr_id);
    }

    void ImGuizmo_PushID_Int(int int_id) {
        ImGuizmo::PushID(int_id);
    }

    void ImGuizmo_PopID() {
        ImGuizmo::PopID();
    }

    unsigned int ImGuizmo_GetID_Str(const char* str_id) {
        return ImGuizmo::GetID(str_id);
    }

    unsigned int ImGuizmo_GetID_StrRange(const char* str_id_begin, const char* str_id_end) {
        return ImGuizmo::GetID(str_id_begin, str_id_end);
    }

    unsigned int ImGuizmo_GetID_Ptr(const void* ptr_id) {
        return ImGuizmo::GetID(ptr_id);
    }

    // Style access
    void ImGuizmo_GetStyle(float* translationLineThickness, float* translationLineArrowSize,
                          float* rotationLineThickness, float* rotationOuterLineThickness,
                          float* scaleLineThickness, float* scaleLineCircleSize,
                          float* hatchedAxisLineThickness, float* centerCircleSize,
                          float* colors) {
        ImGuizmo::Style& style = ImGuizmo::GetStyle();
        *translationLineThickness = style.TranslationLineThickness;
        *translationLineArrowSize = style.TranslationLineArrowSize;
        *rotationLineThickness = style.RotationLineThickness;
        *rotationOuterLineThickness = style.RotationOuterLineThickness;
        *scaleLineThickness = style.ScaleLineThickness;
        *scaleLineCircleSize = style.ScaleLineCircleSize;
        *hatchedAxisLineThickness = style.HatchedAxisLineThickness;
        *centerCircleSize = style.CenterCircleSize;
        
        // Copy colors (each color is 4 floats: r, g, b, a)
        for (int i = 0; i < ImGuizmo::COLOR::COUNT; ++i) {
            colors[i * 4 + 0] = style.Colors[i].x;
            colors[i * 4 + 1] = style.Colors[i].y;
            colors[i * 4 + 2] = style.Colors[i].z;
            colors[i * 4 + 3] = style.Colors[i].w;
        }
    }

    void ImGuizmo_SetStyle(float translationLineThickness, float translationLineArrowSize,
                          float rotationLineThickness, float rotationOuterLineThickness,
                          float scaleLineThickness, float scaleLineCircleSize,
                          float hatchedAxisLineThickness, float centerCircleSize,
                          const float* colors) {
        ImGuizmo::Style& style = ImGuizmo::GetStyle();
        style.TranslationLineThickness = translationLineThickness;
        style.TranslationLineArrowSize = translationLineArrowSize;
        style.RotationLineThickness = rotationLineThickness;
        style.RotationOuterLineThickness = rotationOuterLineThickness;
        style.ScaleLineThickness = scaleLineThickness;
        style.ScaleLineCircleSize = scaleLineCircleSize;
        style.HatchedAxisLineThickness = hatchedAxisLineThickness;
        style.CenterCircleSize = centerCircleSize;
        
        // Set colors (each color is 4 floats: r, g, b, a)
        for (int i = 0; i < ImGuizmo::COLOR::COUNT; ++i) {
            style.Colors[i].x = colors[i * 4 + 0];
            style.Colors[i].y = colors[i * 4 + 1];
            style.Colors[i].z = colors[i * 4 + 2];
            style.Colors[i].w = colors[i * 4 + 3];
        }
    }
}
