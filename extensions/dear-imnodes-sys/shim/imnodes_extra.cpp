#include "imnodes_internal.h"
extern "C" {
bool* imnodes_getIOKeyShiftPtr() { return &ImGui::GetIO().KeyShift; }
bool* imnodes_getIOKeyAltPtr() { return &ImGui::GetIO().KeyAlt; }
void imnodes_EditorContextResetToDefault()
{
    if (GImNodes != nullptr) {
        GImNodes->EditorCtx = GImNodes->DefaultEditorCtx;
    }
}
ImNodesEditorContext* imnodes_EditorContextGetCurrent()
{
    return GImNodes != nullptr ? GImNodes->EditorCtx : nullptr;
}
void imnodes_EditorContextResetToDefaultIfCurrent(ImNodesEditorContext* editor)
{
    if (GImNodes != nullptr && GImNodes->EditorCtx == editor) {
        GImNodes->EditorCtx = GImNodes->DefaultEditorCtx;
    }
}
}
