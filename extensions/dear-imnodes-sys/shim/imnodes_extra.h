#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// Extra helpers to fetch ImGui IO modifier pointers
bool* imnodes_getIOKeyShiftPtr();
bool* imnodes_getIOKeyAltPtr();

// Reset the active editor context to the current ImNodes context's default editor.
void imnodes_EditorContextResetToDefault();

// Reset only if the active editor context matches `editor`.
void imnodes_EditorContextResetToDefaultIfCurrent(ImNodesEditorContext* editor);

#ifdef __cplusplus
}
#endif

