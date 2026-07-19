#pragma once

#include "cimnodes.h"

#ifdef __cplusplus
extern "C" {
#endif

// Extra helpers to fetch ImGui IO modifier pointers
CIMGUI_API bool* imnodes_getIOKeyShiftPtr();
CIMGUI_API bool* imnodes_getIOKeyAltPtr();

// Reset the active editor context to the current ImNodes context's default editor.
CIMGUI_API void imnodes_EditorContextResetToDefault();

// Return the active editor context for the current ImNodes context.
CIMGUI_API ImNodesEditorContext* imnodes_EditorContextGetCurrent();

// Reset only if the active editor context matches `editor`.
CIMGUI_API void imnodes_EditorContextResetToDefaultIfCurrent(ImNodesEditorContext* editor);

#ifdef __cplusplus
}
#endif

