#pragma once

// Include ImGui headers for type definitions
#include "imgui.h"
#include "ImGuizmo.h"

#ifdef __cplusplus
extern "C" {
#endif

// C-style wrapper functions for ImGuizmo

// Basic manipulation functions
bool ImGuizmo_Manipulate(const float* view, const float* projection, int operation, int mode, 
                        float* matrix, float* deltaMatrix, const float* snap, 
                        const float* localBounds, const float* boundsSnap);

// View manipulation
void ImGuizmo_ViewManipulate(float* view, float length, float pos_x, float pos_y, 
                            float size_x, float size_y, unsigned int backgroundColor);

void ImGuizmo_ViewManipulate_Extended(float* view, const float* projection, int operation, int mode, 
                                     float* matrix, float length, float pos_x, float pos_y, 
                                     float size_x, float size_y, unsigned int backgroundColor);

// Matrix decomposition and recomposition
void ImGuizmo_DecomposeMatrixToComponents(const float* matrix, float* translation, 
                                         float* rotation, float* scale);
void ImGuizmo_RecomposeMatrixFromComponents(const float* translation, const float* rotation, 
                                           const float* scale, float* matrix);

// Configuration functions
void ImGuizmo_SetRect(float x, float y, float width, float height);
void ImGuizmo_SetOrthographic(bool isOrthographic);
void ImGuizmo_Enable(bool enable);

// Drawing functions
void ImGuizmo_DrawCubes(const float* view, const float* projection, const float* matrices, int matrixCount);
void ImGuizmo_DrawGrid(const float* view, const float* projection, const float* matrix, float gridSize);

// Query functions
bool ImGuizmo_IsOver_Operation(int op);
bool ImGuizmo_IsOver_Position(float* position, float pixelRadius);
bool ImGuizmo_IsUsing();

// Style and appearance
void ImGuizmo_SetGizmoSizeClipSpace(float value);
void ImGuizmo_AllowAxisFlip(bool value);
void ImGuizmo_SetAxisLimit(float value);
void ImGuizmo_SetAxisMask(bool x, bool y, bool z);
void ImGuizmo_SetPlaneLimit(float value);

// ID management
void ImGuizmo_PushID_Str(const char* str_id);
void ImGuizmo_PushID_StrRange(const char* str_id_begin, const char* str_id_end);
void ImGuizmo_PushID_Ptr(const void* ptr_id);
void ImGuizmo_PushID_Int(int int_id);
void ImGuizmo_PopID();
unsigned int ImGuizmo_GetID_Str(const char* str_id);
unsigned int ImGuizmo_GetID_StrRange(const char* str_id_begin, const char* str_id_end);
unsigned int ImGuizmo_GetID_Ptr(const void* ptr_id);

// Style access
void ImGuizmo_GetStyle(float* translationLineThickness, float* translationLineArrowSize,
                      float* rotationLineThickness, float* rotationOuterLineThickness,
                      float* scaleLineThickness, float* scaleLineCircleSize,
                      float* hatchedAxisLineThickness, float* centerCircleSize,
                      float* colors); // colors should be float[COLOR::COUNT * 4]

void ImGuizmo_SetStyle(float translationLineThickness, float translationLineArrowSize,
                      float rotationLineThickness, float rotationOuterLineThickness,
                      float scaleLineThickness, float scaleLineCircleSize,
                      float hatchedAxisLineThickness, float centerCircleSize,
                      const float* colors); // colors should be float[COLOR::COUNT * 4]

#ifdef __cplusplus
}
#endif
