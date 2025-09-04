// Wrapper file to compile all Dear ImGui source files together
// This improves build speed and allows better optimization

#include "third-party/imgui/imgui.cpp"
#include "third-party/imgui/imgui_demo.cpp"
#include "third-party/imgui/imgui_draw.cpp"
#include "third-party/imgui/imgui_tables.cpp"
#include "third-party/imgui/imgui_widgets.cpp"

#ifdef IMGUI_ENABLE_FREETYPE
#include "third-party/imgui/misc/freetype/imgui_freetype.cpp"
#endif
