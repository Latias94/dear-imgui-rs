#include "imgui_internal.h"

#ifdef IMGUI_ENABLE_TEST_ENGINE

#include <cstdarg>

namespace {

using ItemAddFn = void (*)(ImGuiContext*, ImGuiID, const ImRect&, const ImGuiLastItemData*);
using ItemInfoFn = void (*)(ImGuiContext*, ImGuiID, const char*, ImGuiItemStatusFlags);
using LogVFn = void (*)(ImGuiContext*, const char*, va_list);
using FindItemDebugLabelFn = const char* (*)(ImGuiContext*, ImGuiID);

ItemAddFn g_item_add = nullptr;
ItemInfoFn g_item_info = nullptr;
LogVFn g_log_v = nullptr;
FindItemDebugLabelFn g_find_item_debug_label = nullptr;

} // namespace

extern "C" void dear_imgui_rs_set_test_engine_hooks(
    ItemAddFn item_add,
    ItemInfoFn item_info,
    LogVFn log_v,
    FindItemDebugLabelFn find_item_debug_label
) {
    g_item_add = item_add;
    g_item_info = item_info;
    g_log_v = log_v;
    g_find_item_debug_label = find_item_debug_label;
}

void ImGuiTestEngineHook_ItemAdd(
    ImGuiContext* ctx,
    ImGuiID id,
    const ImRect& bb,
    const ImGuiLastItemData* item_data
) {
    if (g_item_add) {
        g_item_add(ctx, id, bb, item_data);
    }
}

void ImGuiTestEngineHook_ItemInfo(
    ImGuiContext* ctx,
    ImGuiID id,
    const char* label,
    ImGuiItemStatusFlags flags
) {
    if (g_item_info) {
        g_item_info(ctx, id, label, flags);
    }
}

void ImGuiTestEngineHook_Log(ImGuiContext* ctx, const char* fmt, ...) {
    if (!g_log_v) {
        return;
    }

    va_list args;
    va_start(args, fmt);
    g_log_v(ctx, fmt, args);
    va_end(args);
}

const char* ImGuiTestEngine_FindItemDebugLabel(ImGuiContext* ctx, ImGuiID id) {
    if (g_find_item_debug_label) {
        return g_find_item_debug_label(ctx, id);
    }
    return nullptr;
}

#endif // IMGUI_ENABLE_TEST_ENGINE

