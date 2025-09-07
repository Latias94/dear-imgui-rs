// Thread-local context for Dear ImGui
// This allows for better Rust integration and potential multi-threading support
struct ImGuiContext;
thread_local ImGuiContext* MyImGuiTLS;

// Define configuration before including imgui sources
#define IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_KEYIO
#define IMGUI_USE_WCHAR32
#define IMGUI_DEFINE_MATH_OPERATORS

// Include all Dear ImGui source files
// This approach compiles everything into a single translation unit
#include "third-party/imgui/imgui.cpp"
#include "third-party/imgui/imgui_widgets.cpp"
#include "third-party/imgui/imgui_draw.cpp"
#include "third-party/imgui/imgui_tables.cpp"
#include "third-party/imgui/imgui_demo.cpp"

// Include freetype support if enabled
#ifdef IMGUI_ENABLE_FREETYPE
    #include "imgui/misc/freetype/imgui_freetype.cpp"
#endif

// Include MSVC ABI fix for Windows MSVC builds
#ifdef _MSC_VER
    #include "hack_msvc.cpp"
#endif

// All wrapper functions removed - using direct C++ bindings via bindgen
