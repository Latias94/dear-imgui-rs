#pragma once

#include "cimguizmo.h"

// Cancel interactions and discard the previous frame's native pointers and camera data.
// Call on the ImGui owner thread before BeginFrame when handing ImGuizmo to another context.
// Style, rectangle, enablement, projection mode, axis options and gizmo size are retained.
CIMGUI_API void ImGuizmo_ResetInteraction(void);
