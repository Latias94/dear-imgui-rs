struct ImGuiContext;

// For WASM, we don't use thread_local as it may not be supported
#if defined(__EMSCRIPTEN__) || defined(__wasm__) || defined(__wasm32__)
ImGuiContext* MyImGuiTLS = nullptr;
#else
thread_local ImGuiContext* MyImGuiTLS;
#endif

#include "imgui.cpp"
#include "imgui_widgets.cpp"
#include "imgui_draw.cpp"
#include "imgui_tables.cpp"
#include "imgui_demo.cpp"
#ifdef IMGUI_ENABLE_FREETYPE
    #include "misc/freetype/imgui_freetype.cpp"
#endif

#ifdef _MSC_VER
#include "imgui_msvc_wrapper.cpp"
#endif
