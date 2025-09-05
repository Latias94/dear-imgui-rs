// Wrapper header for Dear ImGui FFI bindings
// This file includes all necessary Dear ImGui headers for bindgen

#pragma once

// Define configuration before including imgui.h
#define IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_KEYIO
#define IMGUI_USE_WCHAR32
#define IMGUI_DEFINE_MATH_OPERATORS

// Thread-local context for better Rust integration
struct ImGuiContext;
extern thread_local ImGuiContext* MyImGuiTLS;
#define GImGui MyImGuiTLS

// Include Dear ImGui headers
#include "imgui/imgui.h"
#include "imgui/imgui_internal.h"

// Include freetype support if enabled
#ifdef IMGUI_ENABLE_FREETYPE
#include "imgui/misc/freetype/imgui_freetype.h"
#endif
