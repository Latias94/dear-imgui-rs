#include "imgui_internal.h"

#ifdef IMGUI_ENABLE_TEST_ENGINE

#include <atomic>
#include <cstdarg>

namespace {

using ItemAddFn = void (*)(ImGuiContext*, ImGuiID, const ImRect&, const ImGuiLastItemData*);
using ItemInfoFn = void (*)(ImGuiContext*, ImGuiID, const char*, ImGuiItemStatusFlags);
using LogVFn = void (*)(ImGuiContext*, const char*, va_list);
using FindItemDebugLabelFn = const char* (*)(ImGuiContext*, ImGuiID);

std::atomic<ItemAddFn> g_item_add{nullptr};
std::atomic<ItemInfoFn> g_item_info{nullptr};
std::atomic<LogVFn> g_log_v{nullptr};
std::atomic<FindItemDebugLabelFn> g_find_item_debug_label{nullptr};

} // namespace

extern "C" void dear_imgui_rs_set_test_engine_hooks(
    ItemAddFn item_add,
    ItemInfoFn item_info,
    LogVFn log_v,
    FindItemDebugLabelFn find_item_debug_label
) noexcept {
    g_item_add.store(item_add, std::memory_order_release);
    g_item_info.store(item_info, std::memory_order_release);
    g_log_v.store(log_v, std::memory_order_release);
    g_find_item_debug_label.store(find_item_debug_label, std::memory_order_release);
}

void ImGuiTestEngineHook_ItemAdd(
    ImGuiContext* ctx,
    ImGuiID id,
    const ImRect& bb,
    const ImGuiLastItemData* item_data
) {
    if (auto hook = g_item_add.load(std::memory_order_acquire)) {
        hook(ctx, id, bb, item_data);
    }
}

void ImGuiTestEngineHook_ItemInfo(
    ImGuiContext* ctx,
    ImGuiID id,
    const char* label,
    ImGuiItemStatusFlags flags
) {
    if (auto hook = g_item_info.load(std::memory_order_acquire)) {
        hook(ctx, id, label, flags);
    }
}

void ImGuiTestEngineHook_Log(ImGuiContext* ctx, const char* fmt, ...) {
    auto hook = g_log_v.load(std::memory_order_acquire);
    if (!hook) {
        return;
    }

    va_list args;
    va_start(args, fmt);
    hook(ctx, fmt, args);
    va_end(args);
}

const char* ImGuiTestEngine_FindItemDebugLabel(ImGuiContext* ctx, ImGuiID id) {
    if (auto hook = g_find_item_debug_label.load(std::memory_order_acquire)) {
        return hook(ctx, id);
    }
    return nullptr;
}

#endif // IMGUI_ENABLE_TEST_ENGINE

