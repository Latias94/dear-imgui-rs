#include "imnodes_internal.h"

#include <cstdlib>

#if defined(__cpp_exceptions) || defined(_CPPUNWIND)
#define IMNODES_TRY try
#define IMNODES_CATCH_ABORT catch (...) { std::abort(); }
#else
#define IMNODES_TRY
#define IMNODES_CATCH_ABORT
#endif

extern "C" {
void imnodes_EditorContextResetToDefault() IMNODES_TRY
{
    if (GImNodes != nullptr) {
        GImNodes->EditorCtx = GImNodes->DefaultEditorCtx;
    }
}
IMNODES_CATCH_ABORT

ImNodesEditorContext* imnodes_EditorContextGetCurrent() IMNODES_TRY
{
    return GImNodes != nullptr ? GImNodes->EditorCtx : nullptr;
}
IMNODES_CATCH_ABORT

void imnodes_EditorContextResetToDefaultIfCurrent(ImNodesEditorContext* editor) IMNODES_TRY
{
    if (GImNodes != nullptr && GImNodes->EditorCtx == editor) {
        GImNodes->EditorCtx = GImNodes->DefaultEditorCtx;
    }
}
IMNODES_CATCH_ABORT
}
