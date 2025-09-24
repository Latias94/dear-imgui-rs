#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// Extra helpers to fetch ImGui IO modifier pointers
bool* imnodes_getIOKeyShiftPtr();
bool* imnodes_getIOKeyAltPtr();

#ifdef __cplusplus
}
#endif

