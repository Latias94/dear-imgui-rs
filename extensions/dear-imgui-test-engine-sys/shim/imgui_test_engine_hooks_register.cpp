#include "imgui_internal.h"
#include "imgui_te_context.h"
#include "imgui_te_engine.h"
#include "imgui_te_internal.h"
#include "cimgui_test_engine_internal.h"

#include <cstdarg>
#include <unordered_map>

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

namespace {

struct ShutdownObserver {
    ImGuiTestEngine* Engine;
    ImGuiID HookId;
};

thread_local std::unordered_map<ImGuiContext*, ShutdownObserver> g_shutdown_observers;

void context_shutdown_observer(ImGuiContext* ui_ctx, ImGuiContextHook*) {
    const auto observer = g_shutdown_observers.find(ui_ctx);
    if (observer == g_shutdown_observers.end()) {
        return;
    }
    const ImGuiTestEngine* engine = observer->second.Engine;
    if (ui_ctx->TestEngine == nullptr && engine->UiContextTarget == nullptr) {
        dear_imgui_test_engine_abi::release_process_binding(
            observer->second.Engine,
            ui_ctx
        );
        dear_imgui_test_engine_abi::increment(
            dear_imgui_test_engine_abi::Counter::EngineUnbound
        );
    }
    g_shutdown_observers.erase(observer);
}

}  // namespace

void dear_imgui_test_engine_abi::register_imgui_hooks() noexcept {
    // Note: hook symbols are renamed inside this crate's build (see build.rs) to avoid
    // clashing with the wrapper symbols provided by dear-imgui-sys.
    dear_imgui_rs_set_test_engine_hooks(
        &ImGuiTestEngineHook_ItemAdd,
        &ImGuiTestEngineHook_ItemInfo,
        &dear_imgui_test_engine_sys__hook_log_v,
        &ImGuiTestEngine_FindItemDebugLabel
    );
}

void dear_imgui_test_engine_abi::register_context_shutdown_observer(
    ImGuiTestEngine* engine,
    ImGuiContext* context
) {
    ImGuiContextHook hook{};
    hook.Type = ImGuiContextHookType_Shutdown;
    hook.Callback = context_shutdown_observer;
    hook.UserData = &g_shutdown_observers;
    const ImGuiID hook_id = ImGui::AddContextHook(context, &hook);

    try {
        g_shutdown_observers.insert_or_assign(
            context,
            ShutdownObserver{engine, hook_id}
        );
    } catch (...) {
        ImGui::RemoveContextHook(context, hook_id);
        throw;
    }
}

void dear_imgui_test_engine_abi::unregister_context_shutdown_observer(
    ImGuiTestEngine* engine,
    ImGuiContext* context
) noexcept {
    const auto observer = g_shutdown_observers.find(context);
    if (observer == g_shutdown_observers.end() || observer->second.Engine != engine) {
        return;
    }
    ImGui::RemoveContextHook(context, observer->second.HookId);
    g_shutdown_observers.erase(observer);
}
