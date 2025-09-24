#include "imgui.h"
extern "C" {
bool* imnodes_getIOKeyShiftPtr() { return &ImGui::GetIO().KeyShift; }
bool* imnodes_getIOKeyAltPtr() { return &ImGui::GetIO().KeyAlt; }
}

