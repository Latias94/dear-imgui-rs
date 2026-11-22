// Included at the end of the upstream ImGuizmo translation unit, which owns gContext.
#include "imguizmo_extra.h"

CIMGUI_API void ImGuizmo_ResetInteraction(void)
{
    using namespace IMGUIZMO_NAMESPACE;

    // Context's constructor relies on static zero-initialization for much of the camera
    // and drag state. Start with the same initialization as the upstream singleton.
    static const Context initial;
    Context next = initial;

    // Carry forward every user-configurable value. The native ImVector assignments release
    // the old ID stack and per-ID view interactions when the new state is installed.
    next.mStyle = gContext.mStyle;
    next.mbEnable = gContext.mbEnable;
    next.mIsOrthographic = gContext.mIsOrthographic;
    next.mAllowAxisFlip = gContext.mAllowAxisFlip;
    next.mAxisMask = gContext.mAxisMask;
    next.mAxisLimit = gContext.mAxisLimit;
    next.mPlaneLimit = gContext.mPlaneLimit;
    next.mGizmoSizeClipSpace = gContext.mGizmoSizeClipSpace;
    next.mX = gContext.mX;
    next.mY = gContext.mY;
    next.mWidth = gContext.mWidth;
    next.mHeight = gContext.mHeight;
    next.mXMax = gContext.mXMax;
    next.mYMax = gContext.mYMax;
    next.mDisplayRatio = gContext.mDisplayRatio;

    gContext = next;
}
