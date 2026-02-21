#include "imgui_internal.h"
#include "imgui_te_context.h"
#include "imgui_te_engine.h"
#include "imgui_te_internal.h"

#include <cstdarg>

// Provided by `dear-imgui-sys` when built with `IMGUI_ENABLE_TEST_ENGINE`.
extern "C" void dear_imgui_rs_set_test_engine_hooks(
    void (*item_add)(ImGuiContext*, ImGuiID, const ImRect&, const ImGuiLastItemData*),
    void (*item_info)(ImGuiContext*, ImGuiID, const char*, ImGuiItemStatusFlags),
    void (*log_v)(ImGuiContext*, const char*, va_list),
    const char* (*find_item_debug_label)(ImGuiContext*, ImGuiID)
);

static void dear_imgui_test_engine_sys__hook_log_v(ImGuiContext* ui_ctx, const char* fmt, va_list args) {
    ImGuiTestEngine* engine = (ImGuiTestEngine*)ui_ctx->TestEngine;
    if (engine == nullptr || engine->TestContext == nullptr) {
        return;
    }
    engine->TestContext->LogExV(ImGuiTestVerboseLevel_Debug, ImGuiTestLogFlags_None, fmt, args);
}

extern "C" void dear_imgui_test_engine_sys_register_imgui_hooks(void) {
    // Note: hook symbols are renamed inside this crate's build (see build.rs) to avoid
    // clashing with the wrapper symbols provided by dear-imgui-sys.
    dear_imgui_rs_set_test_engine_hooks(
        &ImGuiTestEngineHook_ItemAdd,
        &ImGuiTestEngineHook_ItemInfo,
        &dear_imgui_test_engine_sys__hook_log_v,
        &ImGuiTestEngine_FindItemDebugLabel
    );
}
